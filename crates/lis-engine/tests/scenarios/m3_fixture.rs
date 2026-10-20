//! The release bench of the M3 race and crash suite
//! (`docs/planning/m3-design-brief.md` §3.6): the M2 journey laboratory
//! ([`crate::m2_lab`]) with a laboratory profile, a releaser **R** who is not
//! the technical reviewer **T**, and one specimen whose definitive component
//! (`SYN-MOR`, `SYN-OXY`) is imported, QC-accepted and approved by T. The
//! journey order authorizes a reflex whose trigger is never assessed, so
//! every bench report is partial and its release acknowledges that.
//!
//! The engine has one window session, so two commands raced with
//! `tokio::join!` both run as the signed-in user. The race actor holds the
//! releaser role plus every role the raced changes need; T reviewed the
//! lines, so the two-person rule still holds. Synthetic data only.

use std::cell::Cell;
use std::time::Duration;

use lis_analytical::{
    AttemptKind, LotView, MaterialKind, QcOutcome, QcRole, QcScope, RevisionIndicator, RunId,
};
use lis_auth::RoleCode;
use lis_catalog::{
    AnalyteId, ValidityAction, ValidityCategory, ValidityPolicyContent, ValidityPolicyVersionId,
};
use lis_core::UserId;
use lis_engine::{
    AddPatientPositionInput, AddQcPositionInput, ApproveValidityPolicyVersion, AssessQcInput,
    CloseRunInput, CreateRunInput, DraftValidityPolicyVersion, GetRunInput, GetRunQcInput,
    ListLotsInput, RecordValidityAssessmentInput, ResultRevisionId, SetLaboratoryProfile,
};

use crate::fixture::NEW_PASSWORD;
use crate::m2_config::URINE;
use crate::m2_lab::{Lab, Role, Sample};
use crate::m2_steps::{JOURNEY, line, message};

/// Validity categories of the bench's second policy version.
const VALID: &str = "SYN-VALID";
pub(crate) const HOLD: &str = "SYN-HOLD";

/// Who acts on the bench.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Staff {
    /// A journey role: A, Q, the accessioner, the analyst or T.
    Desk(Role),
    /// R: the releaser role only.
    Releaser,
    /// The race actor: releaser, quality manager, accessioner, technical
    /// reviewer and administrator.
    Racer,
}

/// Which specimen the bench reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    /// Journey sample 0 with the panel only.
    Panel,
    /// A sample with the panel plus a validity component, assessed valid
    /// under a policy that also has a `hold` category. Its creatinine is
    /// measured on its own run with both controls, QC-accepted and
    /// approved by T, so the validity line is reported (REQ-RPT-008).
    Validity,
}

/// A bench account and its authenticator secret.
struct Account {
    user_id: UserId,
    username: &'static str,
    secret: String,
}

/// The bench: see the module documentation.
pub(crate) struct Bench {
    lab: Lab,
    releaser: Account,
    racer: Account,
    /// True while a bench account is signed in, which the desk does not know.
    desk_stale: Cell<bool>,
    /// The index of the reported sample in `lab.samples`.
    pub(crate) index: usize,
    /// The approved policy version with both validity categories (validity variant).
    pub(crate) validity_policy: Option<ValidityPolicyVersionId>,
}

impl Bench {
    pub(crate) async fn start(variant: Variant) -> Self {
        Self::start_as(variant, lis_engine::InstallationKind::Laboratory).await
    }

    /// The same bench in an installation of `kind` (proposed REQ-OPS-016).
    pub(crate) async fn start_as(variant: Variant, kind: lis_engine::InstallationKind) -> Self {
        let lab =
            Lab::start_as_with_profile(RevisionIndicator::None, kind, Some(laboratory_profile()))
                .await;
        let (releaser, racer) = accounts(&lab).await;
        let mut bench = Self {
            lab,
            releaser,
            racer,
            desk_stale: Cell::new(true),
            index: 0,
            validity_policy: None,
        };
        match variant {
            Variant::Panel => {
                let lab = bench.desk().await;
                lab.import(JOURNEY).await;
                lab.accept_qc().await;
            }
            Variant::Validity => bench.add_validity_sample().await,
        }
        bench.approve_reported(variant).await;
        bench
    }

    /// T approves the drug lines, and the creatinine of the validity variant.
    async fn approve_reported(&self, variant: Variant) {
        let mut reviewed = vec![
            self.revision("SYN-MOR").await,
            self.revision("SYN-OXY").await,
        ];
        if variant == Variant::Validity {
            reviewed.push(self.revision("SYN-CRE").await);
        }
        let lab = self.desk().await;
        lab.approve(self.index, reviewed).await.unwrap();
    }

