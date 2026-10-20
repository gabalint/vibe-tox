//! Run directory queries, including stable bounded continuation.
use crate::model::RunState;
use crate::views::RunSummary;
use lis_core::InstrumentId;
use rusqlite::{Connection, params};

pub(crate) fn runs(
    conn: &Connection,
    state: Option<RunState>,
    instrument_id: Option<InstrumentId>,
) -> rusqlite::Result<Vec<RunSummary>> {
    runs_page(
        conn,
        &crate::ListRunsPageInput {
            state,
            instrument_id,
            method_version_id: None,
            after_run_id: None,
            limit: 500,
        },
    )
}

pub(crate) fn runs_page(
    conn: &Connection,
    input: &crate::ListRunsPageInput,
) -> rusqlite::Result<Vec<RunSummary>> {
    conn.prepare_cached(
        "SELECT r.id, r.instrument_id, r.method_version_id, r.state, r.created_at, \
         (SELECT count(*) FROM ana_run_positions p WHERE p.run_id = r.id) FROM ana_runs r \
         WHERE (?1 IS NULL OR r.state = ?1) AND (?2 IS NULL OR r.method_version_id = ?2) \
         AND (?3 IS NULL OR (r.created_at, r.id) < (SELECT created_at, id FROM ana_runs WHERE id = ?3)) \
         AND (?5 IS NULL OR r.instrument_id = ?5) \
         ORDER BY r.created_at DESC, r.id DESC LIMIT ?4",
    )?.query_map(params![input.state, input.method_version_id, input.after_run_id, input.limit, input.instrument_id], |row| Ok(RunSummary {
        run_id: row.get(0)?, instrument_id: row.get(1)?, method_version_id: row.get(2)?,
        state: row.get(3)?, created_at: row.get(4)?, position_count: row.get(5)?,
    }))?.collect()
}
