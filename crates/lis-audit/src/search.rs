//! Finding audit records by readable values (REQ-OPS-001). Each owner module turns the
//! typed text into its own record IDs with [`RecordSearch::records`]; [`audit_records`]
//! keeps those that audit events name. The caller authorizes the search, restricts patient
//! records to users who may read them and records the search as an `access` event.

use lis_core::{UtcTimestamp, like_prefix};
use rusqlite::{Connection, params};

use crate::list::streams;
use crate::model::{AuditError, Stream};

/// A record type code and a record ID, as audit events store them.
pub type RecordRef = (&'static str, String);

/// Record types whose stored record ID is itself a readable value, matched by prefix.
const READABLE_ID_TYPES: &str = "app_version";

/// What a user typed in the audit trail's Find, for one or every record type.
#[derive(Debug, Clone, Copy)]
pub struct RecordSearch<'a> {
    /// The record type searched; `None` searches every type.
    pub record_type: Option<&'a str>,
    /// The start of a readable value, trimmed.
    pub text: &'a str,
    /// At most this many records of each type.
    pub limit: u32,
}

impl RecordSearch<'_> {
    /// Whether the search covers record type `owned`.
    #[must_use]
    pub fn covers(&self, owned: &str) -> bool {
        self.record_type
            .is_none_or(|record_type| record_type == owned)
    }

    /// Up to the limit of `owned` records, when the search covers it. `sql` selects their
    /// IDs, with `?1` the `LIKE` pattern for the text (`ESCAPE '\'`) and `?2` the limit.
    pub fn records(
        &self,
        conn: &Connection,
        owned: &'static str,
        sql: &str,
    ) -> rusqlite::Result<Vec<RecordRef>> {
        if !self.covers(owned) {
            return Ok(Vec::new());
        }
        conn.prepare_cached(sql)?
            .query_map(params![like_prefix(self.text), self.limit], |row| {
                Ok((owned, row.get(0)?))
            })?
            .collect()
    }
}

/// A record that audit events name, with where and when it last appears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Its record type code.
    pub record_type: String,
    /// Its record ID.
    pub record_id: String,
    /// The streams with events about it, in [`Stream::ALL`] order.
    pub streams: Vec<Stream>,
    /// When its latest event was appended.
    pub last_recorded_at: UtcTimestamp,
}

/// The records named by events in `stream` (`None`: every stream) among `candidates`,
/// plus those whose stored ID is `search.text` itself and, for types whose ID is readable,
/// starts with it; at most `limit`, by type and ID. One pass over `audit_events`.
pub fn audit_records(
    conn: &Connection,
    stream: Option<Stream>,
    search: &RecordSearch<'_>,
    candidates: &[RecordRef],
    limit: u32,
) -> Result<Vec<AuditRecord>, AuditError> {
    let candidates = serde_json::to_string(candidates)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let records = conn
        .prepare_cached(
            "SELECT record_type, record_id, group_concat(DISTINCT stream), max(recorded_at) \
             FROM audit_events \
             WHERE (?1 IS NULL OR stream = ?1) AND (?2 IS NULL OR record_type = ?2) \
             AND record_type IS NOT NULL AND (record_id = ?3 \
             OR (record_type = ?4 AND record_id LIKE ?5 ESCAPE '\\') \
             OR (record_type, record_id) IN (SELECT json_extract(value, '$[0]'), \
             json_extract(value, '$[1]') FROM json_each(?6))) \
             GROUP BY record_type, record_id ORDER BY record_type, record_id LIMIT ?7",
        )?
        .query_map(
            params![
                stream.map(Stream::as_str),
                search.record_type,
                search.text,
                READABLE_ID_TYPES,
                like_prefix(search.text),
                candidates,
                limit,
            ],
            |row| {
                Ok(AuditRecord {
                    record_type: row.get(0)?,
                    record_id: row.get(1)?,
                    streams: streams(&row.get::<_, String>(2)?),
                    last_recorded_at: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(records)
}
