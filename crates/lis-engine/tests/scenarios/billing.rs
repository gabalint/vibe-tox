//! Billing scenarios (REQ-BIL-001/002/005, VAL-23; brief §5, §7) on the M2
//! journey laboratory ([`crate::m2_lab`]), plus the billing calls of the
//! registry-wide authorization table. Every sample of the journey is
//! collected on 2026-10-12 for one subject, so those five specimens form
//! one date of service; the scenarios add specimens on other dates. The
//! expected classes come from the mapping and orders entered here, never
//! from the implementation. Synthetic data only.

use std::path::PathBuf;

use lis_analytical::{ExecutionState, RevisionIndicator};
use lis_auth::RoleCode;
use lis_catalog::{MethodCertainty, OrderableVersionId};
use lis_core::{CommandError, DecimalText, ErrorCode, UserId};
use lis_engine::{
    AcceptOrderInput, AmendOrderEvidenceInput, ApproveClassMappingVersion,
    ApproveServiceDerivation, AuditStream, AuthorityEvidenceInput, AuthorityEvidenceKind,
    BillingClassMappingEntry, BillingClassMappingVersionId, BillingComponentDisposition,
    BillingDerivationId, BillingDerivationState, BillingHandoffId, BillingHandoffInput,
    BillingHandoffItemId, BillingWorkFilter, BillingWorkItem, BillingWorkState,
    ConfirmClassMappingVersion, CreateAccessionInput, CreateBillingHandoff, DerivationSpecimenView,
    DerivationView, DeriveServices, DeviationTarget, DiagnosisInput, DraftClassMappingVersion,
    Engine, EvidenceInput, FailExecutionInput, GetClassMappingVersionInput, GetRunInput,
    GetServiceDerivationInput, HandoffItemKind, HandoffOutcome, HandoffStatus, HandoffView,
    ListAuditEventsInput, ListBillingWork, OpenDeviationInput, PointOfCareResultInput,
    PrepareReportCandidateInput, RecordHandoffOutcome, RecordReceiptDecisionInput,
    RecordTriggerNotMetInput, ReleaseReportInput, RetireClassMappingVersion, SetLaboratoryProfile,
    TriggerSource, UpdateClassMappingDraft, WithdrawReportInput,
};
use lis_reporting::{ReportCompleteness, ReportId};
use lis_specimens::{
    CollectionInput, CollectionTimeInput, ReceiptDecision, SpecimenInput, VolumeKind,
};

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_config::URINE;
use crate::m2_lab::{Lab, Role, Sample};
use crate::m2_steps::{JOURNEY, gate_codes, message};

const OPIATES: &str = "SYN-OPIATES";
const OXYCODONE: &str = "SYN-OXYCODONE";
const DIAGNOSIS: &str = "SYN-DX-1";
const PREPRINTED_DIAGNOSIS: &str = "SYN-DX-PREPRINTED";

