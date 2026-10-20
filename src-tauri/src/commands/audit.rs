//! Audit trail commands (M1 IPC contract; capability `view_audit`). The engine
//! records an `access` event for every call.

use lis_core::CommandError;
use lis_engine::{
    AuditEventView, ChainVerification, Engine, ListAuditEventsInput, VerifyAuditChainInput,
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
pub(crate) async fn verify_audit_chain(
    engine: State<'_, Engine>,
    input: VerifyAuditChainInput,
) -> Result<ChainVerification, CommandError> {
    engine.verify_audit_chain(input).await
}