    /// The journey laboratory, with the desk's sign-in state made true.
    pub(crate) async fn desk(&self) -> &Lab {
        if self.desk_stale.get() {
            self.sign_in(Staff::Desk(Role::Admin)).await;
        }
        &self.lab
    }

    /// The journey laboratory for engine calls as whoever is signed in.
    pub(crate) fn lab(&self) -> &Lab {
        &self.lab
    }

    pub(crate) fn sample(&self) -> &Sample {
        &self.lab.samples[self.index]
    }

    pub(crate) fn user_id(&self, staff: Staff) -> UserId {
        match staff {
            Staff::Releaser => self.releaser.user_id,
            Staff::Racer => self.racer.user_id,
            Staff::Desk(_) => panic!("desk accounts are not bench accounts"),
        }
    }

    /// Signs `staff` in for real.
    pub(crate) async fn sign_in(&self, staff: Staff) {
        let account = match staff {
            Staff::Desk(role) => return self.sign_in_desk(role).await,
            Staff::Releaser => &self.releaser,
            Staff::Racer => &self.racer,
        };
        self.lab
            .t()
            .sign_in_with(account.username, NEW_PASSWORD, &account.secret)
            .await;
        self.desk_stale.set(true);
    }

    async fn sign_in_desk(&self, role: Role) {
        if self.desk_stale.replace(false) {
            // The desk skips signing in the role it believes is signed in;
            // signing another role first makes the next sign-in real.
            let other = if role == Role::Admin {
                Role::Quality
            } else {
                Role::Admin
            };
            self.lab.sign_in(other).await;
        }
        self.lab.sign_in(role).await;
    }

    /// Stops without shutdown, as a crash would, and starts again; nobody
    /// is signed in afterwards.
    pub(crate) async fn crash_and_restart(mut self) -> Self {
        self.lab.desk.t =
            tokio::time::timeout(Duration::from_secs(11), self.lab.desk.t.crash_and_restart())
                .await
                .expect("restart exceeded its bounded lock-release wait");
        self.desk_stale.set(true);
        self
    }

    /// The reported sample's current revision of the analyte `code`.
    pub(crate) async fn revision(&self, code: &str) -> ResultRevisionId {
        let results = self.desk().await.results(self.index).await;
        line(&results, code).current.as_ref().unwrap().revision_id
    }

    /// A validity sample on the drug run, its creatinine on a validity run,
    /// and a `valid` assessment under a policy that can also `hold`.
    async fn add_validity_sample(&mut self) {
        let policy = self.approve_hold_policy().await;
        self.validity_policy = Some(policy);
        let sample = self
            .lab
            .add_sample(self.lab.orderables.with_validity.1)
            .await;
        self.lab.place(&sample).await;
        let label = sample.label_code.clone();
        self.lab.samples.push(sample);
        self.index = self.lab.samples.len() - 1;
        self.lab.import(JOURNEY).await;
        self.lab
            .import(&message(&[
                "P|1",
                &format!("O|1|{label}|SYN-POS-21||R"),
                "R|1|^^^SYN-MOR|75.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
                "R|2|^^^SYN-OXY|12.0|ng/mL||N||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
            ]))
            .await;
        self.lab.accept_qc().await;
        self.measure_creatinine(&label).await;
        let creatinine = self.revision("SYN-CRE").await;
        self.lab.sign_in(Role::Reviewer).await;
        self.lab
            .engine()
            .record_validity_assessment(RecordValidityAssessmentInput {
                specimen_id: self.sample().specimen_id,
                policy_version_id: policy,
                category_code: VALID.to_owned(),
                evidence_revision_ids: vec![creatinine],
                supersedes_id: None,
                reason: None,
            })
            .await
            .unwrap();
    }

