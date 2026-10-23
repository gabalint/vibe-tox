//! Accessioning and label printing through the engine (REQ-SPC-001–003,
//! REQ-SPC-005, REQ-OPS-001): an accepted order is accessioned, labels go
//! through the `spc.label.print` outbox worker to a TCP test printer, and
//! the sensitive reads are audited. Synthetic data only.

use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::thread;
use std::time::Duration;

use lis_auth::{DataKey, RoleCode};
use lis_catalog::{
    ComponentInput, ComponentPurpose, MethodAnalyte, MethodCertainty, MethodVersionContent,
    OrderableKind, OrderableVersionContent, OrderableVersionId, OutputScale, ThresholdRounding,
    ThresholdRule,
};
use lis_core::{CommandError, DecimalText, ErrorCode, Phi, UserId, UtcTimestamp};
use lis_engine::{
    AcceptOrderInput, AccessionDetail, ApproveMethodVersion, ApproveOrderableVersion,
    AssessExcursionInput, AuthorityEvidenceInput, AuthorityEvidenceKind, BillingResponsibility,
    CodedName, ConfigureLabelPrinterInput, ConfigureStorageLocationInput, CreateAccessionInput,
    CreateAliquotInput, CreateAnalyte, CreateOrderable, DisposeContainerInput, DraftMethodVersion,
    DraftOrderableVersion, Engine, EvidenceInput, FindContainerInput, GetAccessionInput,
    KeyProvider, MarkContainerDepletedInput, MoveContainerInput, OrderItemInput,
    PlaceContainerHoldInput, PrintLabelsInput, RecordExcursionInput, RecordReceiptDecisionInput,
    ReflexAuthorization, RegisterOrderInput, RegisterStandingOrderInput, ReleaseContainerHoldInput,
    SourceKind, TemplateUseInput,
};
use lis_identity::{CreateClient, DemographicsInput, RegisterPractitioner, RegisterSubject};
use lis_specimens::{
    CollectionInput, CollectionTimeInput, ContainerHoldKind, ContainerId, ExcursionId,
    ExcursionOutcome, HoldId, LabelJobKind, LabelOutcome, LocationId, PrinterId, PrinterTransport,
    ReceiptDecision, SpecimenId, SpecimenInput, VolumeKind,
};

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of};
use crate::m4_runtime::Host;

const URINE: &str = "SYN-UR";

#[tokio::test]
async fn order_accession_context_is_audited_and_changes_no_clinical_state() {
    let t = TestEngine::start().await;
    let staff = Staff::create(&t).await;
    let test = approved_urine_test(&t, &staff).await;
    let (order_id, items) = accepted_order(&t, &staff, test).await;
    let input = lis_orders::GetOrderInput { order_id };
    let before = t.fingerprint();
    let context = t.engine.get_order_accession_context(input).await.unwrap();
    assert_eq!(context.allowed_actions, ["create_accession"]);
    assert!(context.items.iter().all(|item| item.eligible));
    let after = t.fingerprint();
    let additions: Vec<_> = after.iter().filter(|row| !before.contains(row)).collect();
    assert_eq!(additions.len(), 1, "only the read audit event is added");
    assert!(before.iter().all(|row| after.contains(row)));
    assert!(additions[0].starts_with("audit_events: "));
    assert_eq!(
        t.events("access").last().unwrap().0,
        "specimen.view_order_accessions"
    );
    let accession = t
        .engine
        .create_accession(accession_input(order_id, items))
        .await
        .unwrap();
    let linked = t.engine.get_order_accession_context(input).await.unwrap();
    assert_eq!(linked.items[0].accession_id, Some(accession.accession_id));
    assert_eq!(linked.accessions[0].pending_receipt_count, 1);
    assert!(linked.allowed_actions.is_empty());
    staff.admin(&t).await;
    assert_eq!(
        t.engine
            .get_order_accession_context(input)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
    assert_eq!(
        t.events("security").last().unwrap(),
        &(
            "specimen.view_order_accessions".to_owned(),
            "denied".to_owned()
        )
    );
    t.engine.shutdown().await;
}

/// Calls the specimens command `name` with plausible input; `None` for other
/// areas. Domain commands refuse at `ctx.require` before reading these IDs.
pub(crate) async fn specimens_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = container_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = hold_and_configuration_call(engine, name).await {
        return Some(result);
    }
    let container_id = ContainerId::generate();
    Some(match name {
        "create_accession" => engine
            .create_accession(accession_input(
                lis_orders::OrderId::generate(),
                vec![lis_orders::OrderItemId::generate()],
            ))
            .await
            .map(drop),
        "get_accession" => engine
            .get_accession(GetAccessionInput {
                accession_id: lis_specimens::AccessionId::generate(),
            })
            .await
            .map(drop),
        "get_order_accession_context" => engine
            .get_order_accession_context(lis_orders::GetOrderInput {
                order_id: lis_orders::OrderId::generate(),
            })
            .await
            .map(drop),
        "find_container" => engine
            .find_container(FindContainerInput {
                label_code: "C00000001".to_owned(),
            })
            .await
            .map(drop),
        "record_receipt_decision" => engine
            .record_receipt_decision(receipt(SpecimenId::generate(), 1))
            .await
            .map(drop),
        "print_labels" => engine
            .print_labels(PrintLabelsInput {
                container_ids: vec![container_id],
                printer_id: PrinterId::generate(),
                kind: LabelJobKind::Print,
                reason: None,
            })
            .await
            .map(drop),
        "list_printers" => engine.list_printers().await.map(drop),
        "list_locations" => engine.list_locations().await.map(drop),
        _ => return None,
    })
}

