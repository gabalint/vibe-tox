//! Thin result-review IPC adapters (REQ-RES-001–005, REQ-ORD-003).

use lis_core::CommandError;
use lis_engine::{
    Engine, GetImportAttemptInput, GetSpecimenResultsInput, ImportAttemptDetail,
    ImportAttemptSummary, ListImportAttemptsInput, ListReflexTriggersInput, ListReviewQueueInput,
    ListValidityPoliciesInput, RecordReflexDecisionInput, RecordResultDispositionInput,
    RecordValidityAssessmentInput, ReflexTriggerView, ResolveQuarantineItemInput,
    ResultRevisionView, RetryImportInput, ReviewQueuePage, ReviewView, SelectResultEvidenceInput,
    SpecimenResults, SubmitTechnicalReviewInput, UseMeasuredResultInput, ValidityAssessmentView,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_review_queue(
    engine: State<'_, Engine>,
    input: ListReviewQueueInput,
) -> Result<ReviewQueuePage, CommandError> {
    engine.list_review_queue(input).await
}

#[tauri::command]
pub(crate) async fn get_specimen_results(
    engine: State<'_, Engine>,
    input: GetSpecimenResultsInput,
) -> Result<SpecimenResults, CommandError> {
    engine.get_specimen_results(input).await
}

#[tauri::command]
pub(crate) async fn select_result_evidence(
    engine: State<'_, Engine>,
    input: SelectResultEvidenceInput,
) -> Result<ResultRevisionView, CommandError> {
    engine.select_result_evidence(input).await
}

#[tauri::command]
pub(crate) async fn record_result_disposition(
    engine: State<'_, Engine>,
    input: RecordResultDispositionInput,
) -> Result<ResultRevisionView, CommandError> {
    engine.record_result_disposition(input).await
}

#[tauri::command]
pub(crate) async fn submit_technical_review(
    engine: State<'_, Engine>,
    input: SubmitTechnicalReviewInput,
) -> Result<ReviewView, CommandError> {
    engine.submit_technical_review(input).await
}

#[tauri::command]
pub(crate) async fn list_validity_policies(
    engine: State<'_, Engine>,
    input: ListValidityPoliciesInput,
) -> Result<Vec<lis_engine::VersionSummary<lis_engine::ValidityPolicyVersion>>, CommandError> {
    engine.list_validity_policies(input).await
}

#[tauri::command]
pub(crate) async fn record_validity_assessment(
    engine: State<'_, Engine>,
    input: RecordValidityAssessmentInput,
) -> Result<ValidityAssessmentView, CommandError> {
    engine.record_validity_assessment(input).await
}

#[tauri::command]
pub(crate) async fn list_import_attempts(
    engine: State<'_, Engine>,
    input: ListImportAttemptsInput,
) -> Result<Vec<ImportAttemptSummary>, CommandError> {
    engine.list_import_attempts(input).await
}

#[tauri::command]
pub(crate) async fn get_import_attempt(
    engine: State<'_, Engine>,
    input: GetImportAttemptInput,
) -> Result<ImportAttemptDetail, CommandError> {
    engine.get_import_attempt(input).await
}

#[tauri::command]
pub(crate) async fn resolve_quarantine_item(
    engine: State<'_, Engine>,
    input: ResolveQuarantineItemInput,
) -> Result<ImportAttemptDetail, CommandError> {
    engine.resolve_quarantine_item(input).await
}

#[tauri::command]
pub(crate) async fn retry_import(
    engine: State<'_, Engine>,
    input: RetryImportInput,
) -> Result<i64, CommandError> {
    engine.retry_import(input).await
}

#[tauri::command]
pub(crate) async fn list_reflex_triggers(
    engine: State<'_, Engine>,
    input: ListReflexTriggersInput,
) -> Result<Vec<ReflexTriggerView>, CommandError> {
    engine.list_reflex_triggers(input).await
}

#[tauri::command]
pub(crate) async fn record_reflex_decision(
    engine: State<'_, Engine>,
    input: RecordReflexDecisionInput,
) -> Result<lis_engine::TriggerAssessmentView, CommandError> {
    engine.record_reflex_decision(input).await
}

#[tauri::command]
pub(crate) async fn use_measured_result(
    engine: State<'_, Engine>,
    input: UseMeasuredResultInput,
) -> Result<SpecimenResults, CommandError> {
    engine.use_measured_result(input).await
}

#[tauri::command]
pub(crate) async fn get_receipt_import_attempt(
    engine: State<'_, Engine>,
    input: lis_engine::GetReceiptImportAttemptInput,
) -> Result<Option<lis_engine::ImportAttemptSummary>, CommandError> {
    engine.get_receipt_import_attempt(input).await
}
