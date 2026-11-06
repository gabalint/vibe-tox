//! An import attempt's quarantined rows, with where each one can be fixed.

use lis_analytical::RunId;

use super::*;
use crate::model::QuarantineReason;
use crate::views::MatchingRunView;

/// The rows `attempt_id` held, each with the run and specimen its position
/// names (through the published position and container views) and, for a
/// label more than one sample matches, the open runs of `instrument_id` that
/// hold that label now.
pub(super) fn quarantine_items(
    conn: &Connection,
    attempt_id: ImportAttemptId,
    instrument_id: InstrumentId,
) -> Result<Vec<QuarantineItemView>, ResultError> {
    let mut items: Vec<QuarantineItemView> = conn
        .prepare_cached(
            "SELECT q.id, q.record_path, q.sample_label, q.source_test_code, q.value_text, \
             q.units_text, q.reason, q.state, q.revision, p.run_id, c.specimen_id \
             FROM res_quarantine_items q \
             LEFT JOIN ana_v_positions p ON p.position_id = q.position_id \
             LEFT JOIN spc_v_containers c ON c.container_id = p.container_id \
             WHERE q.import_attempt_id = ?1 ORDER BY q.record_path, q.id",
        )?
        .query_map([attempt_id], |row| {
            Ok(QuarantineItemView {
                item_id: row.get(0)?,
                record_path: row.get(1)?,
                sample_label: row.get(2)?,
                test_code: row.get(3)?,
                value_text: row.get(4)?,
                units_text: row.get(5)?,
                reason: row.get(6)?,
                state: row.get(7)?,
                revision: row.get(8)?,
                run_id: row.get(9)?,
                specimen_id: row.get(10)?,
                matching_runs: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    for item in &mut items {
        if item.reason != QuarantineReason::AmbiguousSample {
            continue;
        }
        if let Some(label) = item.sample_label.as_deref() {
            item.matching_runs = open_runs_with_label(conn, instrument_id, label)?;
        }
    }
    Ok(items)
}

/// The open runs of `instrument_id` with a position labelled `label`, oldest first: the
/// same match the importer makes, so these are the runs that made the label ambiguous.
fn open_runs_with_label(
    conn: &Connection,
    instrument_id: InstrumentId,
    label: &str,
) -> Result<Vec<MatchingRunView>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT run_id, run_created_at FROM ana_v_positions \
             WHERE instrument_id = ?1 AND sample_label = ?2 AND run_state = 'open' \
             ORDER BY run_created_at, run_id",
        )?
        .query_map(params![instrument_id, label], |row| {
            Ok(MatchingRunView {
                run_id: row.get::<_, RunId>(0)?,
                created_at: row.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}