async fn hold_and_configuration_call(
    engine: &Engine,
    name: &str,
) -> Option<Result<(), CommandError>> {
    let container_id = ContainerId::generate();
    let reason = || "Synthetic authorization test".to_owned();
    Some(match name {
        "place_hold" => engine
            .place_hold(PlaceContainerHoldInput {
                container_id,
                kind: ContainerHoldKind::Quality,
                reason: reason(),
            })
            .await
            .map(drop),
        "release_hold" => engine
            .release_hold(ReleaseContainerHoldInput {
                hold_id: HoldId::generate(),
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        "record_excursion" => engine
            .record_excursion(RecordExcursionInput {
                location_id: LocationId::generate(),
                observed_from: UtcTimestamp::parse("2026-10-12T07:00:00.000000000Z").unwrap(),
                observed_to: None,
                description: reason(),
            })
            .await
            .map(drop),
        "assess_excursion" => engine
            .assess_excursion(AssessExcursionInput {
                excursion_id: ExcursionId::generate(),
                expected_revision: 1,
                outcome: ExcursionOutcome::NoImpact,
                container_ids: Vec::new(),
                reason: reason(),
            })
            .await
            .map(drop),
        "configure_location" => engine
            .configure_location(location_input("fridge-9"))
            .await
            .map(drop),
        "configure_printer" => engine
            .configure_printer(printer_input(9100))
            .await
            .map(drop),
        _ => return None,
    })
}

async fn container_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let container_id = ContainerId::generate();
    let reason = || "Synthetic authorization test".to_owned();
    Some(match name {
        "create_aliquot" => engine
            .create_aliquot(CreateAliquotInput {
                parent_container_id: container_id,
                expected_revision: 1,
                volume_taken: None,
                volume_unit: None,
                volume_kind: VolumeKind::Unknown,
            })
            .await
            .map(drop),
        "mark_container_depleted" => engine
            .mark_container_depleted(MarkContainerDepletedInput {
                container_id,
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        "move_container" => engine
            .move_container(MoveContainerInput {
                container_id,
                expected_revision: 1,
                location_id: LocationId::generate(),
                reason: None,
            })
            .await
            .map(drop),
        "dispose_container" => engine
            .dispose_container(DisposeContainerInput {
                container_id,
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

pub(crate) fn accession_input(
    order_id: lis_orders::OrderId,
    order_item_ids: Vec<lis_orders::OrderItemId>,
) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids,
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

fn receipt(specimen_id: SpecimenId, expected_revision: i64) -> RecordReceiptDecisionInput {
    RecordReceiptDecisionInput {
        specimen_id,
        expected_revision,
        decision: ReceiptDecision::Accepted,
        discrepancies: Vec::new(),
        policy_reference: None,
        scope: None,
        reason: None,
    }
}

fn location_input(code: &str) -> ConfigureStorageLocationInput {
    ConfigureStorageLocationInput {
        location_id: None,
        expected_revision: None,
        code: code.to_owned(),
        name: "Synthetic fridge".to_owned(),
        storage_condition: None,
        active: true,
    }
}

pub(crate) fn printer_input(port: u16) -> ConfigureLabelPrinterInput {
    ConfigureLabelPrinterInput {
        printer_id: None,
        expected_revision: None,
        code: format!("bench-{port}"),
        name: "Synthetic bench printer".to_owned(),
        transport: PrinterTransport::Tcp,
        ip_address: Some("127.0.0.1".to_owned()),
        port: Some(port),
        queue: None,
        active: true,
    }
}

/// The signed-in staff of the journey, with their TOTP secrets.
struct Staff {
    admin: String,
    approver: String,
    quality: String,
    accessioner: String,
}

impl Staff {
    async fn create(t: &TestEngine) -> Self {
        let admin = t.admin_session().await;
        t.create_user("cfg2", &[RoleCode::Administrator]).await;
        t.create_user("qm1", &[RoleCode::QualityManager]).await;
        t.create_user("acc1", &[RoleCode::Accessioner]).await;
        let (_, approver) = t.first_sign_in("cfg2").await;
        let (_, quality) = t.first_sign_in("qm1").await;
        let (_, accessioner) = t.first_sign_in("acc1").await;
        Self {
            admin,
            approver,
            quality,
            accessioner,
        }
    }

    async fn admin(&self, t: &TestEngine) {
        t.sign_in_with(ADMIN, PASSWORD, &self.admin).await;
    }

    async fn quality(&self, t: &TestEngine) {
        t.sign_in_with("qm1", NEW_PASSWORD, &self.quality).await;
    }

    async fn approver(&self, t: &TestEngine) {
        t.sign_in_with("cfg2", NEW_PASSWORD, &self.approver).await;
    }

    async fn accessioner(&self, t: &TestEngine) {
        t.sign_in_with("acc1", NEW_PASSWORD, &self.accessioner)
            .await;
    }
}

/// A quantitative method for `specimen_type` with one analyte.
fn method_content(
    specimen_type_id: lis_catalog::SpecimenTypeId,
    analyte_id: lis_catalog::AnalyteId,
) -> MethodVersionContent {
    MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![MethodAnalyte {
            reference_interval: None,
            range_policy: lis_catalog::RangePolicy::ReportOnly,
            analyte_id,
            unit: Some("ng/mL".to_owned()),
            cutoff: Some(DecimalText::parse("100").unwrap()),
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: Some(0),
        }],
    }
}

/// An approved urine test: authored and approved by distinct administrators.
async fn approved_urine_test(
    t: &TestEngine,
    staff: &Staff,
) -> (lis_catalog::OrderableId, OrderableVersionId) {
    let coded = |code: &str| CodedName {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
    };
    staff.admin(t).await;
    let e = &t.engine;
    let urine = e.create_specimen_type(coded(URINE)).await.unwrap();
    let analyte = e
        .create_analyte(CreateAnalyte {
            code: "SYN-A".to_owned(),
            name: "Synthetic analyte".to_owned(),
            loinc_code: None,
            drug_class_code: None,
        })
        .await
        .unwrap();
    let method = e.create_method(coded("SYN-M")).await.unwrap();
    let content = method_content(urine, analyte);
    let method_version = e
        .draft_method_version(DraftMethodVersion {
            method_id: method,
            content,
        })
        .await
        .unwrap()
        .id;
    let orderable = e
        .create_orderable(CreateOrderable {
            code: "SYN-T".to_owned(),
            name: "Synthetic test".to_owned(),
            kind: OrderableKind::Test,
        })
        .await
        .unwrap();
    approve_method_with_role_checks(t, staff, method_version).await;
    let version = e
        .draft_orderable_version(DraftOrderableVersion {
            orderable_id: orderable,
            content: OrderableVersionContent {
                components: vec![ComponentInput {
                    method_version_id: method_version,
                    purpose: ComponentPurpose::Analytical,
                    analyte_ids: vec![analyte],
                }],
            },
        })
        .await
        .unwrap()
        .id;
    staff.approver(t).await;
    e.approve_orderable_version(ApproveOrderableVersion {
        orderable_version_id: version,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    (orderable, version)
}

async fn approve_method_with_role_checks(
    t: &TestEngine,
    staff: &Staff,
    method_version: lis_catalog::MethodVersionId,
) {
    let e = &t.engine;
    let approval = ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id: method_version,
        expected_revision: 1,
        self_approval: false,
    };
    staff.quality(t).await;
    assert_eq!(
        code_of(e.approve_method_version(approval.clone()).await),
        ErrorCode::Forbidden
    );
    staff.approver(t).await;
    e.approve_method_version(approval).await.unwrap();
    staff.admin(t).await;
}

async fn register_synthetic_standing_order(
    engine: &Engine,
    subject_id: lis_identity::SubjectId,
    practitioner_id: lis_identity::PractitionerId,
    orderable_id: lis_catalog::OrderableId,
) {
    engine
        .register_standing_order(RegisterStandingOrderInput {
            subject_id,
            practitioner_id,
            scope_orderable_ids: vec![orderable_id],
            valid_from: "2026-10-12".to_owned(),
            valid_until: "2026-12-31".to_owned(),
            visit_limit: Some(1),
            document_ref: "SYN-STANDING-1".to_owned(),
        })
        .await
        .unwrap();
}

/// A new accepted order for the urine test, entered by the accessioner.
async fn accepted_order(
    t: &TestEngine,
    staff: &Staff,
    test: (lis_catalog::OrderableId, OrderableVersionId),
) -> (lis_orders::OrderId, Vec<lis_orders::OrderItemId>) {
    staff.admin(t).await;
    let e = &t.engine;
    let client_id = e
        .create_client(CreateClient {
            code: "SYN-CLIENT".to_owned(),
            name: "Synthetic Clinic".to_owned(),
        })
        .await
        .unwrap();
    let practitioner_id = e
        .register_practitioner(RegisterPractitioner {
            npi: "1234567893".to_owned(),
            family_name: "Synthetic".to_owned(),
            given_names: None,
        })
        .await
        .unwrap();
    staff.accessioner(t).await;
    let subject_id = e
        .register_subject(RegisterSubject {
            demographics: DemographicsInput {
                family_name: Phi::new("SYNTHETIC".to_owned()),
                given_names: Some(Phi::new("Patient Beta".to_owned())),
                birth_date: Phi::new("1975-05-05".to_owned()),
                administrative_sex: None,
            },
            source: "requisition".to_owned(),
            identifiers: Vec::new(),
            acknowledged_candidates: Vec::new(),
        })
        .await
        .unwrap()
        .subject_id;
    register_synthetic_standing_order(e, subject_id, practitioner_id, test.0).await;
    let order = e
        .register_order(RegisterOrderInput {
            subject_id,
            client_id,
            client_location_id: None,
            practitioner_id,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("SYN-REQ-2".to_owned()),
            billing_responsibility: BillingResponsibility::NotSupplied,
            facility_setting: None,
            standing_order_id: None,
            template_use: TemplateUseInput::None,
            items: vec![OrderItemInput {
                orderable_version_id: test.1,
                sequence_role: lis_orders::SequenceRole::FirstLine,
                reflex_authorization: ReflexAuthorization::NotStated,
                reflex_scope_orderable_ids: Vec::new(),
            }],
            evidence: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::WetSignature,
                    document_ref: Some("SYN-REQ-2".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        })
        .await
        .unwrap();
    let accepted = e
        .accept_order(AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        })
        .await
        .unwrap();
    let items = accepted.items.iter().map(|item| item.item_id).collect();
    (accepted.order_id, items)
}

/// A TCP test printer accepting `jobs` connections; returns what each sent.
pub(crate) fn test_printer(jobs: usize) -> (u16, thread::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let receiver = thread::spawn(move || {
        (0..jobs)
            .map(|_| {
                let (mut connection, _) = listener.accept().unwrap();
                let mut received = Vec::new();
                connection.read_to_end(&mut received).unwrap();
                received
            })
            .collect()
    });
    (port, receiver)
}

/// The accession once the label worker has recorded an outcome for every
/// job. The worker runs on its own schedule, so this waits, with a bound.
async fn after_label_outcomes(t: &TestEngine, accession: &AccessionDetail) -> AccessionDetail {
    let input = GetAccessionInput {
        accession_id: accession.accession_id,
    };
    for _ in 0..400 {
        let detail = t.engine.get_accession(input).await.unwrap();
        let jobs = &detail.specimens[0].containers[0].label_jobs;
        if jobs.iter().all(|job| job.outcome.is_some()) {
            return detail;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the label worker recorded no outcome");
}

async fn exercise_hold_responses(t: &TestEngine, staff: &Staff, accession: &AccessionDetail) {
    let container = &accession.specimens[0].containers[0];
    staff.quality(t).await;
    let hold = t
        .engine
        .place_hold(PlaceContainerHoldInput {
            container_id: container.container_id,
            kind: ContainerHoldKind::Quality,
            reason: "Synthetic investigation".to_owned(),
        })
        .await
        .unwrap();
    staff.accessioner(t).await;
    let disposal = t
        .engine
        .dispose_container(DisposeContainerInput {
            container_id: container.container_id,
            expected_revision: 1,
            reason: "Synthetic".to_owned(),
        })
        .await;
    assert_eq!(code_of(disposal), ErrorCode::GateBlocked);
    let detail = t
        .engine
        .get_accession(GetAccessionInput {
            accession_id: accession.accession_id,
        })
        .await
        .unwrap();
    assert_eq!(detail.specimens[0].containers[0].open_holds.len(), 1);
    staff.quality(t).await;
    t.engine
        .release_hold(ReleaseContainerHoldInput {
            hold_id: hold.hold_id,
            expected_revision: hold.revision,
            reason: "Synthetic investigation closed".to_owned(),
        })
        .await
        .unwrap();
}

async fn exercise_excursion_responses(t: &TestEngine, staff: &Staff) {
    staff.admin(t).await;
    let location = t
        .engine
        .configure_location(location_input("fridge-2"))
        .await
        .unwrap();
    staff.accessioner(t).await;
    let excursion = t
        .engine
        .record_excursion(RecordExcursionInput {
            location_id: location.location_id,
            observed_from: UtcTimestamp::parse("2026-10-12T07:00:00.000000000Z").unwrap(),
            observed_to: None,
            description: "Synthetic temperature observation".to_owned(),
        })
        .await
        .unwrap();
    staff.quality(t).await;
    let assessed = t
        .engine
        .assess_excursion(AssessExcursionInput {
            excursion_id: excursion.excursion_id,
            expected_revision: excursion.revision,
            outcome: ExcursionOutcome::NoImpact,
            container_ids: Vec::new(),
            reason: "Synthetic assessment".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(
        (
            assessed.recorded_by_display.as_deref(),
            assessed.assessed_by_display.as_deref(),
        ),
        (Some("Test acc1"), Some("Test qm1"))
    );
}

/// The app's only printer policy refuses the loopback test printer the scenarios use:
/// label content never leaves for a destination outside the laboratory network
/// (REQ-SPC-004, NFR-32, security review 2026-10-13).
#[tokio::test]
async fn the_production_printer_policy_refuses_a_loopback_printer() {
    let host = Host::new();
    let engine = host
        .start(KeyProvider::Fixed(DataKey::from_bytes([7; 32])))
        .await;
    host.enrol_admin(&engine).await;
    let refused = engine
        .configure_printer(printer_input(9100))
        .await
        .unwrap_err();
    let json = serde_json::to_value(&refused).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "ipAddress", "{json}");
    assert_eq!(json["fieldErrors"][0]["code"], "not_lab_network", "{json}");
    engine.shutdown().await;
}

#[tokio::test]
async fn labels_go_through_the_outbox_worker_and_a_reprint_needs_a_reason() {
    let t = TestEngine::start().await;
    let staff = Staff::create(&t).await;
    let test = approved_urine_test(&t, &staff).await;
    let (port, printer) = test_printer(2);
    staff.admin(&t).await;
    let printer_id = t
        .engine
        .configure_printer(printer_input(port))
        .await
        .unwrap()
        .printer_id;
    let (order_id, items) = accepted_order(&t, &staff, test).await;
    let accession = t
        .engine
        .create_accession(accession_input(order_id, items))
        .await
        .unwrap();
    let container = &accession.specimens[0].containers[0];
    let print = |kind, reason: Option<&str>| PrintLabelsInput {
        container_ids: vec![container.container_id],
        printer_id,
        kind,
        reason: reason.map(str::to_owned),
    };
    t.engine
        .print_labels(print(LabelJobKind::Print, None))
        .await
        .unwrap();
    let printed = after_label_outcomes(&t, &accession).await;
    let jobs = &printed.specimens[0].containers[0].label_jobs;
    assert_eq!(jobs[0].outcome, Some(LabelOutcome::HandedOff));

    let refused = t
        .engine
        .print_labels(print(LabelJobKind::Reprint, None))
        .await;
    assert_eq!(code_of(refused), ErrorCode::Validation);
    t.engine
        .print_labels(print(LabelJobKind::Reprint, Some("Label torn")))
        .await
        .unwrap();
    let reprinted = after_label_outcomes(&t, &accession).await;
    let jobs = &reprinted.specimens[0].containers[0].label_jobs;
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[1].outcome, Some(LabelOutcome::HandedOff));
    // Both labels carry the same container code; the bytes are identical.
    let labels = printer.join().unwrap();
    assert_eq!(labels[0], labels[1]);
    let text = String::from_utf8(labels[0].clone()).unwrap();
    assert!(
        text.contains(&format!("^FH^FD{}^FS", container.label_code)),
        "{text}"
    );
    let clinical: Vec<String> = t
        .events("clinical")
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    let outcomes = clinical
        .iter()
        .filter(|action| *action == "specimen.record_label_outcome")
        .count();
    assert_eq!(outcomes, 2);
}

#[tokio::test]
async fn accession_reads_are_audited_and_material_gates_hold() {
    let t = TestEngine::start().await;
    let staff = Staff::create(&t).await;
    let test = approved_urine_test(&t, &staff).await;
    let (order_id, items) = accepted_order(&t, &staff, test).await;
    let accession = t
        .engine
        .create_accession(accession_input(order_id, items))
        .await
        .unwrap();
    let specimen = &accession.specimens[0];
    let container = &specimen.containers[0];
    let found = t
        .engine
        .find_container(FindContainerInput {
            label_code: container.label_code.clone(),
        })
        .await
        .unwrap();
    assert_eq!(found.accession_id, accession.accession_id);
    assert_eq!(found.subject.subject_id, accession.subject_id);
    assert_eq!(found.subject.family_name.expose(), "SYNTHETIC");
    assert!(
        t.events("access")
            .iter()
            .any(|(action, _)| action == "specimen.find_container")
    );
    staff.accessioner(&t).await;
    let received = t
        .engine
        .record_receipt_decision(receipt(specimen.specimen_id, 1))
        .await
        .unwrap();
    assert_eq!(
        received.specimens[0].decisions[0]
            .decided_by_display
            .as_deref(),
        Some("Test acc1")
    );
    exercise_hold_responses(&t, &staff, &accession).await;
    exercise_excursion_responses(&t, &staff).await;
    let access = t.events("access");
    assert_eq!(
        access.last().map(|(action, _)| action.as_str()),
        Some("specimen.assess_excursion")
    );
    assert_write_access_events(&t);
    // Locations are configuration; the accessioner may list but not configure them.
    staff.accessioner(&t).await;
    let configure = t
        .engine
        .configure_location(location_input("fridge-1"))
        .await;
    assert_eq!(code_of(configure), ErrorCode::Forbidden);
    assert_eq!(t.engine.list_locations().await.unwrap().len(), 1);
}

fn assert_write_access_events(t: &TestEngine) {
    let access_actions: Vec<String> = t
        .events("access")
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    for expected in [
        "order.register",
        "order.register_standing_order",
        "order.accept",
        "specimen.create_accession",
        "specimen.record_receipt_decision",
        "specimen.place_hold",
        "specimen.release_hold",
        "specimen.record_excursion",
        "specimen.assess_excursion",
    ] {
        assert!(
            access_actions.iter().any(|action| action == expected),
            "missing access event {expected}; recorded {access_actions:?}"
        );
    }
    let (streams, correlations): (i64, i64) = t
        .conn()
        .query_row(
            "SELECT count(DISTINCT stream), count(DISTINCT correlation_id) \
             FROM audit_events WHERE action = 'specimen.create_accession'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((streams, correlations), (2, 1));
}