    /// Drafted by A, approved by Q: `SYN-VALID` (no action) and `SYN-HOLD`.
    async fn approve_hold_policy(&self) -> ValidityPolicyVersionId {
        let lab = self.desk().await;
        lab.sign_in(Role::Admin).await;
        let urine = lab
            .engine()
            .list_specimen_types()
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.code == URINE)
            .unwrap()
            .id;
        let category = |code: &str, action| ValidityCategory {
            code: code.to_owned(),
            label: format!("Synthetic {code}"),
            action,
            qualifier_text: None,
        };
        let content = ValidityPolicyContent {
            categories: vec![
                category(VALID, ValidityAction::None),
                category(HOLD, ValidityAction::Hold),
            ],
        };
        let id = lab
            .engine()
            .draft_validity_policy_version(DraftValidityPolicyVersion {
                specimen_type_id: urine,
                content,
            })
            .await
            .unwrap()
            .id;
        lab.sign_in(Role::Quality).await;
        lab.engine()
            .approve_validity_policy_version(ApproveValidityPolicyVersion {
                policy_version_id: id,
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .unwrap();
        id
    }

    /// Closes the drug run so the label matches one open position, imports
    /// the creatinine and both controls from its own run, and accepts its
    /// QC as Q.
    async fn measure_creatinine(&self, label: &str) {
        let lab = self.desk().await;
        lab.sign_in(Role::Analyst).await;
        let e = lab.engine();
        let drugs = e.get_run(GetRunInput { run_id: lab.run }).await.unwrap();
        e.close_run(CloseRunInput {
            run_id: lab.run,
            expected_revision: drugs.revision,
            reason: None,
        })
        .await
        .unwrap();
        let controls: Vec<LotView> = e
            .list_lots(ListLotsInput { active_only: true })
            .await
            .unwrap()
            .into_iter()
            .filter(|lot| lot.material_kind == MaterialKind::Control)
            .collect();
        let run = e
            .create_run(CreateRunInput {
                instrument_id: lab.instrument,
                method_version_id: lab.methods.validity,
                lot_ids: controls.iter().map(|lot| lot.lot_id).collect(),
            })
            .await
            .unwrap();
        for (n, lot) in controls.iter().enumerate() {
            e.add_qc_position(AddQcPositionInput {
                run_id: run.run_id,
                position_label: format!("VQ{}", n + 1),
                role: QcRole::Control,
                lot_id: Some(lot.lot_id),
                sample_label: None,
            })
            .await
            .unwrap();
        }
        e.add_patient_position(AddPatientPositionInput {
            run_id: run.run_id,
            position_label: "V1".to_owned(),
            container_label: label.to_owned(),
            attempt_kind: AttemptKind::Initial,
            related_execution_id: None,
            dilution_factor: None,
            prep_batch_id: None,
            reason: None,
        })
        .await
        .unwrap();
        lab.import(&message(&[
            "P|1",
            "O|1|QC-LOW-SYN|SYN-POS-23||R",
            "R|1|^^^SYN-CRE|40.0|mg/dL||N||F||SYN-TECH-01||20261012140000|SYNTH-LCMS-01",
            "P|2",
            &format!("O|1|{label}|SYN-POS-22||R"),
            "R|1|^^^SYN-CRE|120.0|mg/dL||N||F||SYN-TECH-01||20261012140500|SYNTH-LCMS-01",
            "P|3",
            "O|1|QC-HIGH-SYN|SYN-POS-24||R",
            "R|1|^^^SYN-CRE|200.0|mg/dL||N||F||SYN-TECH-01||20261012141000|SYNTH-LCMS-01",
        ]))
        .await;
        accept_run_qc(lab, run.run_id, lab.analytes.cre).await;
    }
}

/// Q accepts the QC of `analyte` on `run` with every current control
/// observation as evidence.
async fn accept_run_qc(lab: &Lab, run_id: RunId, analyte: AnalyteId) {
    lab.sign_in(Role::Quality).await;
    let qc = lab
        .engine()
        .get_run_qc(GetRunQcInput { run_id })
        .await
        .unwrap();
    let entry = qc
        .analytes
        .iter()
        .find(|a| a.analyte_id == analyte)
        .unwrap();
    let evidence: Vec<_> = entry
        .observations
        .iter()
        .filter(|o| !o.superseded)
        .map(|o| o.observation_id)
        .collect();
    assert_eq!(evidence.len(), 2, "both controls observed");
    lab.engine()
        .assess_qc(AssessQcInput {
            scope: QcScope::RunAnalyte,
            run_id: Some(run_id),
            analyte_id: Some(analyte),
            prep_batch_id: None,
            outcome: QcOutcome::Accepted,
            evidence_observation_ids: evidence,
            supersedes_assessment_id: None,
            reason: None,
        })
        .await
        .unwrap();
}

/// Creates R and the race actor as A, then signs each in once; the desk
/// does not know about these sign-ins.
async fn accounts(lab: &Lab) -> (Account, Account) {
    let racer_roles = [
        RoleCode::Releaser,
        RoleCode::QualityManager,
        RoleCode::Accessioner,
        RoleCode::TechnicalReviewer,
        RoleCode::Administrator,
    ];
    lab.sign_in(Role::Admin).await;
    let releaser = lab.t().create_user("rel1", &[RoleCode::Releaser]).await;
    let racer = lab.t().create_user("rel2", &racer_roles).await;
    let account = async |user_id, username| Account {
        user_id,
        username,
        secret: lab.t().first_sign_in(username).await.1,
    };
    (
        account(releaser.user_id, "rel1").await,
        account(racer.user_id, "rel2").await,
    )
}

fn laboratory_profile() -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version: None,
        name: "Synthetic Toxicology Laboratory".to_owned(),
        address_lines: vec!["100 Synthetic Way".to_owned()],
        clia_id: "00D0000000".to_owned(),
        director: "Director Synthetic".to_owned(),
        phone: "555-0100".to_owned(),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}
