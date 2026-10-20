//! Every analytical command refuses a principal without its capability
//! before reading or writing anything, and its capability alone passes the
//! check (REQ-IDN-003, NFR-08). Administrative capabilities grant nothing here.

use lis_analytical::{
    AddQcPositionInput, AnalyticalError, ApproveSourceInput, AssessQcInput, CloseDeviationInput,
    CloseRunInput, CompletePrepBatchInput, CreatePrepBatchInput, CreateRunInput, DeviationId,
    DeviationTarget, ExecutionId, FailExecutionInput, InstrumentStatus, LotId, MaterialKind,
    OpenDeviationInput, PrepBatchId, QcConflictId, QcConflictResolution, QcOutcome, QcRole,
    QcScope, RecordPrepStepInput, RegisterFileSourceInput, RegisterLotInput, RegisterSourceInput,
    ResolveQcConflictInput, RevisionIndicator, RunId, SampleIdField, SetInstrumentStatusInput,
    SourceRegistrationId, WithdrawLotInput,
};
use lis_core::{Capability, ErrorCode, InstrumentId};
use lis_store::CommandContext;

use crate::{Fixture, counts, fixture, patient_input, run};

type Command = Box<dyn FnOnce(&mut CommandContext<'_>) -> Result<(), AnalyticalError> + Send>;

const ALL: [Capability; 14] = [
    Capability::Accession,
    Capability::EnterOrders,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
    Capability::ReleaseReports,
    Capability::ManageQuality,
    Capability::Billing,
    Capability::ViewAudit,
    Capability::Export,
    Capability::BackupRestore,
    Capability::ManageUsers,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
    Capability::InstrumentIntake,
];

type Entry = (&'static str, Capability, Command);

fn entry<F>(name: &'static str, capability: Capability, command: F) -> Entry
where
    F: FnOnce(&mut CommandContext<'_>) -> Result<(), AnalyticalError> + Send + 'static,
{
    (name, capability, Box::new(command))
}

fn reason() -> String {
    "Synthetic authorization test".to_owned()
}

fn commands(f: &Fixture) -> Vec<Entry> {
    let mut all = configuration_commands(f.instrument);
    all.extend(run_commands(f.urine.method));
    all.extend(quality_commands(f.urine.analytes[0]));
    all
}

fn configuration_commands(instrument: InstrumentId) -> Vec<Entry> {
    vec![
        entry("set_instrument_status", Capability::ManageQuality, |ctx| {
            let input = SetInstrumentStatusInput {
                instrument_id: InstrumentId::generate(),
                status: InstrumentStatus::InService,
                reason: reason(),
            };
            lis_analytical::set_instrument_status(ctx, input).map(drop)
        }),
        entry(
            "register_source",
            Capability::ManageConfiguration,
            move |ctx| {
                let input = RegisterSourceInput {
                    au_profile: None,
                    instrument_id: instrument,
                    sender_name: "SYNTHETIC".to_owned(),
                    sender_version: "1".to_owned(),
                    sample_id_field: SampleIdField::SpecimenId,
                    revision_indicator: RevisionIndicator::None,
                    reports_final_concentration: false,
                    test_codes: Vec::new(),
                };
                lis_analytical::register_source(ctx, input).map(drop)
            },
        ),
        entry(
            "register_file_source",
            Capability::ManageConfiguration,
            move |ctx| {
                let input = RegisterFileSourceInput {
                    instrument_id: instrument,
                    software_name: "SynthQuant".to_owned(),
                    software_version: "1".to_owned(),
                    reports_final_concentration: false,
                    layout: crate::file_sources::layout(),
                    test_codes: Vec::new(),
                };
                lis_analytical::register_file_source(ctx, input).map(drop)
            },
        ),
        entry("approve_source", Capability::ApproveConfiguration, |ctx| {
            let input = ApproveSourceInput {
                registration_id: SourceRegistrationId::generate(),
                expected_revision: 1,
                self_approval: false,
            };
            lis_analytical::approve_source(ctx, input).map(drop)
        }),
        entry("register_lot", Capability::ManageQuality, |ctx| {
            let input = RegisterLotInput {
                material_kind: MaterialKind::Reagent,
                product_name: " ".to_owned(),
                lot_number: "L".to_owned(),
                level_text: None,
                sample_label: None,
                expires_on: "2027-01-01".to_owned(),
            };
            lis_analytical::register_lot(ctx, input).map(drop)
        }),
        entry("withdraw_lot", Capability::ManageQuality, |ctx| {
            let input = WithdrawLotInput {
                lot_id: LotId::generate(),
                expected_revision: 1,
                reason: reason(),
            };
            lis_analytical::withdraw_lot(ctx, input).map(drop)
        }),
    ]
}

fn run_commands(method: lis_catalog::MethodVersionId) -> Vec<Entry> {
    let run_id = RunId::generate();
    vec![
        entry("create_prep_batch", Capability::PerformAnalysis, |ctx| {
            let input = CreatePrepBatchInput {
                method_version_id: lis_catalog::MethodVersionId::generate(),
            };
            lis_analytical::create_prep_batch(ctx, input).map(drop)
        }),
        entry("record_prep_step", Capability::PerformAnalysis, |ctx| {
            let input = RecordPrepStepInput {
                prep_batch_id: PrepBatchId::generate(),
                step_code: "s".to_owned(),
                lot_id: None,
                note: None,
            };
            lis_analytical::record_prep_step(ctx, input).map(drop)
        }),
        entry("complete_prep_batch", Capability::PerformAnalysis, |ctx| {
            let input = CompletePrepBatchInput {
                prep_batch_id: PrepBatchId::generate(),
                expected_revision: 1,
            };
            lis_analytical::complete_prep_batch(ctx, input).map(drop)
        }),
        entry("create_run", Capability::PerformAnalysis, move |ctx| {
            let input = CreateRunInput {
                instrument_id: InstrumentId::generate(),
                method_version_id: method,
                lot_ids: Vec::new(),
            };
            lis_analytical::create_run(ctx, input).map(drop)
        }),
        entry(
            "add_patient_position",
            Capability::PerformAnalysis,
            move |ctx| {
                lis_analytical::add_patient_position(ctx, patient_input(run_id, "S1", "C1"))
                    .map(drop)
            },
        ),
        entry("add_qc_position", Capability::PerformAnalysis, move |ctx| {
            let input = AddQcPositionInput {
                run_id,
                position_label: "P1".to_owned(),
                role: QcRole::Blank,
                lot_id: None,
                sample_label: Some("B".to_owned()),
            };
            lis_analytical::add_qc_position(ctx, input).map(drop)
        }),
        entry("fail_execution", Capability::PerformAnalysis, |ctx| {
            let input = FailExecutionInput {
                execution_id: ExecutionId::generate(),
                expected_revision: 1,
                reason: reason(),
            };
            lis_analytical::fail_execution(ctx, input).map(drop)
        }),
        entry("close_run", Capability::PerformAnalysis, move |ctx| {
            let input = CloseRunInput {
                run_id,
                expected_revision: 1,
                reason: None,
            };
            lis_analytical::close_run(ctx, input).map(drop)
        }),
    ]
}

fn quality_commands(analyte: lis_catalog::AnalyteId) -> Vec<Entry> {
    let run_id = RunId::generate();
    vec![
        entry("assess_qc", Capability::ManageQuality, move |ctx| {
            let input = AssessQcInput {
                scope: QcScope::RunAnalyte,
                run_id: Some(run_id),
                analyte_id: Some(analyte),
                prep_batch_id: None,
                outcome: QcOutcome::Accepted,
                evidence_observation_ids: Vec::new(),
                supersedes_assessment_id: None,
                reason: None,
            };
            lis_analytical::assess_qc(ctx, input).map(drop)
        }),
        entry("open_deviation", Capability::ManageQuality, move |ctx| {
            let input = OpenDeviationInput {
                scope: DeviationTarget::Run(run_id),
                analyte_id: None,
                description: "Synthetic".to_owned(),
                reason: reason(),
            };
            lis_analytical::open_deviation(ctx, input).map(drop)
        }),
        entry("close_deviation", Capability::ManageQuality, |ctx| {
            let input = CloseDeviationInput {
                deviation_id: DeviationId::generate(),
                expected_revision: 1,
                disposition: "Synthetic".to_owned(),
                reason: reason(),
            };
            lis_analytical::close_deviation(ctx, input).map(drop)
        }),
        entry("resolve_qc_conflict", Capability::ManageQuality, |ctx| {
            let input = ResolveQcConflictInput {
                conflict_id: QcConflictId::generate(),
                expected_revision: 1,
                resolution: QcConflictResolution::Reject,
                reason: reason(),
            };
            lis_analytical::resolve_qc_conflict(ctx, input).map(drop)
        }),
        entry(
            "record_qc_observations",
            Capability::InstrumentIntake,
            |ctx| lis_analytical::record_qc_observations(ctx, &[]).map(drop),
        ),
    ]
}

#[test]
fn every_command_refuses_a_principal_without_its_capability() {
    let f = fixture();
    let before = counts(&f);
    let mut checked = 0;
    for (name, capability, command) in commands(&f) {
        let others: Vec<Capability> = ALL.into_iter().filter(|c| *c != capability).collect();
        let result = run(&f, &others, command);
        assert!(
            matches!(&result, Err(error) if error.code() == ErrorCode::Forbidden),
            "{name}: {result:?}"
        );
        assert_eq!(counts(&f), before, "{name} wrote something");
        checked += 1;
    }
    assert_eq!(checked, 19);
}

#[test]
fn each_command_passes_the_check_with_its_capability_alone() {
    let f = fixture();
    for (name, capability, command) in commands(&f) {
        let result = run(&f, &[capability], command);
        if let Err(error) = &result {
            assert_ne!(error.code(), ErrorCode::Forbidden, "{name}: {error:?}");
        }
    }
}
