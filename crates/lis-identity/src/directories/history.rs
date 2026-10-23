//! Typed immutable directory history and its exact-record query.
use crate::error::IdentityError;
use crate::model::{ClientId, ClientLocationId, PractitionerId};
use lis_core::UtcTimestamp;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A concrete directory record; an existing order keeps this identity after correction.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, TS)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
#[ts(export)]
pub enum DirectoryRecord {
    /// Client organization.
    Client(ClientId),
    /// Location within its original organization.
    Location(ClientLocationId),
    /// Ordering practitioner.
    Practitioner(PractitionerId),
}
impl DirectoryRecord {
    pub(super) fn key(self) -> (&'static str, String) {
        match self {
            Self::Client(id) => ("client", id.to_string()),
            Self::Location(id) => ("location", id.to_string()),
            Self::Practitioner(id) => ("practitioner", id.to_string()),
        }
    }
}
/// Saved facts from a single revision.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum DirectorySnapshot {
    /// Client facts.
    Client {
        code: String,
        name: String,
        active: bool,
    },
    /// Location facts; the parent organization cannot change.
    Location {
        code: String,
        name: String,
        active: bool,
    },
    /// Practitioner facts.
    Practitioner {
        npi: String,
        family_name: String,
        given_names: Option<String>,
        active: bool,
    },
}
/// One immutable revision. Legacy baseline attribution is explicitly unknown.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DirectoryHistoryEntry {
    /// Stable causal revision, independent of clock ordering.
    #[ts(type = "number")]
    pub revision: i64,
    /// Exact stored facts.
    pub snapshot: DirectorySnapshot,
    /// Original record time for legacy baselines, command time otherwise.
    pub recorded_at: UtcTimestamp,
    /// Acting user; absent only for migrated legacy baselines.
    pub recorded_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub recorded_by_display: Option<String>,
    /// Required explanation for correction/retirement.
    pub reason: Option<String>,
    /// `legacy_baseline`, `registered` or `corrected`.
    pub origin: String,
}
/// Request for the complete correction history of one selected directory record.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DirectoryHistoryInput {
    /// Existing record.
    pub record: DirectoryRecord,
}

/// Reads revisions in causal order without discarding retired records.
pub fn directory_history(
    conn: &Connection,
    input: DirectoryHistoryInput,
) -> Result<Vec<DirectoryHistoryEntry>, IdentityError> {
    let (kind, id) = input.record.key();
    let rows=conn.prepare_cached("SELECT revision,code,name,given_names,active,recorded_at,recorded_by,reason,origin FROM idn_directory_history WHERE record_kind=?1 AND record_id=?2 ORDER BY revision")?.query_map(params![kind,id],|row| {
        let code:String=row.get(1)?; let name:String=row.get(2)?; let active:bool=row.get(4)?;
        let snapshot=match input.record {
            DirectoryRecord::Client(_)=>DirectorySnapshot::Client{code,name,active},
            DirectoryRecord::Location(_)=>DirectorySnapshot::Location{code,name,active},
            DirectoryRecord::Practitioner(_)=>DirectorySnapshot::Practitioner{npi:code,family_name:name,given_names:row.get(3)?,active},
        };
        Ok(DirectoryHistoryEntry{revision:row.get(0)?,snapshot,recorded_at:row.get(5)?,recorded_by:row.get(6)?,recorded_by_display:None,reason:row.get(7)?,origin:row.get(8)?})
    })?.collect::<Result<Vec<_>,_>>()?;
    if rows.is_empty() {
        return Err(IdentityError::NotFound("directory record"));
    }
    Ok(rows)
}
