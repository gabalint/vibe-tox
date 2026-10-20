//! Standard Catalog and catalog review (REQ-OPS-014, REQ-OPS-015, proposed):
//! load tree, load, banner status, Catalog Preview and All Data Reviewed.
//! The engine authorizes, re-authenticates and audits; a shown preview opens
//! in a report window ([`crate::report_window`]).

use lis_core::CommandError;
use lis_engine::{
    ApproveReviewedCatalogInput, CatalogPreviewId, CatalogPreviewPdfInput, CatalogPreviewSummary,
    CatalogReviewApproved, Engine, ExportedReportFile, LoadStandardCatalog, StandardCatalogLoaded,
    StandardCatalogStatus, StandardCatalogTree,
};
use tauri::{AppHandle, Manager, Runtime, State};

#[tauri::command]
pub(crate) async fn standard_catalog_tree(
    engine: State<'_, Engine>,
) -> Result<StandardCatalogTree, CommandError> {
    engine.standard_catalog_tree().await
}

#[tauri::command]
pub(crate) async fn load_standard_catalog(
    engine: State<'_, Engine>,
    input: LoadStandardCatalog,
) -> Result<StandardCatalogLoaded, CommandError> {
    engine.load_standard_catalog(input).await
}

#[tauri::command]
pub(crate) async fn standard_catalog_status(
    engine: State<'_, Engine>,
) -> Result<StandardCatalogStatus, CommandError> {
    engine.standard_catalog_status().await
}

#[tauri::command]
pub(crate) async fn generate_catalog_preview(
    engine: State<'_, Engine>,
) -> Result<CatalogPreviewId, CommandError> {
    engine.generate_catalog_preview().await
}

#[tauri::command]
pub(crate) async fn list_catalog_previews(
    engine: State<'_, Engine>,
) -> Result<Vec<CatalogPreviewSummary>, CommandError> {
    engine.list_catalog_previews().await
}

#[tauri::command]
pub(crate) async fn view_catalog_preview_pdf<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: CatalogPreviewPdfInput,
) -> Result<(), CommandError> {
    let pdf = engine.view_catalog_preview_pdf(input).await?;
    let id = input.preview_id;
    crate::report_window::show(
        &app,
        &format!("catalog-preview-{id}"),
        &format!("Catalog Preview {id}"),
        pdf,
    )
}

#[tauri::command]
pub(crate) async fn export_catalog_preview_pdf<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: CatalogPreviewPdfInput,
) -> Result<ExportedReportFile, CommandError> {
    let downloads = app.path().download_dir().ok();
    engine.export_catalog_preview_pdf(input, downloads).await
}

#[tauri::command]
pub(crate) async fn approve_reviewed_catalog(
    engine: State<'_, Engine>,
    input: ApproveReviewedCatalogInput,
) -> Result<CatalogReviewApproved, CommandError> {
    engine.approve_reviewed_catalog(input).await
}
