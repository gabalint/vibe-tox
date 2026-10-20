//! Thin report, delivery and destination IPC adapters (REQ-RPT-001/002/003/
//! 005/011/012/013). Export resolves the OS Downloads folder through Tauri's
//! core path resolver; the engine does everything else.

use lis_core::CommandError;
use lis_engine::{
    AddDeliveryObligationInput, CandidateView, ClosePreservationIncidentInput,
    CreateDeliveryDestinationInput, DestinationView, Engine, ExportReportPdfInput,
    ExportedReportFile, GetReportCandidateInput, GetReportInput, GetReportRevisionInput,
    GetReportWorkspaceInput, ImpactView, ListDeliveryDestinationsInput,
    ListDeliveryObligationsInput, ListReportsInput, ObligationView, PrepareReportCandidateInput,
    PreservationIncidentView, RecordDeliveryAttemptInput, RecordImpactAssessmentInput,
    RedirectDeliveryObligationInput, ReleaseReportInput, ReplaceDeliveryDestinationInput,
    ReportDetail, ReportSummary, ReportWorkspace, RetireDeliveryDestinationInput, RevisionDetail,
    RevisionSummary, VerifyDeliveryDestinationInput, WithdrawReportInput,
};
use tauri::{AppHandle, Manager, Runtime, State};

#[tauri::command]
pub(crate) async fn get_report_workspace(
    engine: State<'_, Engine>,
    input: GetReportWorkspaceInput,
) -> Result<ReportWorkspace, CommandError> {
    engine.get_report_workspace(input).await
}

#[tauri::command]
pub(crate) async fn prepare_report_candidate(
    engine: State<'_, Engine>,
    input: PrepareReportCandidateInput,
) -> Result<CandidateView, CommandError> {
    engine.prepare_report_candidate(input).await
}

#[tauri::command]
pub(crate) async fn get_report_candidate(
    engine: State<'_, Engine>,
    input: GetReportCandidateInput,
) -> Result<CandidateView, CommandError> {
    engine.get_report_candidate(input).await
}

#[tauri::command]
pub(crate) async fn release_report(
    engine: State<'_, Engine>,
    input: ReleaseReportInput,
) -> Result<RevisionSummary, CommandError> {
    engine.release_report(input).await
}

#[tauri::command]
pub(crate) async fn withdraw_report(
    engine: State<'_, Engine>,
    input: WithdrawReportInput,
) -> Result<ReportDetail, CommandError> {
    engine.withdraw_report(input).await
}

#[tauri::command]
pub(crate) async fn list_reports(
    engine: State<'_, Engine>,
    input: ListReportsInput,
) -> Result<Vec<ReportSummary>, CommandError> {
    engine.list_reports(input).await
}

#[tauri::command]
pub(crate) async fn get_report(
    engine: State<'_, Engine>,
    input: GetReportInput,
) -> Result<ReportDetail, CommandError> {
    engine.get_report(input).await
}

#[tauri::command]
pub(crate) async fn get_report_revision(
    engine: State<'_, Engine>,
    input: GetReportRevisionInput,
) -> Result<RevisionDetail, CommandError> {
    engine.get_report_revision(input).await
}

#[tauri::command]
pub(crate) async fn export_report_pdf<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: ExportReportPdfInput,
) -> Result<ExportedReportFile, CommandError> {
    let downloads = app.path().download_dir().ok();
    engine.export_report_pdf(input, downloads).await
}

#[tauri::command]
pub(crate) async fn record_impact_assessment(
    engine: State<'_, Engine>,
    input: RecordImpactAssessmentInput,
) -> Result<ImpactView, CommandError> {
    engine.record_impact_assessment(input).await
}

#[tauri::command]
pub(crate) async fn list_delivery_obligations(
    engine: State<'_, Engine>,
    input: ListDeliveryObligationsInput,
) -> Result<lis_engine::DeliveryQueuePage, CommandError> {
    engine.list_delivery_obligations(input).await
}

#[tauri::command]
pub(crate) async fn record_delivery_attempt(
    engine: State<'_, Engine>,
    input: RecordDeliveryAttemptInput,
) -> Result<ObligationView, CommandError> {
    engine.record_delivery_attempt(input).await
}

#[tauri::command]
pub(crate) async fn add_delivery_obligation(
    engine: State<'_, Engine>,
    input: AddDeliveryObligationInput,
) -> Result<ObligationView, CommandError> {
    engine.add_delivery_obligation(input).await
}

#[tauri::command]
pub(crate) async fn redirect_delivery_obligation(
    engine: State<'_, Engine>,
    input: RedirectDeliveryObligationInput,
) -> Result<ObligationView, CommandError> {
    engine.redirect_delivery_obligation(input).await
}

#[tauri::command]
pub(crate) async fn list_delivery_destinations(
    engine: State<'_, Engine>,
    input: ListDeliveryDestinationsInput,
) -> Result<Vec<DestinationView>, CommandError> {
    engine.list_delivery_destinations(input).await
}

#[tauri::command]
pub(crate) async fn create_delivery_destination(
    engine: State<'_, Engine>,
    input: CreateDeliveryDestinationInput,
) -> Result<DestinationView, CommandError> {
    engine.create_delivery_destination(input).await
}

#[tauri::command]
pub(crate) async fn verify_delivery_destination(
    engine: State<'_, Engine>,
    input: VerifyDeliveryDestinationInput,
) -> Result<DestinationView, CommandError> {
    engine.verify_delivery_destination(input).await
}

#[tauri::command]
pub(crate) async fn replace_delivery_destination(
    engine: State<'_, Engine>,
    input: ReplaceDeliveryDestinationInput,
) -> Result<DestinationView, CommandError> {
    engine.replace_delivery_destination(input).await
}

#[tauri::command]
pub(crate) async fn retire_delivery_destination(
    engine: State<'_, Engine>,
    input: RetireDeliveryDestinationInput,
) -> Result<DestinationView, CommandError> {
    engine.retire_delivery_destination(input).await
}

#[tauri::command]
pub(crate) async fn close_preservation_incident(
    engine: State<'_, Engine>,
    input: ClosePreservationIncidentInput,
) -> Result<PreservationIncidentView, CommandError> {
    engine.close_preservation_incident(input).await
}

#[tauri::command]
pub(crate) async fn get_delivery_obligation(
    engine: State<'_, Engine>,
    input: lis_engine::GetDeliveryObligationInput,
) -> Result<lis_engine::ObligationView, CommandError> {
    engine.get_delivery_obligation(input).await
}
