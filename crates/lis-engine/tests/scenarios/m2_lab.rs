//! The synthetic laboratory of the M2 journey tests, built only through the
//! engine's public methods with the set-up `fixtures/astm/m2/README.md`
//! assumes: sender `SYNTH-LCMS` 1.0 with the sample label in O.3, test codes
//! `SYN-MOR` and `SYN-OXY` in `ng/mL` with cutoff `50` (`at_or_above`,
//! `before_rounding`, display scale 1), and one open run holding the control
//! positions `QC-LOW-SYN` and `QC-HIGH-SYN` and the patient containers
//! `C00000001` to `C00000005` ([`crate::m2_config`] builds the configuration).
//! Each step runs as the role that owns it. The quality fixture also holds the
//! Administrator role for configuration approval, separate from the drafting admin.
//! Synthetic data only.

use std::cell::Cell;

use lis_analytical::{AttemptKind, QcRole, RevisionIndicator, RunId};
use lis_auth::RoleCode;
use lis_catalog::OrderableVersionId;
use lis_core::{DecimalText, InstrumentId, Phi};
use lis_engine::{
    AcceptOrderInput, AddPatientPositionInput, AddQcPositionInput, AuthorityEvidenceInput,
    AuthorityEvidenceKind, BillingResponsibility, ClientId, CreateAccessionInput, CreateRunInput,
    Engine, EvidenceInput, OrderDetail, OrderId, OrderItemId, OrderItemInput, PractitionerId,
    RecordReceiptDecisionInput, ReflexAuthorization, RegisterOrderInput, SequenceRole, SourceKind,
    StandingOrderId, TemplateUseInput,
};
use lis_identity::{CreateClient, DemographicsInput, RegisterPractitioner, RegisterSubject};
use lis_specimens::{
    CollectionInput, CollectionTimeInput, ContainerId, ReceiptDecision, SpecimenId, SpecimenInput,
    VolumeKind,
};

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine};
use crate::m2_config::{Analytes, Configured, Methods, Orderables, URINE, configure};

/// Patient containers on the run, `C00000001` to `C00000005`.
const RUN_SAMPLES: usize = 5;

/// The journey's staff, one role each; the order indexes their TOTP secrets.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Admin,
    Quality,
    Accessioner,
    Analyst,
    Reviewer,
}

/// One accepted order with one received specimen in one container.
#[derive(Clone)]
pub(crate) struct Sample {
    pub(crate) order_id: OrderId,
    pub(crate) item_id: OrderItemId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) container_id: ContainerId,
    pub(crate) label_code: String,
}

/// The engine and its signed-in staff.
pub(crate) struct Desk {
    pub(crate) t: TestEngine,
    secrets: [String; 5],
    signed_in: Cell<Option<Role>>,
}

/// The configured laboratory with an open run.
pub(crate) struct Lab {
    pub(crate) desk: Desk,
    pub(crate) analytes: Analytes,
    pub(crate) methods: Methods,
    pub(crate) orderables: Orderables,
    pub(crate) instrument: InstrumentId,
    pub(crate) port: u16,
    pub(crate) run: RunId,
    pub(crate) parties: Parties,
    /// Index `n` holds container `C0000000{n + 1}`.
    pub(crate) samples: Vec<Sample>,
}

/// Who orders, and for whom.
pub(crate) struct Parties {
    client: ClientId,
    pub(crate) practitioner: PractitionerId,
    pub(crate) subject: lis_identity::SubjectId,
}

impl Desk {
    pub(crate) fn engine(&self) -> &Engine {
        &self.t.engine
    }

    /// Signs `role` in unless it is already the signed-in user.
    pub(crate) async fn sign_in(&self, role: Role) {
        if self.signed_in.get() == Some(role) {
            return;
        }
        let (username, password) = match role {
            Role::Admin => (ADMIN, PASSWORD),
            Role::Quality => ("qm1", NEW_PASSWORD),
            Role::Accessioner => ("acc1", NEW_PASSWORD),
            Role::Analyst => ("an1", NEW_PASSWORD),
            Role::Reviewer => ("rev1", NEW_PASSWORD),
        };
        self.t
            .sign_in_with(username, password, &self.secrets[role as usize])
            .await;
        self.signed_in.set(Some(role));
    }
}

impl Lab {
    /// Staff, configuration, an open run with both controls, and five
    /// accepted and received samples placed on it.
    pub(crate) async fn start(indicator: RevisionIndicator) -> Self {
        Self::start_as(indicator, lis_engine::InstallationKind::Laboratory).await
    }

