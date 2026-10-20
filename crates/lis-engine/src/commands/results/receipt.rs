//! Exact retained-receipt import reconciliation.
use super::{CommandError, CommandSpec, Engine, ImportAttemptSummary, REVIEW, access, spec};
use serde::Deserialize;
use ts_rs::TS;

/// Exact receipt reconciliation, including old duplicates and asynchronous processing.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetReceiptImportAttemptInput {
    pub receipt_id: lis_interfaces::ReceiptId,
}
pub(crate) const GET_RECEIPT_IMPORT_ATTEMPT: CommandSpec = spec(
    "get_receipt_import_attempt",
    "result.get_receipt_import_attempt",
    REVIEW,
);
impl Engine {
    pub async fn get_receipt_import_attempt(
        &self,
        input: GetReceiptImportAttemptInput,
    ) -> Result<Option<ImportAttemptSummary>, CommandError> {
        self.run(&GET_RECEIPT_IMPORT_ATTEMPT, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_results::latest_import_attempt_for_receipt(conn, input.receipt_id),
                move |_| {
                    access(GET_RECEIPT_IMPORT_ATTEMPT.action).record("receipt", input.receipt_id)
                },
            )
            .await
        })
        .await
    }
}
