//! Readiness, registrations, lots, runs and QC through the engine
//! (REQ-ANA-001, REQ-ANA-003–007, REQ-OPS-001): role separation, and the
//! clinical audit trail of each change. Synthetic data only.

use std::net::{IpAddr, Ipv4Addr, TcpListener};

use lis_analytical::{
    AttemptKind, CompletePrepBatchInput, DeviationId, ExecutionId, InstrumentStatus, LotId,
    MaterialKind, PrepBatchId, QcConflictId, QcConflictResolution, QcOutcome, QcRole, QcScope,
    ResolveQcConflictInput, RevisionIndicator, RunDetail, RunId, RunQcView, SampleIdField,
    SourceRegistrationId, SourceTestCodeInput,
};
use lis_auth::RoleCode;
use lis_catalog::{
    AnalyteId, ApproveMethodVersion, CodedName, CreateAnalyte, DraftMethodVersion, MethodAnalyte,
    MethodCertainty, MethodVersionContent, MethodVersionId, OutputScale, ThresholdRounding,
    ThresholdRule,
};
use lis_core::{CommandError, ErrorCode, InstrumentId, UserId};
use lis_engine::{
    AddPatientPositionInput, AddQcPositionInput, ApproveSourceInput, AssessQcInput,
    CloseDeviationInput, CloseRunInput, CreatePrepBatchInput, CreateRunInput, DeviationTarget,
    Engine, FailExecutionInput, GetRunInput, GetRunQcInput, ListLotsInput, ListRunsInput,
    ListSourceRegistrationsInput, ListWorklistInput, OpenDeviationInput, RecordPrepStepInput,
    RegisterInstrument, RegisterLotInput, RegisterSourceInput, SetInstrumentStatusInput,
    WithdrawLotInput,
};
use lis_interfaces::ConnectionSettings;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of};

