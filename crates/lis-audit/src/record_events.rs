//! Read-only lookup of audit positions for domain-owned causal records.
//!
//! Safety decisions compare these sequence numbers instead of host-clock
//! timestamps. A clinical stream has one writer and therefore gives every
//! committed clinical change a total order (NFR-01, REQ-RPT-002).

use rusqlite::{Connection, params};

use crate::Stream;

/// The position and aggregate revision recorded by a succeeded event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordEvent {
    /// The event's position in its audit stream.
    pub seq: i64,
    /// The aggregate revision, when the command recorded one.
    pub new_revision: Option<i64>,
}

/// Succeeded events for one action and record, oldest first.
///
/// This is an owner query over the audit trail. Domain modules use it to
/// associate their append-only causal records with commit order without
/// exposing `audit_events` SQL to consumers.
pub fn succeeded_record_events(
    conn: &Connection,
    stream: Stream,
    action: &str,
    record_type: &str,
    record_id: &str,
) -> rusqlite::Result<Vec<RecordEvent>> {
    conn.prepare_cached(
        "SELECT seq, new_revision FROM audit_events \
         WHERE stream = ?1 AND action = ?2 AND outcome = 'succeeded' \
           AND record_type = ?3 AND record_id = ?4 ORDER BY seq",
    )?
    .query_map(
        params![stream.as_str(), action, record_type, record_id],
        |row| {
            Ok(RecordEvent {
                seq: row.get(0)?,
                new_revision: row.get(1)?,
            })
        },
    )?
    .collect()
}
