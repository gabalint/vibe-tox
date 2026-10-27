//! Thin report layout IPC adapters (REQ-RPT-007). A layout preview opens in a
//! report window of its own; the engine does everything else.

use lis_core::CommandError;
use lis_engine::{
    ApproveReportLayoutInput, DraftReportLayoutInput, Engine, PreviewReportLayoutInput,
    RejectReportLayoutInput, ReportLayoutsView,
};
use tauri::{AppHandle, Runtime, State};

#[tauri::command]
pub(crate) async fn get_report_layouts(
    engine: State<'_, Engine>,
) -> Result<ReportLayoutsView, CommandError> {
    engine.get_report_layouts().await
}

#[tauri::command]
pub(crate) async fn preview_report_layout<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: PreviewReportLayoutInput,
) -> Result<(), CommandError> {
    let pdf = engine.preview_report_layout(input).await?;
    let layout = input.layout;
    crate::report_window::show(
        &app,
        &format!("layout-preview-{}", layout.as_str()),
        &format!("Report layout preview: {}", layout.name()),
        pdf,
    )
}

#[tauri::command]
pub(crate) async fn draft_report_layout(
    engine: State<'_, Engine>,
    input: DraftReportLayoutInput,
) -> Result<ReportLayoutsView, CommandError> {
    engine.draft_report_layout(input).await
}

#[tauri::command]
pub(crate) async fn approve_report_layout(
    engine: State<'_, Engine>,
    input: ApproveReportLayoutInput,
) -> Result<ReportLayoutsView, CommandError> {
    engine.approve_report_layout(input).await
}

#[tauri::command]
pub(crate) async fn reject_report_layout(
    engine: State<'_, Engine>,
    input: RejectReportLayoutInput,
) -> Result<ReportLayoutsView, CommandError> {
    engine.reject_report_layout(input).await
}