/// Calls the analytical command `name` with plausible input; `None` for
/// other areas. Domain commands refuse at `ctx.require` before reading IDs.
pub(crate) async fn analytical_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = run_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = quality_call(engine, name).await {
        return Some(result);
    }
    let instrument_id = InstrumentId::generate();
    Some(match name {
        "list_instrument_readiness" => engine.list_instrument_readiness().await.map(drop),
        "list_source_registrations" => engine
            .list_source_registrations(ListSourceRegistrationsInput { instrument_id })
            .await
            .map(drop),
        "register_source" => engine
            .register_source(source_input(instrument_id, AnalyteId::generate()))
            .await
            .map(drop),
        "approve_source" => engine
            .approve_source(ApproveSourceInput {
                registration_id: SourceRegistrationId::generate(),
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "list_lots" => engine
            .list_lots(ListLotsInput { active_only: true })
            .await
            .map(drop),
        "register_lot" => engine.register_lot(lot_input("AUTH-1")).await.map(drop),
        "withdraw_lot" => engine
            .withdraw_lot(WithdrawLotInput {
                lot_id: LotId::generate(),
                expected_revision: 1,
                reason: "Synthetic".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn prep_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "create_prep_batch" => engine
            .create_prep_batch(CreatePrepBatchInput {
                method_version_id: MethodVersionId::generate(),
            })
            .await
            .map(drop),
        "record_prep_step" => engine
            .record_prep_step(RecordPrepStepInput {
                prep_batch_id: PrepBatchId::generate(),
                step_code: "spike".to_owned(),
                lot_id: None,
                note: None,
            })
            .await
            .map(drop),
        "complete_prep_batch" => engine
            .complete_prep_batch(CompletePrepBatchInput {
                prep_batch_id: PrepBatchId::generate(),
                expected_revision: 1,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn run_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    if let Some(result) = prep_call(engine, name).await {
        return Some(result);
    }
    let run_id = RunId::generate();
    Some(match name {
        "list_worklist" => engine
            .list_worklist(ListWorklistInput {
                method_version_id: MethodVersionId::generate(),
            })
            .await
            .map(drop),
        "create_run" => engine
            .create_run(CreateRunInput {
                instrument_id: InstrumentId::generate(),
                method_version_id: MethodVersionId::generate(),
                lot_ids: Vec::new(),
            })
            .await
            .map(drop),
        "get_run" => engine.get_run(GetRunInput { run_id }).await.map(drop),
        "list_runs" => engine
            .list_runs(ListRunsInput {
                method_version_id: None,
                after_run_id: None,
                limit: None,
                state: None,
                instrument_id: None,
            })
            .await
            .map(drop),
        "add_patient_position" => engine
            .add_patient_position(AddPatientPositionInput {
                run_id,
                position_label: "S1".to_owned(),
                container_label: "C00000001".to_owned(),
                attempt_kind: AttemptKind::Initial,
                related_execution_id: None,
                dilution_factor: None,
                prep_batch_id: None,
                reason: None,
            })
            .await
            .map(drop),
        "add_qc_position" => engine.add_qc_position(blank(run_id)).await.map(drop),
        "fail_execution" => engine
            .fail_execution(FailExecutionInput {
                execution_id: ExecutionId::generate(),
                expected_revision: 1,
                reason: "Synthetic".to_owned(),
            })
            .await
            .map(drop),
        "close_run" => engine
            .close_run(CloseRunInput {
                run_id,
                expected_revision: 1,
                reason: None,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn quality_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let run_id = RunId::generate();
    Some(match name {
        "get_run_qc" => engine.get_run_qc(GetRunQcInput { run_id }).await.map(drop),
        "assess_qc" => engine.assess_qc(run_qc(run_id)).await.map(drop),
        "resolve_qc_conflict" => engine
            .resolve_qc_conflict(ResolveQcConflictInput {
                conflict_id: QcConflictId::generate(),
                expected_revision: 1,
                resolution: QcConflictResolution::Reject,
                reason: "Synthetic".to_owned(),
            })
            .await
            .map(drop),
        "open_deviation" => engine
            .open_deviation(OpenDeviationInput {
                scope: DeviationTarget::Run(run_id),
                analyte_id: None,
                description: "Synthetic".to_owned(),
                reason: "Synthetic".to_owned(),
            })
            .await
            .map(drop),
        "close_deviation" => engine
            .close_deviation(CloseDeviationInput {
                deviation_id: DeviationId::generate(),
                expected_revision: 1,
                disposition: "Synthetic".to_owned(),
                reason: "Synthetic".to_owned(),
            })
            .await
            .map(drop),
        "set_instrument_status" => engine
            .set_instrument_status(status(InstrumentId::generate()))
            .await
            .map(drop),
        _ => return None,
    })
}

fn source_input(instrument_id: InstrumentId, analyte_id: AnalyteId) -> RegisterSourceInput {
    RegisterSourceInput {
        au_profile: None,
        instrument_id,
        sender_name: "SYNTHETIC".to_owned(),
        sender_version: "1.0".to_owned(),
        sample_id_field: SampleIdField::SpecimenId,
        revision_indicator: RevisionIndicator::None,
        reports_final_concentration: false,
        test_codes: vec![SourceTestCodeInput {
            source_test_code: "A1".to_owned(),
            analyte_id,
        }],
    }
}

fn lot_input(number: &str) -> RegisterLotInput {
    RegisterLotInput {
        material_kind: MaterialKind::Control,
        product_name: "Synthetic control".to_owned(),
        lot_number: number.to_owned(),
        level_text: None,
        sample_label: Some(format!("QC-{number}")),
        expires_on: "2027-12-31".to_owned(),
    }
}

fn blank(run_id: RunId) -> AddQcPositionInput {
    AddQcPositionInput {
        run_id,
        position_label: "B1".to_owned(),
        role: QcRole::Blank,
        lot_id: None,
        sample_label: Some("BLANK-1".to_owned()),
    }
}

fn run_qc(run_id: RunId) -> AssessQcInput {
    AssessQcInput {
        scope: QcScope::Run,
        run_id: Some(run_id),
        analyte_id: None,
        prep_batch_id: None,
        outcome: QcOutcome::Rejected,
        evidence_observation_ids: Vec::new(),
        supersedes_assessment_id: None,
        reason: None,
    }
}

fn status(instrument_id: InstrumentId) -> SetInstrumentStatusInput {
    SetInstrumentStatusInput {
        instrument_id,
        status: InstrumentStatus::InService,
        reason: "Synthetic daily maintenance".to_owned(),
    }
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Two distinct administrators, a quality manager and an analyst, with their TOTP secrets.
struct Staff {
    admin: String,
    approver: String,
    quality: String,
    analyst: String,
}

impl Staff {
    async fn create(t: &TestEngine) -> Self {
        let admin = t.admin_session().await;
        t.create_user("cfg2", &[RoleCode::Administrator]).await;
        t.create_user("qm1", &[RoleCode::QualityManager]).await;
        t.create_user("an1", &[RoleCode::Analyst]).await;
        let (_, approver) = t.first_sign_in("cfg2").await;
        let (_, quality) = t.first_sign_in("qm1").await;
        let (_, analyst) = t.first_sign_in("an1").await;
        Self {
            admin,
            approver,
            quality,
            analyst,
        }
    }

    async fn admin(&self, t: &TestEngine) {
        t.sign_in_with(ADMIN, PASSWORD, &self.admin).await;
    }

    async fn approver(&self, t: &TestEngine) {
        t.sign_in_with("cfg2", NEW_PASSWORD, &self.approver).await;
    }

    async fn quality(&self, t: &TestEngine) {
        t.sign_in_with("qm1", NEW_PASSWORD, &self.quality).await;
    }

    async fn analyst(&self, t: &TestEngine) {
        t.sign_in_with("an1", NEW_PASSWORD, &self.analyst).await;
    }
}

/// An approved method version for a new synthetic urine type: drafted by
/// one administrator, approved by the other.
async fn approved_method(t: &TestEngine, staff: &Staff, analyte: AnalyteId) -> MethodVersionId {
    let e = &t.engine;
    let coded = |code: &str| CodedName {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
    };
    let specimen_type_id = e.create_specimen_type(coded("SYN-UR")).await.unwrap();
    let method_id = e.create_method(coded("SYN-M")).await.unwrap();
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![MethodAnalyte {
            reference_interval: None,
            range_policy: lis_catalog::RangePolicy::ReportOnly,
            analyte_id: analyte,
            unit: Some("ng/mL".to_owned()),
            cutoff: None,
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: None,
        }],
    };
    let version = e
        .draft_method_version(DraftMethodVersion { method_id, content })
        .await
        .unwrap()
        .id;
    let approval = ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id: version,
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
    version
}

/// An analyzer in service with an approved registration, and an approved
/// method: configuration approval by a second administrator, readiness by
/// the quality manager. Administration alone grants no quality authority.
async fn configured(t: &TestEngine, staff: &Staff) -> (InstrumentId, MethodVersionId) {
    let e = &t.engine;
    staff.admin(t).await;
    let analyte = e
        .create_analyte(CreateAnalyte {
            code: "SYN-A".to_owned(),
            name: "Synthetic analyte".to_owned(),
            loinc_code: None,
            drug_class_code: None,
        })
        .await
        .unwrap();
    let instrument = e
        .register_instrument(RegisterInstrument {
            code: "SYN-ANALYZER".to_owned(),
            name: "Synthetic analyzer".to_owned(),
            connection: ConnectionSettings::AstmTcp {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: free_port(),
                allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            },
            enabled: false,
        })
        .await
        .unwrap()
        .instrument
        .id;
    let draft = e
        .register_source(source_input(instrument, analyte))
        .await
        .unwrap();
    // Administration grants no quality authority.
    assert_eq!(
        code_of(e.set_instrument_status(status(instrument)).await),
        ErrorCode::Forbidden
    );
    let method = approved_method(t, staff, analyte).await;
    e.approve_source(ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    staff.quality(t).await;
    e.set_instrument_status(status(instrument)).await.unwrap();
    let readiness = e.list_instrument_readiness().await.unwrap();
    assert_eq!(readiness[0].status, Some(InstrumentStatus::InService));
    assert!(readiness[0].active_registration.is_some());

    (instrument, method)
}

#[tokio::test]
async fn analytical_work_runs_through_the_engine_with_separate_roles() {
    let t = TestEngine::start().await;
    let staff = Staff::create(&t).await;
    let e = &t.engine;
    let (instrument, method) = configured(&t, &staff).await;
    staff.analyst(&t).await;
    assert_eq!(
        code_of(e.assess_qc(run_qc(RunId::generate())).await),
        ErrorCode::Forbidden
    );
    let run = e
        .create_run(CreateRunInput {
            instrument_id: instrument,
            method_version_id: method,
            lot_ids: Vec::new(),
        })
        .await
        .unwrap();
    let run = e.add_qc_position(blank(run.run_id)).await.unwrap();
    assert_eq!(run.positions.len(), 1);
    let fetched = e.get_run(GetRunInput { run_id: run.run_id }).await.unwrap();
    assert_eq!(fetched, run);
    e.list_worklist(ListWorklistInput {
        method_version_id: method,
    })
    .await
    .unwrap();

    staff.quality(&t).await;
    e.open_deviation(OpenDeviationInput {
        scope: DeviationTarget::Run(run.run_id),
        analyte_id: None,
        description: "Synthetic temperature alarm".to_owned(),
        reason: "Synthetic alarm log".to_owned(),
    })
    .await
    .unwrap();
    e.assess_qc(run_qc(run.run_id)).await.unwrap();
    let qc = e
        .get_run_qc(GetRunQcInput { run_id: run.run_id })
        .await
        .unwrap();
    assert_eq!(qc.deviations.len(), 1);
    assert_actor_names(&fetched, &qc);
    assert_eq!(
        qc.run_assessments.current.map(|a| a.outcome),
        Some(QcOutcome::Rejected)
    );

    staff.analyst(&t).await;
    let closed = e
        .close_run(CloseRunInput {
            run_id: run.run_id,
            expected_revision: run.revision,
            reason: None,
        })
        .await
        .unwrap();
    assert!(closed.allowed_actions.is_empty());
    let clinical: Vec<String> = t.events("clinical").into_iter().map(|e| e.0).collect();
    for action in [
        "analytical.register_source",
        "analytical.approve_source",
        "analytical.set_instrument_status",
        "analytical.create_run",
        "analytical.add_qc_position",
        "analytical.open_deviation",
        "analytical.assess_qc",
        "analytical.close_run",
    ] {
        assert!(clinical.iter().any(|a| a == action), "{action}");
    }
    // Reads and responses that disclose sample labels leave an access event.
    let access: Vec<String> = t.events("access").into_iter().map(|e| e.0).collect();
    for action in [
        "analytical.create_run",
        "analytical.add_qc_position",
        "analytical.get_run",
        "analytical.list_worklist",
        "analytical.close_run",
    ] {
        assert!(access.iter().any(|a| a == action), "{action}");
    }
    t.engine.shutdown().await;
}

/// People show by their current account name, never only by user ID.
fn assert_actor_names(run: &RunDetail, qc: &RunQcView) {
    assert_eq!(run.created_by_display.as_deref(), Some("Test an1"));
    assert_eq!(
        qc.deviations[0].opened_by_display.as_deref(),
        Some("Test qm1")
    );
    let assessment = qc.run_assessments.current.as_ref().unwrap();
    assert_eq!(assessment.assessed_by_display.as_deref(), Some("Test qm1"));
}