pub(crate) async fn billing_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = service_call(engine, name).await {
        return Some(result);
    }
    let version_id = BillingClassMappingVersionId::generate();
    Some(match name {
        "list_class_mapping_versions" => engine.list_class_mapping_versions().await.map(drop),
        "get_class_mapping_version" => engine
            .get_class_mapping_version(GetClassMappingVersionInput { version_id })
            .await
            .map(drop),
        "draft_class_mapping_version" => engine
            .draft_class_mapping_version(DraftClassMappingVersion {
                entries: vec![entry(lis_engine::AnalyteId::generate(), "SYN-CLASS")],
            })
            .await
            .map(drop),
        "update_class_mapping_draft" => engine
            .update_class_mapping_draft(UpdateClassMappingDraft {
                version_id,
                expected_revision: 1,
                entries: vec![entry(lis_engine::AnalyteId::generate(), "SYN-CLASS")],
            })
            .await
            .map(drop),
        "approve_class_mapping_version" => engine
            .approve_class_mapping_version(ApproveClassMappingVersion {
                version_id,
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "confirm_class_mapping_version" => engine
            .confirm_class_mapping_version(ConfirmClassMappingVersion {
                version_id,
                expected_revision: 2,
            })
            .await
            .map(drop),
        "retire_class_mapping_version" => engine
            .retire_class_mapping_version(RetireClassMappingVersion {
                version_id,
                expected_revision: 3,
                reason: "Synthetic authorization test".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// Derivation and handoff calls for the authorization table.
async fn service_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let derivation_id = BillingDerivationId::generate();
    let handoff_id = BillingHandoffId::generate();
    Some(match name {
        "list_billing_work" => engine
            .list_billing_work(ListBillingWork {
                after: None,
                target: None,
                filter: BillingWorkFilter::Held,
                limit: 10,
            })
            .await
            .map(drop),
        "derive_services" => engine
            .derive_services(DeriveServices {
                subject_id: lis_identity::SubjectId::generate(),
                date_of_service: None,
            })
            .await
            .map(drop),
        "get_service_derivation" => engine
            .get_service_derivation(GetServiceDerivationInput { derivation_id })
            .await
            .map(drop),
        "approve_service_derivation" => engine
            .approve_service_derivation(ApproveServiceDerivation {
                derivation_id,
                expected_revision: 1,
            })
            .await
            .map(drop),
        "create_billing_handoff" => engine
            .create_billing_handoff(CreateBillingHandoff {
                derivation_ids: vec![derivation_id],
            })
            .await
            .map(drop),
        "get_billing_handoff" => engine
            .get_billing_handoff(BillingHandoffInput { handoff_id })
            .await
            .map(drop),
        "export_billing_handoff" => engine
            .export_billing_handoff(BillingHandoffInput { handoff_id }, None)
            .await
            .map(drop),
        "record_handoff_outcome" => engine
            .record_handoff_outcome(RecordHandoffOutcome {
                item_id: BillingHandoffItemId::generate(),
                outcome: HandoffOutcome::Acknowledged,
                external_reference: None,
                reason: None,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn entry(analyte_id: lis_engine::AnalyteId, class_code: &str) -> BillingClassMappingEntry {
    BillingClassMappingEntry {
        analyte_id,
        class_code: class_code.to_owned(),
        class_label: format!("Synthetic {class_code}"),
    }
}

/// Someone signed in outside the journey's roles.
#[derive(Clone, Copy)]
enum Staff {
    Releaser,
    Author,
    Approver,
}

/// The journey laboratory with a laboratory profile that has a billing NPI
/// and TIN, a releaser and two billing users.
struct Desk {
    lab: Lab,
    secrets: [String; 3],
    profile_version: std::cell::Cell<i64>,
}

// Version 1 is established before runs; subsequent updates keep their original sequence.
fn laboratory_profile(current: i64) -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version: (current > 0).then_some(current),
        name: "Synthetic Toxicology Laboratory".to_owned(),
        address_lines: vec!["100 Synthetic Way".to_owned()],
        clia_id: "00D0000000".to_owned(),
        director: "Director Synthetic".to_owned(),
        phone: format!("555-01{current:02}"),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: Some("1234567893".to_owned()),
        tax_id: Some("123456789".to_owned()),
        reason: (current > 0).then(|| "Synthetic profile change".to_owned()),
    }
}

impl Desk {
    async fn start() -> Self {
        let lab = Lab::start_as_with_profile(
            RevisionIndicator::None,
            lis_engine::InstallationKind::Laboratory,
            Some(laboratory_profile(0)),
        )
        .await;
        lab.sign_in(Role::Admin).await;
        let accounts = [
            ("rel1", RoleCode::Releaser),
            ("bil1", RoleCode::Billing),
            ("bil2", RoleCode::Billing),
        ];
        for (username, role) in accounts {
            lab.t().create_user(username, &[role]).await;
        }
        let mut secrets = Vec::new();
        for (username, _) in accounts {
            secrets.push(lab.t().first_sign_in(username).await.1);
        }
        let desk = Self {
            lab,
            secrets: secrets.try_into().unwrap(),
            profile_version: std::cell::Cell::new(1),
        };
        desk.resync().await;
        desk.lab.sign_in(Role::Accessioner).await;
        for sample in &desk.lab.samples {
            crate::billing_financial::confirm_client(desk.engine(), sample.order_id).await;
        }
        desk
    }

    fn engine(&self) -> &Engine {
        self.lab.engine()
    }

    /// Signs `staff` in; [`Self::resync`] must follow before a journey step.
    async fn as_staff(&self, staff: Staff) -> &Engine {
        let (username, index) = match staff {
            Staff::Releaser => ("rel1", 0),
            Staff::Author => ("bil1", 1),
            Staff::Approver => ("bil2", 2),
        };
        self.lab
            .t()
            .sign_in_with(username, NEW_PASSWORD, &self.secrets[index])
            .await;
        self.engine()
    }

    /// Makes the signed-in user and the lab's record of it agree again.
    async fn resync(&self) {
        self.lab.sign_in(Role::Analyst).await;
        self.lab.sign_in(Role::Quality).await;
    }

    /// The next laboratory profile version, with billing NPI and TIN.
    async fn set_profile(&self) {
        self.lab.sign_in(Role::Admin).await;
        let current = self.profile_version.get();
        self.engine()
            .set_laboratory_profile(laboratory_profile(current))
            .await
            .unwrap();
        self.profile_version.set(current + 1);
    }

    /// An accepted order for one orderable, accessioned with `time` and
    /// accepted at receipt; placed on the run when `place`.
    async fn add_sample(
        &mut self,
        orderable_version_id: OrderableVersionId,
        time: CollectionTimeInput,
        place: bool,
    ) -> usize {
        self.lab.sign_in(Role::Accessioner).await;
        let e = self.engine();
        let order = e
            .register_order(self.lab.order_input(orderable_version_id, None))
            .await
            .unwrap();
        let accepted = e
            .accept_order(AcceptOrderInput {
                order_id: order.order_id,
                expected_revision: order.revision,
            })
            .await
            .unwrap();
        crate::billing_financial::confirm_client(e, accepted.order_id).await;
        let item_id = accepted.items[0].item_id;
        let accession = e
            .create_accession(CreateAccessionInput {
                order_id: accepted.order_id,
                order_item_ids: vec![item_id],
                collection: CollectionInput {
                    time,
                    collector: Some("Synthetic Collector".to_owned()),
                    observations: Vec::new(),
                },
                specimens: vec![SpecimenInput {
                    specimen_type_code: URINE.to_owned(),
                    container_count: 1,
                    volume: Some(DecimalText::parse("30").unwrap()),
                    volume_unit: Some("mL".to_owned()),
                    volume_kind: VolumeKind::Estimated,
                }],
            })
            .await
            .unwrap();
        let specimen = &accession.specimens[0];
        e.record_receipt_decision(RecordReceiptDecisionInput {
            specimen_id: specimen.specimen_id,
            expected_revision: specimen.revision,
            decision: ReceiptDecision::Accepted,
            discrepancies: Vec::new(),
            policy_reference: None,
            scope: None,
            reason: None,
        })
        .await
        .unwrap();
        let sample = Sample {
            order_id: accepted.order_id,
            item_id,
            specimen_id: specimen.specimen_id,
            container_id: specimen.containers[0].container_id,
            label_code: specimen.containers[0].label_code.clone(),
        };
        if place {
            self.lab.place(&sample).await;
        }
        self.lab.samples.push(sample);
        self.lab.samples.len() - 1
    }

    /// Imports morphine at 75.0 (meets the cutoff) and oxycodone at 12.0
    /// (does not) for sample `index`.
    async fn import_drugs(&self, index: usize) {
        let label = &self.lab.samples[index].label_code;
        self.lab
            .import(&message(&[
                "P|1",
                &format!("O|1|{label}|SYN-POS-31||R"),
                "R|1|^^^SYN-MOR|75.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
                "R|2|^^^SYN-OXY|12.0|ng/mL||N||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
            ]))
            .await;
    }

    /// Drafted by one billing user, approved by the other, confirmed by Q:
    /// morphine and oxycodone in their own classes, creatinine in a third.
    async fn confirm_mapping(&self) {
        let analytes = &self.lab.analytes;
        let draft = self
            .as_staff(Staff::Author)
            .await
            .draft_class_mapping_version(DraftClassMappingVersion {
                entries: vec![
                    entry(analytes.mor, OPIATES),
                    entry(analytes.oxy, OXYCODONE),
                    entry(analytes.cre, "SYN-CREATININE"),
                ],
            })
            .await
            .unwrap();
        assert_eq!(
            draft.allowed_actions,
            [
                "update_class_mapping_draft",
                "approve_class_mapping_version"
            ]
        );
        assert_eq!(
            draft.summary.created_by_display.as_deref(),
            Some("Test bil1")
        );
        let version_id = draft.summary.version_id;
        let approved = self
            .as_staff(Staff::Approver)
            .await
            .approve_class_mapping_version(ApproveClassMappingVersion {
                version_id,
                expected_revision: draft.summary.revision,
                self_approval: false,
            })
            .await
            .unwrap();
        assert_eq!(
            (
                approved.summary.created_by_display.as_deref(),
                approved.summary.approved_by_display.as_deref(),
            ),
            (Some("Test bil1"), Some("Test bil2"))
        );
        self.resync().await;
        self.engine()
            .confirm_class_mapping_version(ConfirmClassMappingVersion {
                version_id,
                expected_revision: approved.summary.revision,
            })
            .await
            .unwrap();
    }

    /// The order supplies a point-of-care result (and a diagnosis) on which
    /// the reflex condition is not met, so the report can become final.
    async fn condition_not_met(&self, index: usize) {
        self.lab.sign_in(Role::Accessioner).await;
        let sample = &self.lab.samples[index];
        let e = self.engine();
        let order = e
            .get_order(lis_engine::GetOrderInput {
                order_id: sample.order_id,
            })
            .await
            .unwrap();
        let amended = e
            .amend_order_evidence(AmendOrderEvidenceInput {
                order_id: sample.order_id,
                expected_revision: order.revision,
                add: EvidenceInput {
                    authority: AuthorityEvidenceInput {
                        kind: AuthorityEvidenceKind::WetSignature,
                        document_ref: Some("SYN-REQ-BIL".to_owned()),
                    },
                    diagnoses: vec![
                        DiagnosisInput {
                            code: DIAGNOSIS.to_owned(),
                            code_system: "SYN-DX".to_owned(),
                            code_system_version: None,
                            document_ref: "SYN-CLINIC-REQ".to_owned(),
                            preprinted_on_lab_form: false,
                        },
                        DiagnosisInput {
                            code: PREPRINTED_DIAGNOSIS.to_owned(),
                            code_system: "SYN-DX".to_owned(),
                            code_system_version: Some("2026".to_owned()),
                            document_ref: "SYN-LAB-FORM".to_owned(),
                            preprinted_on_lab_form: true,
                        },
                    ],
                    medications: Vec::new(),
                    point_of_care_results: vec![PointOfCareResultInput {
                        device_or_test: "Synthetic cup".to_owned(),
                        analyte_text: "Synthetic opiates".to_owned(),
                        result_text: "Synthetic not detected".to_owned(),
                        performed_at_text: None,
                        source: "Synthetic clinic".to_owned(),
                    }],
                },
                remove_evidence_ids: Vec::new(),
                reason: "Synthetic requisition details received".to_owned(),
            })
            .await
            .unwrap();
        e.record_trigger_not_met(RecordTriggerNotMetInput {
            parent_item_id: sample.item_id,
            orderable_id: self.lab.orderables.confirmation.0,
            trigger_source: TriggerSource::ClientPointOfCare,
            trigger_ref: amended.evidence.point_of_care_results[0]
                .evidence_id
                .to_string(),
            reason: "Synthetic condition not met".to_owned(),
        })
        .await
        .unwrap();
    }

    /// Prepares and releases sample `index`'s final report as R.
    async fn release_final(&self, index: usize) -> ReportId {
        let e = self.as_staff(Staff::Releaser).await;
        let candidate = e
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id: self.lab.samples[index].specimen_id,
                amendment_reason: None,
            })
            .await
            .unwrap();
        assert_eq!(candidate.completeness, ReportCompleteness::Final);
        e.release_report(ReleaseReportInput {
            candidate_id: candidate.candidate_id,
            expected_revision: candidate.revision,
            content_sha256: candidate.content_sha256.clone(),
            acknowledge_partial: false,
            password: secret(NEW_PASSWORD).into(),
            self_approval: false,
        })
        .await
        .unwrap();
        self.resync().await;
        candidate.report_id
    }

    async fn withdraw(&self, report_id: ReportId) {
        self.as_staff(Staff::Releaser)
            .await
            .withdraw_report(WithdrawReportInput {
                report_id,
                expected_revision_no: 1,
                reason: "Synthetic report withdrawal".to_owned(),
                password: secret(NEW_PASSWORD).into(),
            })
            .await
            .unwrap();
        self.resync().await;
    }

    async fn derive(&self, date_of_service: Option<&str>) -> DerivationView {
        self.as_staff(Staff::Author)
            .await
            .derive_services(DeriveServices {
                subject_id: self.lab.parties.subject,
                date_of_service: date_of_service.map(str::to_owned),
            })
            .await
            .unwrap()
    }

    async fn derivation(&self, view: &DerivationView) -> DerivationView {
        self.as_staff(Staff::Author)
            .await
            .get_service_derivation(GetServiceDerivationInput {
                derivation_id: view.derivation_id,
            })
            .await
            .unwrap()
    }

    async fn approve(&self, view: &DerivationView) -> Result<DerivationView, CommandError> {
        self.as_staff(Staff::Approver)
            .await
            .approve_service_derivation(ApproveServiceDerivation {
                derivation_id: view.derivation_id,
                expected_revision: view.revision,
            })
            .await
    }

    async fn hand_off(&self, views: &[&DerivationView]) -> Result<HandoffView, CommandError> {
        self.as_staff(Staff::Author)
            .await
            .create_billing_handoff(CreateBillingHandoff {
                derivation_ids: views.iter().map(|view| view.derivation_id).collect(),
            })
            .await
    }

    async fn work(&self, filter: BillingWorkFilter) -> Vec<BillingWorkItem> {
        self.as_staff(Staff::Author)
            .await
            .list_billing_work(ListBillingWork {
                after: None,
                target: None,
                filter,
                limit: 50,
            })
            .await
            .unwrap()
    }

    fn count(&self, sql: &str) -> i64 {
        self.lab
            .t()
            .conn()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }
}

/// `(class code, ordered, performed, reported)` of one specimen of a view.
fn classes(view: &DerivationView, accession: &str) -> Vec<(String, bool, bool, bool)> {
    view.specimens
        .iter()
        .find(|s| s.accession_number == accession)
        .unwrap()
        .classes
        .iter()
        .map(|c| (c.class_code.clone(), c.ordered, c.performed, c.reported))
        .collect()
}

fn flag_codes(view: &DerivationView) -> Vec<&str> {
    view.flags.iter().map(|flag| flag.code.as_str()).collect()
}

fn class(code: &str, performed: bool, reported: bool) -> (String, bool, bool, bool) {
    (code.to_owned(), true, performed, reported)
}

fn accession_of(desk: &Desk, view: &DerivationView, index: usize) -> String {
    let specimen_id = desk.lab.samples[index].specimen_id;
    view.specimens
        .iter()
        .find(|s| s.specimen_id == specimen_id)
        .unwrap()
        .accession_number
        .clone()
}

/// A specimen collected at 23:30 at UTC-04:00 on 2026-10-11 (03:30 UTC on
/// the 12th): its own date of service, 2026-10-11.
fn late_evening() -> CollectionTimeInput {
    CollectionTimeInput::Datetime {
        local: "2026-10-11T23:30".to_owned(),
        utc_offset_minutes: -240,
    }
}

/// The full path: mapping, final release, derive (twice), approve, hand
/// off, export, outcome; then a change after handoff makes the derivation
/// stale and every later derivation is an adjustment (REQ-BIL-001/002/005).
#[tokio::test]
async fn a_final_report_becomes_an_acknowledged_handoff_and_later_changes_adjust_it() {
    let mut desk = Desk::start().await;
    let s = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(s).await;
    desk.lab.accept_qc().await;
    desk.lab.approve_all(s).await.unwrap();

    // No mapping and no final report yet: held, and approval names the flags.
    let held = desk.derive(Some("2026-10-11")).await;
    assert_eq!(held.state, BillingDerivationState::Held);
    assert!(flag_codes(&held).contains(&"mapping_missing"));
    assert!(flag_codes(&held).contains(&"report_not_final"));
    let refused = desk.approve(&held).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&refused).contains(&"mapping_missing".to_owned()));

    desk.resync().await;
    desk.confirm_mapping().await;
    desk.condition_not_met(s).await;
    let report_id = desk.release_final(s).await;
    let needs = desk.work(BillingWorkFilter::NeedsDerivation).await;
    let accession = needs
        .iter()
        .find(|item| item.date_of_service.as_deref() == Some("2026-10-11"))
        .unwrap()
        .accession_numbers
        .clone();

    let first = desk.derive(Some("2026-10-11")).await;
    assert_candidate(&desk, &first, &accession[0], &held).await;
    let sealed: bool = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT facts_sealed FROM bil_derivations WHERE id = ?1",
            [first.derivation_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(sealed);
    let late_evidence = desk.lab.t().conn().execute(
        "INSERT INTO bil_derivation_flags (derivation_id, code, ref_id, blocking) \
         VALUES (?1, 'ordered_not_performed', 'late-fact', 0)",
        [first.derivation_id],
    );
    assert!(late_evidence.is_err());
    // VAL-23: the same inputs give the same derivation and no new event.
    let derive_events = || {
        desk.count("SELECT count(*) FROM audit_events WHERE action = 'billing.derive' AND stream = 'clinical'")
    };
    let events = derive_events();
    let again = desk.derive(Some("2026-10-11")).await;
    assert_eq!(again.derivation_id, first.derivation_id);
    assert_eq!(derive_events(), events);
    assert_eq!(desk.count("SELECT count(*) FROM bil_derivations"), 2);

    let behind = DerivationView {
        revision: first.revision + 1,
        ..first.clone()
    };
    assert_eq!(
        desk.approve(&behind).await.unwrap_err().code(),
        ErrorCode::Conflict
    );
    let approved = desk.approve(&first).await.unwrap();
    assert_eq!(approved.state, BillingDerivationState::Approved);
    assert_eq!(approved.allowed_actions, ["create_billing_handoff"]);
    let handoff = desk.hand_off(&[&approved]).await.unwrap();
    assert_new_handoff(&handoff);
    let refused = desk.hand_off(&[&approved]).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    assert_export(&desk, &handoff, &accession[0]).await;
    record_outcomes(&desk, &handoff).await;

    adjust_after_changes(&desk, &approved, &handoff).await;
    desk.withdraw(report_id).await;
    let withdrawn = desk.work(BillingWorkFilter::NeedsDerivation).await;
    assert!(withdrawn.iter().any(|item| {
        item.date_of_service.as_deref() == Some("2026-10-11")
            && item.state == BillingWorkState::NeedsDerivation
            && item.allowed_actions == ["derive_services"]
    }));
    assert_audit_record_labels(&desk, s, report_id, &approved, &handoff, &accession[0]).await;
}

async fn assert_audit_record_labels(
    desk: &Desk,
    sample: usize,
    report_id: ReportId,
    derivation: &DerivationView,
    handoff: &HandoffView,
    accession_number: &str,
) {
    desk.lab.sign_in(Role::Admin).await;
    desk.lab
        .t()
        .create_user("auditor-billing", &[RoleCode::Auditor])
        .await;
    desk.lab.t().first_sign_in("auditor-billing").await;
    let revision_id: lis_reporting::ReportRevisionId = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT id FROM rpt_revisions WHERE report_id = ?1 AND revision_no = 1",
            [report_id],
            |row| row.get(0),
        )
        .unwrap();
    let report_number: String = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT report_number FROM rpt_reports WHERE id = ?1",
            [report_id],
            |row| row.get(0),
        )
        .unwrap();
    for (record_type, record_id, expected) in [
        (
            "specimen",
            desk.lab.samples[sample].specimen_id.to_string(),
            accession_number.to_owned(),
        ),
        (
            "report_revision",
            revision_id.to_string(),
            format!("{report_number} revision 1"),
        ),
        (
            "billing_derivation",
            derivation.derivation_id.to_string(),
            format!("Derivation {}", derivation.derivation_no),
        ),
        (
            "billing_handoff",
            handoff.handoff_id.to_string(),
            format!("Handoff {}", handoff.handoff_no),
        ),
    ] {
        let events = desk
            .engine()
            .list_audit_events(ListAuditEventsInput {
                stream: AuditStream::Clinical,
                before_seq: None,
                limit: 500,
                action: None,
                record_type: Some(record_type.to_owned()),
                record_id: Some(record_id),
                self_approvals_only: None,
            })
            .await
            .unwrap();
        assert!(!events.is_empty(), "missing {record_type} audit event");
        assert!(
            events
                .iter()
                .all(|event| event.record_label.as_deref() == Some(expected.as_str())),
            "wrong {record_type} audit label"
        );
    }
}

/// Positivity never counts: oxycodone at 12.0 (under the cutoff) is as
/// performed as morphine at 75.0; one class each, ordered, performed and
/// reported, and the held predecessor is superseded.
async fn assert_candidate(
    desk: &Desk,
    view: &DerivationView,
    accession: &str,
    held: &DerivationView,
) {
    assert_eq!(view.state, BillingDerivationState::Candidate);
    assert_eq!(view.derivation_no, 2);
    assert!(view.flags.is_empty(), "{:?}", view.flags);
    assert_eq!(
        classes(view, accession),
        [class(OPIATES, true, true), class(OXYCODONE, true, true)]
    );
    let specimen = &view.specimens[0];
    assert!(
        specimen
            .classes
            .iter()
            .all(|c| c.certainty == MethodCertainty::Definitive)
    );
    assert_eq!(
        specimen.components[0].disposition,
        BillingComponentDisposition::Performed
    );
    assert!(view.adjustment_of.is_none());
    assert_eq!(view.allowed_actions, ["approve_service_derivation"]);
    let superseded = desk.derivation(held).await;
    assert_eq!(superseded.state, BillingDerivationState::Superseded);
    let ready = desk.work(BillingWorkFilter::Ready).await;
    assert!(
        ready
            .iter()
            .any(|item| item.derivation_id == Some(view.derivation_id))
    );
}

/// The file in Downloads is labelled a billing handoff, carries the
/// diagnosis exactly as supplied and the classes derived; the export is a
/// security event, and a second export never overwrites the first.
async fn assert_export(desk: &Desk, handoff: &HandoffView, accession: &str) {
    let downloads = tempfile::tempdir().unwrap();
    let e = desk.as_staff(Staff::Author).await;
    let input = BillingHandoffInput {
        handoff_id: handoff.handoff_id,
    };
    let mut names = Vec::new();
    for _ in 0..2 {
        let exported = e
            .export_billing_handoff(input, Some(downloads.path().to_path_buf()))
            .await
            .unwrap();
        assert_eq!(exported.folder, "downloads");
        names.push(exported.file_name);
    }
    let stem = format!("billing-handoff-{}", handoff.handoff_id);
    assert_eq!(names, [format!("{stem}.json"), format!("{stem}-1.json")]);
    // Each export commits its intent before the file is published, then records it.
    let events = |record_type: &str| {
        format!(
            "SELECT count(*) FROM audit_events WHERE stream = 'security' \
             AND action = 'billing.export_handoff' AND record_type = '{record_type}'"
        )
    };
    assert_eq!(desk.count(&events("export_intent")), 2);
    assert_eq!(desk.count(&events("billing_handoff")), 2);
    let path: PathBuf = downloads.path().join(&names[0]);
    assert_file(&std::fs::read(path).unwrap(), accession);
}

fn assert_file(bytes: &[u8], accession: &str) {
    assert!(
        !String::from_utf8_lossy(bytes).contains("75.0"),
        "no result values"
    );
    let file: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(file["format"], "bil-handoff-v2");
    assert_eq!(file["laboratory"]["billingNpi"], "1234567893");
    let item = &file["items"][0];
    assert_eq!(item["kind"], "original");
    assert_eq!(item["dateOfService"], "2026-10-11");
    assert_eq!(item["subject"]["familyName"], "SYNTHETIC");
    let specimen = &item["specimens"][0];
    assert_eq!(specimen["accessionNumber"], accession);
    assert_eq!(specimen["practitionerNpi"], "1234567893");
    let diagnoses = specimen["diagnoses"].as_array().unwrap();
    assert_eq!(diagnoses.len(), 2);
    assert_diagnosis(diagnoses, DIAGNOSIS, "SYN-CLINIC-REQ", false);
    assert_diagnosis(diagnoses, PREPRINTED_DIAGNOSIS, "SYN-LAB-FORM", true);
    assert_eq!(specimen["classes"].as_array().unwrap().len(), 2);
}

/// A supplied diagnosis keeps its provenance: evidence, document and whether the
/// laboratory's own form preprinted it.
fn assert_diagnosis(diagnoses: &[serde_json::Value], code: &str, document: &str, preprinted: bool) {
    let diagnosis = diagnoses
        .iter()
        .find(|diagnosis| diagnosis["code"] == code)
        .unwrap();
    assert!(diagnosis["evidenceId"].as_str().is_some());
    assert_eq!(diagnosis["documentRef"], document);
    assert_eq!(diagnosis["preprintedOnLabForm"], preprinted);
}

/// Changes the laboratory profile, which makes `derivation` stale: it leaves the
/// ready work and comes back as derivation work.
async fn change_profile_and_expect_rework(desk: &Desk, derivation: BillingDerivationId) {
    desk.resync().await;
    desk.set_profile().await;
    assert!(desk.work(BillingWorkFilter::Ready).await.is_empty());
    let rework = desk.work(BillingWorkFilter::NeedsDerivation).await;
    assert!(
        rework
            .iter()
            .any(|item| item.derivation_id == Some(derivation))
    );
}

/// A rejection needs a reason; one outcome per item.
async fn record_outcomes(desk: &Desk, handoff: &HandoffView) {
    let e = desk.as_staff(Staff::Author).await;
    let outcome = |outcome, reason: Option<&str>| RecordHandoffOutcome {
        item_id: handoff.items[0].item_id,
        outcome,
        external_reference: Some("SYN-CLAIM-1".to_owned()),
        reason: reason.map(str::to_owned),
    };
    let refused = e
        .record_handoff_outcome(outcome(HandoffOutcome::Rejected, None))
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    let outstanding = desk.work(BillingWorkFilter::Outstanding).await;
    assert_eq!(outstanding[0].item_id, Some(handoff.items[0].item_id));
    let e = desk.as_staff(Staff::Author).await;
    let rejected = e
        .record_handoff_outcome(outcome(
            HandoffOutcome::Rejected,
            Some("Synthetic missing external account reference"),
        ))
        .await
        .unwrap();
    assert_eq!(rejected.items[0].status, HandoffStatus::Rejected);
    let recorded = acknowledge_rejection(e, handoff.items[0].item_id).await;
    assert_acknowledged(&recorded);
    let again = e
        .record_handoff_outcome(outcome(HandoffOutcome::Acknowledged, None))
        .await
        .unwrap_err();
    assert_eq!(again.code(), ErrorCode::Conflict);
    assert!(desk.work(BillingWorkFilter::Outstanding).await.is_empty());
}

fn assert_acknowledged(recorded: &HandoffView) {
    assert_eq!(recorded.items[0].status, HandoffStatus::Acknowledged);
    assert_eq!(
        recorded.items[0]
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.recorded_by_display.as_deref()),
        Some("Test bil1")
    );
    assert!(recorded.items[0].allowed_actions.is_empty());
}

async fn acknowledge_rejection(e: &Engine, item_id: BillingHandoffItemId) -> HandoffView {
    let followup = || lis_billing::RecordHandoffFollowupInput {
        item_id,
        expected_revision: 1,
        outcome: HandoffOutcome::Acknowledged,
        external_reference: Some("SYN-CLAIM-1".into()),
        reason: "Synthetic billing owner accepted the corrected routing".into(),
    };
    let recorded = e.record_handoff_followup(followup()).await.unwrap();
    assert_eq!(recorded.items[0].outcome_revision, Some(2));
    assert_eq!(recorded.items[0].outcome_history.as_ref().unwrap().len(), 2);
    assert_eq!(
        e.record_handoff_followup(followup())
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    recorded
}

fn assert_new_handoff(handoff: &HandoffView) {
    assert_eq!(handoff.items.len(), 1);
    assert_eq!(handoff.handoff_no, 1);
    assert_eq!(handoff.created_by_display.as_deref(), Some("Test bil1"));
    assert_eq!(handoff.items[0].kind, HandoffItemKind::Original);
    assert_eq!(handoff.items[0].status, HandoffStatus::Outstanding);
}

/// Each profile change makes the newest derivation stale: approval and
/// handoff refuse it, and the next derivation still adjusts the newest
/// actually handed-off one; its handoff item names the same lineage.
async fn adjust_after_changes(desk: &Desk, original: &DerivationView, first: &HandoffView) {
    change_profile_and_expect_rework(desk, original.derivation_id).await;
    let stale = desk.derivation(original).await;
    assert_eq!(
        (stale.stale, stale.allowed_actions),
        (true, vec!["derive_services".to_owned()])
    );
    let approved_stale = refuse_stale(desk, original).await;
    let adjustment = desk.derive(Some("2026-10-11")).await;
    assert_eq!(adjusted(&adjustment), Some(original.derivation_id));
    let superseded = desk.derivation(&approved_stale).await;
    assert_eq!(superseded.state, BillingDerivationState::Superseded);
    let adjustment = desk.approve(&adjustment).await.unwrap();
    let handoff = desk.hand_off(&[&adjustment]).await.unwrap();
    let outstanding = desk.work(BillingWorkFilter::Outstanding).await;
    assert_adjustment_handoff(&handoff, first, &outstanding);
    let original = desk.derivation(original).await;
    assert_eq!(original.state, BillingDerivationState::HandedOff);
}

fn assert_adjustment_handoff(handoff: &HandoffView, first: &HandoffView, work: &[BillingWorkItem]) {
    assert_eq!(handoff.handoff_no, first.handoff_no + 1);
    assert!(work.iter().any(|item| {
        item.handoff_id == Some(handoff.handoff_id) && item.handoff_no == Some(handoff.handoff_no)
    }));
    let item = &handoff.items[0];
    assert_eq!(
        (item.kind, item.adjusts_item_id),
        (HandoffItemKind::Adjustment, Some(first.items[0].item_id))
    );
}

/// A final report is frozen evidence even when a later deviation makes its
/// technical review stale. The persisted class stays reported, is not
/// performed, and carries review work instead of silently becoming billable.
#[tokio::test]
async fn stale_review_preserves_reported_membership_without_performed_service() {
    let mut desk = Desk::start().await;
    let sample = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(sample).await;
    desk.lab.accept_qc().await;
    desk.lab.approve_all(sample).await.unwrap();
    desk.confirm_mapping().await;
    desk.condition_not_met(sample).await;
    desk.release_final(sample).await;

    desk.lab.sign_in(Role::Quality).await;
    desk.engine()
        .open_deviation(OpenDeviationInput {
            scope: DeviationTarget::Run(desk.lab.run),
            analyte_id: None,
            description: "Synthetic run-wide review invalidation".to_owned(),
            reason: "Synthetic post-release investigation".to_owned(),
        })
        .await
        .unwrap();
    let derived = desk.derive(Some("2026-10-11")).await;
    let accession = accession_of(&desk, &derived, sample);
    assert_eq!(
        classes(&derived, &accession),
        [class(OPIATES, false, true), class(OXYCODONE, false, true)]
    );
    assert!(flag_codes(&derived).contains(&"ordered_not_performed"));
    let persisted: Vec<(bool, bool)> = desk
        .lab
        .t()
        .conn()
        .prepare(
            "SELECT performed, reported FROM bil_derivation_classes \
             WHERE derivation_id = ?1 ORDER BY class_code",
        )
        .unwrap()
        .query_map([derived.derivation_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(persisted, [(false, true), (false, true)]);
}

/// The queue limit bounds subject/date groups, not individual reports: every
/// accession of the selected same-day group remains visible.
#[tokio::test]
async fn billing_work_limit_keeps_every_accession_in_the_selected_group() {
    let mut desk = Desk::start().await;
    let first = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    let second = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(first).await;
    desk.import_drugs(second).await;
    desk.lab.accept_qc().await;
    desk.lab.approve_all(first).await.unwrap();
    desk.lab.approve_all(second).await.unwrap();
    desk.condition_not_met(first).await;
    desk.condition_not_met(second).await;
    desk.release_final(first).await;
    desk.release_final(second).await;

    let work = desk
        .as_staff(Staff::Author)
        .await
        .list_billing_work(ListBillingWork {
            after: None,
            target: None,
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].accession_numbers.len(), 2);
    assert_ne!(work[0].accession_numbers[0], work[0].accession_numbers[1]);
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events WHERE stream = 'access' \
             AND action = 'billing.list_billing_work'"
        ),
        1
    );
}

#[tokio::test]
async fn billing_work_marks_a_capped_answer() {
    let mut desk = Desk::start().await;
    let first = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    let second = desk
        .add_sample(
            desk.lab.orderables.panel.1,
            CollectionTimeInput::Date {
                date: "2026-10-12".to_owned(),
            },
            true,
        )
        .await;
    desk.lab.import(JOURNEY).await;
    desk.import_drugs(first).await;
    desk.import_drugs(second).await;
    desk.lab.accept_qc().await;
    for index in [first, second] {
        desk.lab.approve_all(index).await.unwrap();
        desk.condition_not_met(index).await;
        desk.release_final(index).await;
    }

    let work = desk
        .as_staff(Staff::Author)
        .await
        .list_billing_work(ListBillingWork {
            after: None,
            target: None,
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].has_more, Some(true));
}

/// A candidate gone stale cannot be approved, and an approved derivation
/// gone stale cannot be handed off; later derivations adjust the handed-off
/// original, never the superseded approval.
/// Returns the stale approved one.
async fn refuse_stale(desk: &Desk, original: &DerivationView) -> DerivationView {
    let candidate = desk.derive(Some("2026-10-11")).await;
    assert_eq!(adjusted(&candidate), Some(original.derivation_id));
    change_profile_and_expect_rework(desk, candidate.derivation_id).await;
    let refused = desk.approve(&candidate).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    let approved = desk.derive(Some("2026-10-11")).await;
    assert_eq!(adjusted(&approved), Some(original.derivation_id));
    let approved = desk.approve(&approved).await.unwrap();
    change_profile_and_expect_rework(desk, approved.derivation_id).await;
    let refused = desk.hand_off(&[&approved]).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    approved
}

fn adjusted(view: &DerivationView) -> Option<BillingDerivationId> {
    view.adjustment_of
        .as_ref()
        .map(|original| original.derivation_id)
}

/// Unknown collection dates, reports not final, validity work, results
/// outside the order and failed-only attempts: held derivations with their
/// flags, nothing counted that was not performed and ordered; held
/// derivations cannot be approved or handed off; billing needs `billing`.
#[tokio::test]
async fn held_derivations_flag_unbilled_work_and_count_only_ordered_performed_classes() {
    let mut desk = Desk::start().await;
    desk.confirm_mapping().await;
    let validity = desk
        .add_sample(
            desk.lab.orderables.with_validity.1,
            CollectionTimeInput::Unknown,
            false,
        )
        .await;
    let confirmation = desk
        .add_sample(
            desk.lab.orderables.confirmation.1,
            CollectionTimeInput::Unknown,
            true,
        )
        .await;
    desk.lab.import(JOURNEY).await;
    // The confirmation orders morphine only: oxycodone is quarantined.
    desk.import_drugs(confirmation).await;
    fail_execution(&desk, 3).await;

    let unknown = desk.derive(None).await;
    assert_eq!(unknown.state, BillingDerivationState::Held);
    assert_eq!(unknown.date_of_service, None);
    let codes = flag_codes(&unknown);
    for code in [
        "date_of_service_unknown",
        "report_not_final",
        "performed_not_ordered",
    ] {
        assert!(codes.contains(&code), "{codes:?}");
    }
    let validity_accession = accession_of(&desk, &unknown, validity);
    let validity_specimen = unknown
        .specimens
        .iter()
        .find(|s| s.accession_number == validity_accession)
        .unwrap();
    assert!(
        validity_specimen
            .components
            .iter()
            .any(|c| c.disposition == BillingComponentDisposition::ValidityExcluded)
    );
    assert_eq!(
        classes(&unknown, &validity_accession),
        [class(OPIATES, false, false), class(OXYCODONE, false, false)]
    );
    let confirmation_accession = accession_of(&desk, &unknown, confirmation);
    assert_eq!(
        classes(&unknown, &confirmation_accession),
        [class(OPIATES, false, false)]
    );

    let journey = desk.derive(Some("2026-10-12")).await;
    let failed = desk.lab.samples[3].specimen_id;
    let failed_specimen = journey
        .specimens
        .iter()
        .find(|s| s.specimen_id == failed)
        .unwrap();
    assert_eq!(
        failed_specimen.components[0].disposition,
        BillingComponentDisposition::FailedOnly
    );
    assert!(failed_specimen.classes.iter().all(|c| !c.performed));
    assert!(
        journey
            .flags
            .iter()
            .any(|flag| flag.code == "failed_attempt_only"
                && flag.ref_id == failed_specimen.components[0].component_id.to_string())
    );
    assert_flag_reference_label(&journey, failed_specimen);

    let refused = desk.approve(&journey).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&refused).contains(&"report_not_final".to_owned()));
    let refused = desk.hand_off(&[&journey]).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    let held = desk.work(BillingWorkFilter::Held).await;
    assert_eq!(held.len(), 2);
    assert!(held.iter().all(|item| item.state == BillingWorkState::Held));

    refuse_malformed_and_unauthorized(&desk).await;
}

fn assert_flag_reference_label(view: &DerivationView, specimen: &DerivationSpecimenView) {
    assert_eq!(view.specimens.len(), 5);
    assert_eq!(
        view.flag_reference_labels
            .as_ref()
            .unwrap()
            .get(&specimen.components[0].component_id.to_string()),
        Some(&specimen.components[0].orderable)
    );
}

/// Fails the analyst's only execution of sample `index`.
async fn fail_execution(desk: &Desk, index: usize) {
    desk.resync().await;
    desk.lab.sign_in(Role::Analyst).await;
    let e = desk.engine();
    let label = &desk.lab.samples[index].label_code;
    let run = e
        .get_run(GetRunInput {
            run_id: desk.lab.run,
        })
        .await
        .unwrap();
    let execution = run
        .positions
        .iter()
        .find(|p| &p.sample_label == label)
        .and_then(|p| p.execution.clone())
        .unwrap();
    assert_eq!(execution.state, ExecutionState::Active);
    e.fail_execution(FailExecutionInput {
        execution_id: execution.execution_id,
        expected_revision: execution.revision,
        reason: "Synthetic extraction failure".to_owned(),
    })
    .await
    .unwrap();
}

async fn refuse_malformed_and_unauthorized(desk: &Desk) {
    let unbounded = desk
        .as_staff(Staff::Author)
        .await
        .list_billing_work(ListBillingWork {
            after: None,
            target: None,
            filter: BillingWorkFilter::Held,
            limit: 0,
        })
        .await
        .unwrap_err();
    assert_eq!(unbounded.code(), ErrorCode::Validation);
    let malformed = desk
        .as_staff(Staff::Author)
        .await
        .derive_services(DeriveServices {
            subject_id: desk.lab.parties.subject,
            date_of_service: Some("2026-13-01".to_owned()),
        })
        .await
        .unwrap_err();
    assert_eq!(malformed.code(), ErrorCode::Validation);
    desk.resync().await;
    desk.lab.sign_in(Role::Reviewer).await;
    let derivations = desk.count("SELECT count(*) FROM bil_derivations");
    let refused = desk
        .engine()
        .derive_services(DeriveServices {
            subject_id: desk.lab.parties.subject,
            date_of_service: Some("2026-10-12".to_owned()),
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Forbidden);
    assert_eq!(
        desk.count("SELECT count(*) FROM bil_derivations"),
        derivations
    );
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events \
             WHERE action = 'billing.derive' AND outcome = 'denied'"
        ),
        1
    );
}

#[path = "billing_date_move.rs"]
mod date_move;

#[path = "billing_conflict.rs"]
mod conflict;

#[path = "billing_empty_date.rs"]
mod empty_date;

#[path = "billing_paging.rs"]
mod paging;
