//! The Result imports list: instrument, outcome, received-period and search filters
//! applied in the query (REQ-ANA-003).
use lis_audit::AuditDetail;
use lis_core::{InstrumentId, UtcTimestamp};
use lis_results::{ImportAttemptFilter, ImportAttemptId, ImportOutcome};
use serde::Deserialize;
use ts_rs::TS;

use super::{CommandError, Engine, ImportAttemptSummary, LIST_IMPORT_ATTEMPTS, access};

/// One page of the import list. Absent filters show every attempt.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListImportAttemptsInput {
    pub instrument_id: Option<InstrumentId>,
    pub limit: u32,
    pub before: Option<UtcTimestamp>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after_import_attempt_id: Option<ImportAttemptId>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub outcome: Option<ImportOutcome>,
    /// Receipts that began to arrive at or after this instant.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub received_from: Option<UtcTimestamp>,
    /// Receipts that began to arrive before this instant.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub received_before: Option<UtcTimestamp>,
    /// Text in a sample label, container label, accession number or file name.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub search: Option<String>,
}

impl Engine {
    /// Counters only. A search names a sample, container, accession or file, so
    /// that read is recorded in the access stream, without the searched text.
    pub async fn list_import_attempts(
        &self,
        input: ListImportAttemptsInput,
    ) -> Result<Vec<ImportAttemptSummary>, CommandError> {
        self.run(&LIST_IMPORT_ATTEMPTS, |call| async move {
            let searched = input
                .search
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty());
            let filter = ImportAttemptFilter {
                instrument_id: input.instrument_id,
                outcome: input.outcome,
                received_from: input.received_from,
                received_before: input.received_before,
                search: input.search,
            };
            let page = move |conn: &rusqlite::Connection| {
                lis_results::list_import_attempts_page(
                    conn,
                    &filter,
                    input.limit,
                    input.before,
                    input.after_import_attempt_id,
                )
            };
            if !searched {
                return self.read(&call, page).await;
            }
            self.audited_read(&call, page, |rows: &Vec<ImportAttemptSummary>| {
                access(LIST_IMPORT_ATTEMPTS.action)
                    .detail("search", AuditDetail::boolean(true))
                    .detail("returned", AuditDetail::count(rows.len()))
            })
            .await
        })
        .await
    }
}
