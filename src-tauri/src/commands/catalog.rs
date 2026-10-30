//! Catalog configuration (M2 IPC contract): reference data and method,
//! orderable and validity policy versions. The engine authorizes and audits.

use lis_core::CommandError;
use lis_engine::{
    AnalyteHistoryEntry, AnalyteId, AnalyteInput, AnalyteUsage, AnalyteView, ApproveMethodVersion,
    ApproveOrderableVersion, ApproveValidityPolicyVersion, CodedName, CreateAnalyte,
    CreateOrderable, DraftMethodVersion, DraftOrderableVersion, DraftValidityPolicyVersion, Engine,
    GetMethodVersionInput, GetOrderableVersionInput, GetValidityPolicyInput,
    ListMethodVersionsInput, ListOrderableVersionsInput, ListValidityPolicyVersionsInput, MethodId,
    MethodVersion, MethodVersionView, MethodView, OrderableId, OrderableVersion,
    OrderableVersionView, OrderableView, RetireMethodVersion, RetireOrderableVersion,
    RetireValidityPolicyVersion, SpecimenTypeId, SpecimenTypeView, UpdateAnalyte,
    UpdateMethodDraft, UpdateOrderableDraft, UpdateValidityPolicyDraft, ValidityPolicyVersion,
    ValidityPolicyView, VersionRef, VersionSummary,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_specimen_types(
    engine: State<'_, Engine>,
) -> Result<Vec<SpecimenTypeView>, CommandError> {
    engine.list_specimen_types().await
}

#[tauri::command]
pub(crate) async fn create_specimen_type(
    engine: State<'_, Engine>,
    input: CodedName,
) -> Result<SpecimenTypeId, CommandError> {
    engine.create_specimen_type(input).await
}

#[tauri::command]
pub(crate) async fn list_analytes(
    engine: State<'_, Engine>,
) -> Result<Vec<AnalyteView>, CommandError> {
    engine.list_analytes().await
}

#[tauri::command]
pub(crate) async fn create_analyte(
    engine: State<'_, Engine>,
    input: CreateAnalyte,
) -> Result<AnalyteId, CommandError> {
    engine.create_analyte(input).await
}

#[tauri::command]
pub(crate) async fn update_analyte(
    engine: State<'_, Engine>,
    input: UpdateAnalyte,
) -> Result<AnalyteView, CommandError> {
    engine.update_analyte(input).await
}

#[tauri::command]
pub(crate) async fn analyte_history(
    engine: State<'_, Engine>,
    input: AnalyteInput,
) -> Result<Vec<AnalyteHistoryEntry>, CommandError> {
    engine.analyte_history(input).await
}

#[tauri::command]
pub(crate) async fn analyte_usage(
    engine: State<'_, Engine>,
    input: AnalyteInput,
) -> Result<AnalyteUsage, CommandError> {
    engine.analyte_usage(input).await
}

#[tauri::command]
pub(crate) async fn list_methods(
    engine: State<'_, Engine>,
) -> Result<Vec<MethodView>, CommandError> {
    engine.list_methods().await
}

#[tauri::command]
pub(crate) async fn create_method(
    engine: State<'_, Engine>,
    input: CodedName,
) -> Result<MethodId, CommandError> {
    engine.create_method(input).await
}

#[tauri::command]
pub(crate) async fn list_orderables(
    engine: State<'_, Engine>,
) -> Result<Vec<OrderableView>, CommandError> {
    engine.list_orderables().await
}

#[tauri::command]
pub(crate) async fn create_orderable(
    engine: State<'_, Engine>,
    input: CreateOrderable,
) -> Result<OrderableId, CommandError> {
    engine.create_orderable(input).await
}

#[tauri::command]
pub(crate) async fn list_active_orderables(
    engine: State<'_, Engine>,
) -> Result<Vec<OrderableVersionView>, CommandError> {
    engine.list_active_orderables().await
}

#[tauri::command]
pub(crate) async fn list_method_versions(
    engine: State<'_, Engine>,
    input: ListMethodVersionsInput,
) -> Result<Vec<VersionSummary<MethodVersion>>, CommandError> {
    engine.list_method_versions(input).await
}

#[tauri::command]
pub(crate) async fn get_method_version(
    engine: State<'_, Engine>,
    input: GetMethodVersionInput,
) -> Result<MethodVersionView, CommandError> {
    engine.get_method_version(input).await
}

#[tauri::command]
pub(crate) async fn draft_method_version(
    engine: State<'_, Engine>,
    input: DraftMethodVersion,
) -> Result<VersionRef<MethodVersion>, CommandError> {
    engine.draft_method_version(input).await
}

#[tauri::command]
pub(crate) async fn update_method_draft(
    engine: State<'_, Engine>,
    input: UpdateMethodDraft,
) -> Result<VersionRef<MethodVersion>, CommandError> {
    engine.update_method_draft(input).await
}

#[tauri::command]
pub(crate) async fn approve_method_version(
    engine: State<'_, Engine>,
    input: ApproveMethodVersion,
) -> Result<VersionRef<MethodVersion>, CommandError> {
    engine.approve_method_version(input).await
}

#[tauri::command]
pub(crate) async fn retire_method_version(
    engine: State<'_, Engine>,
    input: RetireMethodVersion,
) -> Result<VersionRef<MethodVersion>, CommandError> {
    engine.retire_method_version(input).await
}

#[tauri::command]
pub(crate) async fn list_orderable_versions(
    engine: State<'_, Engine>,
    input: ListOrderableVersionsInput,
) -> Result<Vec<VersionSummary<OrderableVersion>>, CommandError> {
    engine.list_orderable_versions(input).await
}

#[tauri::command]
pub(crate) async fn get_orderable_version(
    engine: State<'_, Engine>,
    input: GetOrderableVersionInput,
) -> Result<OrderableVersionView, CommandError> {
    engine.get_orderable_version(input).await
}

#[tauri::command]
pub(crate) async fn draft_orderable_version(
    engine: State<'_, Engine>,
    input: DraftOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CommandError> {
    engine.draft_orderable_version(input).await
}

#[tauri::command]
pub(crate) async fn update_orderable_draft(
    engine: State<'_, Engine>,
    input: UpdateOrderableDraft,
) -> Result<VersionRef<OrderableVersion>, CommandError> {
    engine.update_orderable_draft(input).await
}

#[tauri::command]
pub(crate) async fn approve_orderable_version(
    engine: State<'_, Engine>,
    input: ApproveOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CommandError> {
    engine.approve_orderable_version(input).await
}

#[tauri::command]
pub(crate) async fn retire_orderable_version(
    engine: State<'_, Engine>,
    input: RetireOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CommandError> {
    engine.retire_orderable_version(input).await
}

#[tauri::command]
pub(crate) async fn list_validity_policy_versions(
    engine: State<'_, Engine>,
    input: ListValidityPolicyVersionsInput,
) -> Result<Vec<VersionSummary<ValidityPolicyVersion>>, CommandError> {
    engine.list_validity_policy_versions(input).await
}

#[tauri::command]
pub(crate) async fn get_validity_policy(
    engine: State<'_, Engine>,
    input: GetValidityPolicyInput,
) -> Result<ValidityPolicyView, CommandError> {
    engine.get_validity_policy(input).await
}

#[tauri::command]
pub(crate) async fn draft_validity_policy_version(
    engine: State<'_, Engine>,
    input: DraftValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
    engine.draft_validity_policy_version(input).await
}

#[tauri::command]
pub(crate) async fn update_validity_policy_draft(
    engine: State<'_, Engine>,
    input: UpdateValidityPolicyDraft,
) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
    engine.update_validity_policy_draft(input).await
}

#[tauri::command]
pub(crate) async fn approve_validity_policy_version(
    engine: State<'_, Engine>,
    input: ApproveValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
    engine.approve_validity_policy_version(input).await
}

#[tauri::command]
pub(crate) async fn retire_validity_policy_version(
    engine: State<'_, Engine>,
    input: RetireValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
    engine.retire_validity_policy_version(input).await
}