    /// The same laboratory in an installation of `kind` (proposed REQ-OPS-016).
    pub(crate) async fn start_as(
        indicator: RevisionIndicator,
        kind: lis_engine::InstallationKind,
    ) -> Self {
        Self::start_as_with_profile(indicator, kind, None).await
    }

    /// Report benches establish the actual laboratory before creating any run.
    pub(crate) async fn start_as_with_profile(
        indicator: RevisionIndicator,
        kind: lis_engine::InstallationKind,
        profile: Option<lis_engine::SetLaboratoryProfile>,
    ) -> Self {
        let t = TestEngine::start_as(kind).await;
        let secrets = staff(&t).await;
        let desk = Desk {
            t,
            secrets,
            signed_in: Cell::new(None),
        };
        let configured = configure(&desk, indicator).await;
        let parties = register_parties(&desk).await;
        if let Some(profile) = profile {
            desk.sign_in(Role::Admin).await;
            desk.engine().set_laboratory_profile(profile).await.unwrap();
        }
        let run = open_run(&desk, &configured).await;
        let mut lab = Self {
            desk,
            analytes: configured.analytes,
            methods: configured.methods,
            orderables: configured.orderables,
            instrument: configured.instrument,
            port: configured.port,
            run,
            parties,
            samples: Vec::new(),
        };
        for _ in 0..RUN_SAMPLES {
            let sample = lab.add_sample(lab.orderables.panel.1).await;
            lab.place(&sample).await;
            lab.samples.push(sample);
        }
        lab
    }

    pub(crate) fn t(&self) -> &TestEngine {
        &self.desk.t
    }

    pub(crate) fn engine(&self) -> &Engine {
        self.desk.engine()
    }

