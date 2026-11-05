//! Audit trail commands (M1 IPC contract; capability `view_audit`). The engine
//! records an `access` event for every call.

use lis_core::CommandError;
use lis_engine::{
    AuditEventChanges, AuditEventView, ChainVerification, Engine, GetAuditEventChangesInput,
    ListAuditEventsInput, VerifyAuditChainInput,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_audit_events(
    engine: State<'_, Engine>,
    input: ListAuditEventsInput,
) -> Result<Vec<AuditEventView>, CommandError> {
    engine.list_audit_events(input).await
}

#[tauri::command]
pub(crate) async fn get_audit_event_changes(
    engine: State<'_, Engine>,
    input: GetAuditEventChangesInput,
) -> Result<AuditEventChanges, CommandError> {
    engine.get_audit_event_changes(input).await
}

#[tauri::command]
pub(crate) async fn verify_audit_chain(
    engine: State<'_, Engine>,
    input: VerifyAuditChainInput,
) -> Result<ChainVerification, CommandError> {
    engine.verify_audit_chain(input).await
}
