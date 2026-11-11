//! Thin billing adapters (REQ-BIL-001/002/005).

use lis_core::CommandError;
use lis_engine::{
    ApproveClassMappingVersion, ApproveServiceDerivation, BillingClassMappingVersionSummary,
    BillingClassMappingVersionView, BillingHandoffInput, BillingHandoffSummary, BillingWorkItem,
    ConfirmClassMappingVersion, CreateBillingHandoff, DerivationView, DeriveServices,
    DraftClassMappingVersion, Engine, ExportedHandoffFile, GetClassMappingVersionInput,
    GetServiceDerivationInput, HandoffView, ListBillingHandoffs, ListBillingWork,
    RecordHandoffOutcome, RetireClassMappingVersion, UpdateClassMappingDraft,
};
use tauri::{AppHandle, Manager, Runtime, State};

#[tauri::command]
pub(crate) async fn list_class_mapping_versions(
    engine: State<'_, Engine>,
) -> Result<Vec<BillingClassMappingVersionSummary>, CommandError> {
    engine.list_class_mapping_versions().await
}

#[tauri::command]
pub(crate) async fn get_class_mapping_version(
    engine: State<'_, Engine>,
    input: GetClassMappingVersionInput,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.get_class_mapping_version(input).await
}

#[tauri::command]
pub(crate) async fn draft_class_mapping_version(
    engine: State<'_, Engine>,
    input: DraftClassMappingVersion,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.draft_class_mapping_version(input).await
}

#[tauri::command]
pub(crate) async fn update_class_mapping_draft(
    engine: State<'_, Engine>,
    input: UpdateClassMappingDraft,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.update_class_mapping_draft(input).await
}

#[tauri::command]
pub(crate) async fn approve_class_mapping_version(
    engine: State<'_, Engine>,
    input: ApproveClassMappingVersion,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.approve_class_mapping_version(input).await
}

#[tauri::command]
pub(crate) async fn confirm_class_mapping_version(
    engine: State<'_, Engine>,
    input: ConfirmClassMappingVersion,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.confirm_class_mapping_version(input).await
}

#[tauri::command]
pub(crate) async fn retire_class_mapping_version(
    engine: State<'_, Engine>,
    input: RetireClassMappingVersion,
) -> Result<BillingClassMappingVersionView, CommandError> {
    engine.retire_class_mapping_version(input).await
}

#[tauri::command]
pub(crate) async fn list_billing_work(
    engine: State<'_, Engine>,
    input: ListBillingWork,
) -> Result<Vec<BillingWorkItem>, CommandError> {
    engine.list_billing_work(input).await
}

#[tauri::command]
pub(crate) async fn list_billing_handoffs(
    engine: State<'_, Engine>,
    input: ListBillingHandoffs,
) -> Result<Vec<BillingHandoffSummary>, CommandError> {
    engine.list_billing_handoffs(input).await
}

#[tauri::command]
pub(crate) async fn derive_services(
    engine: State<'_, Engine>,
    input: DeriveServices,
) -> Result<DerivationView, CommandError> {
    engine.derive_services(input).await
}

#[tauri::command]
pub(crate) async fn get_service_derivation(
    engine: State<'_, Engine>,
    input: GetServiceDerivationInput,
) -> Result<DerivationView, CommandError> {
    engine.get_service_derivation(input).await
}

#[tauri::command]
pub(crate) async fn approve_service_derivation(
    engine: State<'_, Engine>,
    input: ApproveServiceDerivation,
) -> Result<DerivationView, CommandError> {
    engine.approve_service_derivation(input).await
}

#[tauri::command]
pub(crate) async fn create_billing_handoff(
    engine: State<'_, Engine>,
    input: CreateBillingHandoff,
) -> Result<HandoffView, CommandError> {
    engine.create_billing_handoff(input).await
}

#[tauri::command]
pub(crate) async fn get_billing_handoff(
    engine: State<'_, Engine>,
    input: BillingHandoffInput,
) -> Result<HandoffView, CommandError> {
    engine.get_billing_handoff(input).await
}

#[tauri::command]
pub(crate) async fn export_billing_handoff<R: Runtime>(
    app: AppHandle<R>,
    engine: State<'_, Engine>,
    input: BillingHandoffInput,
) -> Result<ExportedHandoffFile, CommandError> {
    let downloads = app.path().download_dir().ok();
    engine.export_billing_handoff(input, downloads).await
}

#[tauri::command]
pub(crate) async fn record_handoff_outcome(
    engine: State<'_, Engine>,
    input: RecordHandoffOutcome,
) -> Result<HandoffView, CommandError> {
    engine.record_handoff_outcome(input).await
}

#[tauri::command]
pub(crate) async fn resolve_billing_context_conflict(
    engine: State<'_, Engine>,
    input: lis_engine::ResolveBillingContextConflictInput,
) -> Result<lis_engine::DerivationView, CommandError> {
    engine.resolve_billing_context_conflict(input).await
}
#[tauri::command]
pub(crate) async fn record_handoff_followup(
    engine: State<'_, Engine>,
    input: lis_engine::RecordHandoffFollowupInput,
) -> Result<lis_engine::HandoffView, CommandError> {
    engine.record_handoff_followup(input).await
}