    pub(crate) async fn sign_in(&self, role: Role) {
        self.desk.sign_in(role).await;
    }
    /// A paper order for one orderable version, as the accessioner enters it.
    pub(crate) fn order_input(
        &self,
        orderable_version_id: OrderableVersionId,
        standing_order_id: Option<StandingOrderId>,
    ) -> RegisterOrderInput {
        RegisterOrderInput {
            subject_id: self.parties.subject,
            client_id: self.parties.client,
            client_location_id: None,
            practitioner_id: self.parties.practitioner,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("SYN-REQ-M2".to_owned()),
            billing_responsibility: BillingResponsibility::NotSupplied,
            facility_setting: None,
            standing_order_id,
            template_use: TemplateUseInput::None,
            items: vec![OrderItemInput {
                orderable_version_id,
                sequence_role: SequenceRole::FirstLine,
                reflex_authorization: ReflexAuthorization::Authorized,
                reflex_scope_orderable_ids: vec![self.orderables.confirmation.0],
            }],
            evidence: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::WetSignature,
                    document_ref: Some("SYN-REQ-M2".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        }
    }

    /// Registers and accepts an order, accessions one urine container and
    /// accepts the specimen at receipt.
    pub(crate) async fn add_sample(&self, orderable_version_id: OrderableVersionId) -> Sample {
        self.add_sample_with_receipt(orderable_version_id, Vec::new())
            .await
    }

    /// Adds preserved held evidence followed by conditional acceptance for report tests.
    pub(crate) async fn add_sample_with_receipt(
        &self,
        orderable_version_id: OrderableVersionId,
        discrepancies: Vec<lis_specimens::DiscrepancyInput>,
    ) -> Sample {
        self.sign_in(Role::Accessioner).await;
        let e = self.engine();
        let order = e
            .register_order(self.order_input(orderable_version_id, None))
            .await
            .unwrap();
        let accepted: OrderDetail = e
            .accept_order(AcceptOrderInput {
                order_id: order.order_id,
                expected_revision: order.revision,
            })
            .await
            .unwrap();
        let item_id = accepted.items[0].item_id;
        let accession = e
            .create_accession(accession_input(accepted.order_id, item_id))
            .await
            .unwrap();
        let specimen = &accession.specimens[0];
        let conditional = !discrepancies.is_empty();
        let mut expected_revision = specimen.revision;
        if conditional {
            let held = e
                .record_receipt_decision(RecordReceiptDecisionInput {
                    specimen_id: specimen.specimen_id,
                    expected_revision,
                    decision: ReceiptDecision::Held,
                    discrepancies: discrepancies.clone(),
                    policy_reference: None,
                    scope: None,
                    reason: Some("Synthetic source review hold".to_owned()),
                })
                .await
                .unwrap();
            expected_revision = held.specimens[0].revision;
        }
        e.record_receipt_decision(RecordReceiptDecisionInput {
            specimen_id: specimen.specimen_id,
            expected_revision,
            decision: if conditional {
                ReceiptDecision::AcceptedConditionally
            } else {
                ReceiptDecision::Accepted
            },
            discrepancies,
            policy_reference: conditional.then(|| "Synthetic receipt policy".to_owned()),
            scope: conditional.then(|| "Original ordered scope".to_owned()),
            reason: None,
        })
        .await
        .unwrap();
        let container = &specimen.containers[0];
        Sample {
            order_id: accepted.order_id,
            item_id,
            specimen_id: specimen.specimen_id,
            container_id: container.container_id,
            label_code: container.label_code.clone(),
        }
    }

    /// Places a sample's container on the run as an initial attempt.
    pub(crate) async fn place(&self, sample: &Sample) {
        self.sign_in(Role::Analyst).await;
        self.engine()
            .add_patient_position(AddPatientPositionInput {
                run_id: self.run,
                position_label: format!("P-{}", sample.label_code),
                container_label: sample.label_code.clone(),
                attempt_kind: AttemptKind::Initial,
                related_execution_id: None,
                dilution_factor: None,
                prep_batch_id: None,
                reason: None,
            })
            .await
            .unwrap();
    }
}

async fn register_parties(desk: &Desk) -> Parties {
    desk.sign_in(Role::Admin).await;
    let e = desk.engine();
    let client = e
        .create_client(CreateClient {
            code: "SYN-CLIENT".to_owned(),
            name: "Synthetic Clinic".to_owned(),
        })
        .await
        .unwrap();
    let practitioner = e
        .register_practitioner(RegisterPractitioner {
            npi: "1234567893".to_owned(),
            family_name: "Synthetic".to_owned(),
            given_names: None,
        })
        .await
        .unwrap();
    desk.sign_in(Role::Accessioner).await;
    let subject = e
        .register_subject(RegisterSubject {
            demographics: DemographicsInput {
                family_name: Phi::new("SYNTHETIC".to_owned()),
                given_names: Some(Phi::new("Patient Mu".to_owned())),
                birth_date: Phi::new("1980-01-01".to_owned()),
                administrative_sex: None,
            },
            source: "requisition".to_owned(),
            identifiers: Vec::new(),
            acknowledged_candidates: Vec::new(),
        })
        .await
        .unwrap()
        .subject_id;
    Parties {
        client,
        practitioner,
        subject,
    }
}

/// An open run on the analyzer with both control positions.
async fn open_run(desk: &Desk, configured: &Configured) -> RunId {
    desk.sign_in(Role::Analyst).await;
    let e = desk.engine();
    let run = e
        .create_run(CreateRunInput {
            instrument_id: configured.instrument,
            method_version_id: configured.methods.drugs,
            lot_ids: configured.lots.to_vec(),
        })
        .await
        .unwrap()
        .run_id;
    for (label, lot_id) in [("Q1", configured.lots[0]), ("Q2", configured.lots[1])] {
        e.add_qc_position(AddQcPositionInput {
            run_id: run,
            position_label: label.to_owned(),
            role: QcRole::Control,
            lot_id: Some(lot_id),
            sample_label: None,
        })
        .await
        .unwrap();
    }
    run
}

/// Creates the five staff accounts; returns their TOTP secrets by [`Role`].
async fn staff(t: &TestEngine) -> [String; 5] {
    let admin = t.admin_session().await;
    let users = [
        ("qm1", RoleCode::QualityManager),
        ("acc1", RoleCode::Accessioner),
        ("an1", RoleCode::Analyst),
        ("rev1", RoleCode::TechnicalReviewer),
    ];
    for (username, role) in users {
        let roles = if username == "qm1" {
            vec![role, RoleCode::Administrator]
        } else {
            vec![role]
        };
        t.create_user(username, &roles).await;
    }
    let mut secrets = vec![admin];
    for (username, _) in users {
        secrets.push(t.first_sign_in(username).await.1);
    }
    secrets.try_into().unwrap()
}

fn accession_input(order_id: OrderId, item_id: OrderItemId) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids: vec![item_id],
        collection: CollectionInput {
            time: CollectionTimeInput::Datetime {
                local: "2026-10-12T03:30".to_owned(),
                utc_offset_minutes: -240,
            },
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
    }
}
