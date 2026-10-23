//! Bounded material evidence reads for explicit operator history/incident opens.
use crate::{ContainerId, ExcursionView, SpecimenError};
use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MaterialHistoryEntry {
    pub id: String,
    pub kind: String,
    pub actor: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub actor_display: Option<String>,
    pub recorded_at: UtcTimestamp,
    pub description: String,
    pub reason: Option<String>,
    #[ts(type = "number | null")]
    pub causal_sequence: Option<i64>,
}

/// Append-only row ceilings freeze page membership, not historical causal ordering.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MaterialHistorySnapshot {
    pub max_move_row_id: String,
    pub max_hold_row_id: String,
}
fn ceiling(value: &str) -> Result<i64, SpecimenError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|n| *n >= 0)
        .ok_or_else(|| {
            crate::error::invalid(
                "snapshot",
                "invalid",
                "Refresh history to start a new page snapshot.",
            )
        })
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MaterialHistoryPage {
    pub snapshot: MaterialHistorySnapshot,
    pub moves: Vec<MaterialHistoryEntry>,
    pub holds: Vec<MaterialHistoryEntry>,
    pub disposal: Option<MaterialHistoryEntry>,
    pub depletion: Option<MaterialHistoryEntry>,
    pub has_more: bool,
}

/// Audited by the engine. Separate streams do not imply a fabricated global chronology.
pub fn get_container_history(
    conn: &Connection,
    container_id: ContainerId,
    offset: u32,
    snapshot: Option<MaterialHistorySnapshot>,
) -> Result<MaterialHistoryPage, SpecimenError> {
    if crate::repo::containers::container(conn, container_id)?.is_none() {
        return Err(SpecimenError::NotFound("container"));
    }
    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => MaterialHistorySnapshot {
            max_move_row_id: conn
                .query_row(
                    "SELECT coalesce(max(rowid),0) FROM spc_location_events WHERE container_id=?1",
                    [container_id],
                    |r| r.get::<_, i64>(0),
                )?
                .to_string(),
            max_hold_row_id: conn
                .query_row(
                    "SELECT coalesce(max(rowid),0) FROM spc_holds WHERE container_id=?1",
                    [container_id],
                    |r| r.get::<_, i64>(0),
                )?
                .to_string(),
        },
    };
    let move_ceiling = ceiling(&snapshot.max_move_row_id)?;
    let hold_ceiling = ceiling(&snapshot.max_hold_row_id)?;
    let mut moves = history_rows(
        conn,
        "SELECT e.id, 'move', e.moved_by, e.moved_at, l.code || ' — ' || l.name, e.reason, o.sequence FROM spc_location_events e JOIN spc_locations l ON l.id=e.location_id LEFT JOIN spc_location_order o ON o.event_id=e.id WHERE e.container_id=?1 AND e.rowid<=?3 ORDER BY o.sequence DESC, e.id LIMIT 51 OFFSET ?2",
        container_id,
        offset,
        move_ceiling,
    )?;
    let mut holds = history_rows(
        conn,
        "SELECT id, kind || ' hold: ' || state, placed_by, placed_at, 'Placed' || CASE WHEN released_at IS NOT NULL THEN '; Released ' || released_at || ' by ' || released_by || ': ' || release_reason ELSE '' END, reason, NULL FROM spc_holds WHERE container_id=?1 AND rowid<=?3 ORDER BY id DESC LIMIT 51 OFFSET ?2",
        container_id,
        offset,
        hold_ceiling,
    )?;
    let disposal = conn.query_row("SELECT container_id, 'disposal', disposed_by, disposed_at, 'Disposed', reason, NULL FROM spc_disposals WHERE container_id=?1", [container_id], history_row).optional()?;
    let has_more = moves.len() > PAGE_SIZE || holds.len() > PAGE_SIZE;
    moves.truncate(PAGE_SIZE);
    holds.truncate(PAGE_SIZE);
    let depletion = lis_audit::list(
        conn,
        &lis_audit::EventFilter {
            stream: lis_audit::Stream::Clinical,
            before_seq: None,
            limit: 1,
            action: Some(crate::actions::MARK_DEPLETED),
            record_type: Some("container"),
            record_id: Some(&container_id.to_string()),
            self_approvals_only: false,
        },
    )
    .map_err(lis_store::StoreError::from)?
    .into_iter()
    .next()
    .map(|event| MaterialHistoryEntry {
        id: event.seq.to_string(),
        kind: "depletion".to_owned(),
        actor: event.principal_id,
        actor_display: None,
        recorded_at: event.occurred_at,
        description: "Marked depleted".to_owned(),
        reason: event.reason,
        causal_sequence: Some(event.seq),
    });
    Ok(MaterialHistoryPage {
        snapshot,
        moves,
        holds,
        disposal,
        depletion,
        has_more,
    })
}

fn history_rows(
    conn: &Connection,
    sql: &str,
    id: ContainerId,
    offset: u32,
    ceiling: i64,
) -> rusqlite::Result<Vec<MaterialHistoryEntry>> {
    conn.prepare_cached(sql)?
        .query_map(params![id, offset, ceiling], history_row)?
        .collect()
}

fn history_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MaterialHistoryEntry> {
    Ok(MaterialHistoryEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        actor: row.get(2)?,
        actor_display: None,
        recorded_at: row.get(3)?,
        description: row.get(4)?,
        reason: row.get(5)?,
        causal_sequence: row.get(6)?,
    })
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExcursionPage {
    pub snapshot: String,
    pub items: Vec<ExcursionView>,
    pub has_more: bool,
}

/// Stable identity paging reaches open and assessed incidents after navigation/restart.
pub fn list_excursions(
    conn: &Connection,
    offset: u32,
    snapshot: Option<String>,
) -> Result<ExcursionPage, SpecimenError> {
    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => conn
            .query_row(
                "SELECT coalesce(max(rowid),0) FROM spc_excursions",
                [],
                |r| r.get::<_, i64>(0),
            )?
            .to_string(),
    };
    let maximum = ceiling(&snapshot)?;
    let mut ids = conn
        .prepare_cached(
            "SELECT id FROM spc_excursions WHERE rowid<=?2 ORDER BY id DESC LIMIT 51 OFFSET ?1",
        )?
        .query_map(params![offset, maximum], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<crate::ExcursionId>>>()?;
    let has_more = ids.len() > PAGE_SIZE;
    ids.truncate(PAGE_SIZE);
    let items = ids
        .into_iter()
        .map(|id| crate::queries::excursion(conn, id))
        .collect::<Result<_, _>>()?;
    Ok(ExcursionPage {
        snapshot,
        items,
        has_more,
    })
}
