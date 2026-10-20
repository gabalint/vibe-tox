//! Instrument readiness, source registration, lot, preparation, worklist,
//! run, QC and deviation commands (M2 IPC contract; capabilities
//! `perform_analysis`, `manage_quality`, `manage_configuration` and
//! `approve_configuration`, checked by the engine).

use lis_core::CommandError;
use lis_engine::{
    AddPatientPositionInput, AddQcPositionInput, ApproveSourceInput, AssessQcInput,
    CloseDeviationInput, CloseRunInput, CompletePrepBatchInput, CreatePrepBatchInput,
    CreateRunInput, DeviationView, Engine, FailExecutionInput, GetRunInput, GetRunQcInput,
    InstrumentReadinessView, ListLotsInput, ListRunsInput, ListSourceRegistrationsInput,
    ListWorklistInput, LotView, OpenDeviationInput, PrepBatchView, QcAssessmentView,
    QcConflictView, RecordPrepStepInput, RegisterLotInput, RegisterSourceInput,
    ResolveQcConflictInput, RunDetail, RunQcView, RunSummary, SetInstrumentStatusInput,
    SourceRegistrationView, WithdrawLotInput, WorklistEntry,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_instrument_readiness(
    engine: State<'_, Engine>,
) -> Result<Vec<InstrumentReadinessView>, CommandError> {
    engine.list_instrument_readiness().await
}

#[tauri::command]
pub(crate) async fn list_source_registrations(
    engine: State<'_, Engine>,
    input: ListSourceRegistrationsInput,
) -> Result<Vec<SourceRegistrationView>, CommandError> {
    engine.list_source_registrations(input).await
}

#[tauri::command]
pub(crate) async fn register_source(
    engine: State<'_, Engine>,
    input: RegisterSourceInput,
) -> Result<SourceRegistrationView, CommandError> {
    engine.register_source(input).await
}

#[tauri::command]
pub(crate) async fn approve_source(
    engine: State<'_, Engine>,
    input: ApproveSourceInput,
) -> Result<SourceRegistrationView, CommandError> {
    engine.approve_source(input).await
}

#[tauri::command]
pub(crate) async fn list_lots(
    engine: State<'_, Engine>,
    input: ListLotsInput,
) -> Result<Vec<LotView>, CommandError> {
    engine.list_lots(input).await
}

#[tauri::command]
pub(crate) async fn register_lot(
    engine: State<'_, Engine>,
    input: RegisterLotInput,
) -> Result<LotView, CommandError> {
    engine.register_lot(input).await
}

#[tauri::command]
pub(crate) async fn withdraw_lot(
    engine: State<'_, Engine>,
    input: WithdrawLotInput,
) -> Result<LotView, CommandError> {
    engine.withdraw_lot(input).await
}

#[tauri::command]
pub(crate) async fn create_prep_batch(
    engine: State<'_, Engine>,
    input: CreatePrepBatchInput,
) -> Result<PrepBatchView, CommandError> {
    engine.create_prep_batch(input).await
}

#[tauri::command]
pub(crate) async fn record_prep_step(
    engine: State<'_, Engine>,
    input: RecordPrepStepInput,
) -> Result<PrepBatchView, CommandError> {
    engine.record_prep_step(input).await
}

#[tauri::command]
pub(crate) async fn list_worklist(
    engine: State<'_, Engine>,
    input: ListWorklistInput,
) -> Result<Vec<WorklistEntry>, CommandError> {
    engine.list_worklist(input).await
}

#[tauri::command]
pub(crate) async fn create_run(
    engine: State<'_, Engine>,
    input: CreateRunInput,
) -> Result<RunDetail, CommandError> {
    engine.create_run(input).await
}

#[tauri::command]
pub(crate) async fn get_run(
    engine: State<'_, Engine>,
    input: GetRunInput,
) -> Result<RunDetail, CommandError> {
    engine.get_run(input).await
}

#[tauri::command]
pub(crate) async fn list_runs(
    engine: State<'_, Engine>,
    input: ListRunsInput,
) -> Result<Vec<RunSummary>, CommandError> {
    engine.list_runs(input).await
}

#[tauri::command]
pub(crate) async fn add_patient_position(
    engine: State<'_, Engine>,
    input: AddPatientPositionInput,
) -> Result<RunDetail, CommandError> {
    engine.add_patient_position(input).await
}

#[tauri::command]
pub(crate) async fn add_qc_position(
    engine: State<'_, Engine>,
    input: AddQcPositionInput,
) -> Result<RunDetail, CommandError> {
    engine.add_qc_position(input).await
}

#[tauri::command]
pub(crate) async fn fail_execution(
    engine: State<'_, Engine>,
    input: FailExecutionInput,
) -> Result<RunDetail, CommandError> {
    engine.fail_execution(input).await
}

#[tauri::command]
pub(crate) async fn close_run(
    engine: State<'_, Engine>,
    input: CloseRunInput,
) -> Result<RunDetail, CommandError> {
    engine.close_run(input).await
}

#[tauri::command]
pub(crate) async fn get_run_qc(
    engine: State<'_, Engine>,
    input: GetRunQcInput,
) -> Result<RunQcView, CommandError> {
    engine.get_run_qc(input).await
}

#[tauri::command]
pub(crate) async fn assess_qc(
    engine: State<'_, Engine>,
    input: AssessQcInput,
) -> Result<QcAssessmentView, CommandError> {
    engine.assess_qc(input).await
}

#[tauri::command]
pub(crate) async fn open_deviation(
    engine: State<'_, Engine>,
    input: OpenDeviationInput,
) -> Result<DeviationView, CommandError> {
    engine.open_deviation(input).await
}

#[tauri::command]
pub(crate) async fn close_deviation(
    engine: State<'_, Engine>,
    input: CloseDeviationInput,
) -> Result<DeviationView, CommandError> {
    engine.close_deviation(input).await
}

#[tauri::command]
pub(crate) async fn set_instrument_status(
    engine: State<'_, Engine>,
    input: SetInstrumentStatusInput,
) -> Result<InstrumentReadinessView, CommandError> {
    engine.set_instrument_status(input).await
}

#[tauri::command]
pub(crate) async fn complete_prep_batch(
    engine: State<'_, Engine>,
    input: CompletePrepBatchInput,
) -> Result<PrepBatchView, CommandError> {
    engine.complete_prep_batch(input).await
}

#[tauri::command]
pub(crate) async fn resolve_qc_conflict(
    engine: State<'_, Engine>,
    input: ResolveQcConflictInput,
) -> Result<QcConflictView, CommandError> {
    engine.resolve_qc_conflict(input).await
}

#[tauri::command]
pub(crate) async fn get_prep_batch(
    engine: State<'_, Engine>,
    input: lis_engine::GetPrepBatchInput,
) -> Result<PrepBatchView, CommandError> {
    engine.get_prep_batch(input).await
}

#[tauri::command]
pub(crate) async fn list_prep_batches(
    engine: State<'_, Engine>,
    input: lis_engine::ListPrepBatchesInput,
) -> Result<Vec<lis_engine::PrepBatchSummary>, CommandError> {
    engine.list_prep_batches(input).await
}

#[tauri::command]
pub(crate) async fn record_run_performing_site(
    engine: State<'_, Engine>,
    input: lis_engine::RecordRunPerformingSiteInput,
) -> Result<RunDetail, CommandError> {
    engine.record_run_performing_site(input).await
}

#[tauri::command]
pub(crate) async fn get_deviation(
    engine: State<'_, Engine>,
    input: lis_engine::GetDeviationInput,
) -> Result<lis_engine::DeviationView, CommandError> {
    engine.get_deviation(input).await
}
