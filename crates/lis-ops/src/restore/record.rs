//! Recording an applied restore in the restored database (M4 design brief §3.7, "after
//! the open"): one transaction inserts the restore and its heads, raises the number
//! floors through their owner and audits `ops.restore_applied`, which opens the recovery
//! reconciliation. The marker is removed only after that commits; if recording fails,
//! the engine stops startup (`restore_record_failed`), so the gate is never missing.
//!
//! The event carries `restore_digest`, a SHA-256 over what lets a restore explain anchors:
//! when it was applied, the set it restored and every head it restored. [`restore_records`]
//! returns only records bound to their own event (hash, action, outcome, record identity
//! and that digest), so a record edited after the fact explains nothing; once the event is
//! covered by a matched anchor, the binding is as strong as the anchor (NFR-18, ADR 0008).

use std::path::Path;

use lis_audit::{AuditDetail, AuditEvent, ChainLink, Outcome, Stream};
use lis_core::UtcTimestamp;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::anchors::from_counts;
use super::apply::{MarkerState, RestoreId, RestoreMarker, pending_restore, remove_staging};
use super::test_runs::set_id_detail;
use crate::anchors::RestoreRecord;
use crate::backup::BackupError;

/// Audit action of a restore recorded in the restored database.
pub const RESTORE_APPLIED: &str = "ops.restore_applied";
/// The system principal that records it.
pub const RESTORE_COMPONENT: &str = "restore";
/// Record type of the restore event.
const RESTORE_RECORD: &str = "restore";
/// Details key of the digest that binds a restore record to its event.
const RESTORE_DIGEST: &str = "restore_digest";
/// Version tag of the digest input.
const RESTORE_DIGEST_FORMAT: &str = "vibe-tox-restore-digest-1";

/// The number floors the specimens owner set after a restore (§3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreFloors {
    /// The accession number floor.
    pub accession: String,
    /// The container code floor.
    pub container: String,
}

/// The applied restore waiting to be recorded, if any; read before the write.
pub fn applied_restore(data_dir: &Path) -> Result<Option<RestoreMarker>, BackupError> {
    Ok(pending_restore(data_dir)?
        .filter(|pending| pending.state == MarkerState::Applied)
        .map(|pending| pending.marker))
}

/// Records `marker`'s restore (`ops.restore_applied`, system principal `restore`).
/// `raise_floors` raises the accession and container floors through the specimens
/// owner in the same transaction (`policy::RESTORE_ACCESSION_SKIP`,
/// `policy::RESTORE_CONTAINER_SKIP`). A restore already recorded is not recorded again.
pub fn record_applied_restore<E: From<BackupError>>(
    ctx: &mut CommandContext<'_>,
    marker: &RestoreMarker,
    raise_floors: impl FnOnce(&mut CommandContext<'_>) -> Result<RestoreFloors, E>,
) -> Result<RestoreId, E> {
    let recorded = ctx
        .tx()
        .prepare_cached("SELECT 1 FROM ops_restores WHERE id = ?1")
        .and_then(|mut statement| statement.exists([marker.restore_id]))
        .map_err(BackupError::from)?;
    if recorded {
        return Ok(marker.restore_id);
    }
    let applied_at = marker.applied_at.ok_or(BackupError::RestoreNotVerified)?;
    let floors = raise_floors(ctx)?;
    let heads: Vec<StoredHead> = marker
        .heads
        .iter()
        .map(|head| (head.stream.clone(), head.seq, head.hash.clone()))
        .collect();
    let digest = restore_digest(applied_at, &marker.set_id, &marker.snapshot_sha256, &heads);
    let event = ctx
        .audit(applied_event(marker, &digest)?)
        .map_err(BackupError::from)?;
    insert(ctx, marker, applied_at, &floors, &event)?;
    Ok(marker.restore_id)
}

fn insert(
    ctx: &CommandContext<'_>,
    marker: &RestoreMarker,
    applied_at: UtcTimestamp,
    floors: &RestoreFloors,
    event: &ChainLink,
) -> Result<(), BackupError> {
    let [
        compared,
        matched,
        lost,
        unverified,
        mismatched,
        missing,
        conflicts,
        gaps,
        unanchored,
    ] = marker
        .anchor_counts
        .map(|count| i64::try_from(count).unwrap_or(i64::MAX));
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_restores (id, set_id, set_created_at, set_kind, snapshot_sha256, \
             backup_app_version, staged_at, applied_at, recorded_at, replaced_database_sha256, \
             anchors_compared, anchors_matched, anchors_lost_interval, anchors_unverified, \
             anchors_mismatched, anchors_missing, anchor_conflicts, anchor_gaps, \
             anchors_unanchored, applied_event_seq, applied_event_hash, accession_floor, \
             container_floor) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
             ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        )?
        .execute(params![
            marker.restore_id,
            marker.set_id,
            marker.set_created_at,
            marker.set_kind,
            marker.snapshot_sha256,
            marker.backup_app_version,
            marker.staged_at,
            applied_at,
            ctx.now(),
            marker.replaced_database_sha256,
            compared,
            matched,
            lost,
            unverified,
            mismatched,
            missing,
            conflicts,
            gaps,
            unanchored,
            event.seq,
            event.hash,
            floors.accession,
            floors.container,
        ])?;
    let mut heads = ctx.tx().prepare_cached(
        "INSERT INTO ops_restore_heads (restore_id, stream, seq, hash) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for head in &marker.heads {
        heads.execute(params![marker.restore_id, head.stream, head.seq, head.hash])?;
    }
    Ok(())
}

fn applied_event(marker: &RestoreMarker, digest: &str) -> Result<AuditEvent, BackupError> {
    let anchors = from_counts(marker.anchor_counts);
    Ok(
        AuditEvent::new(Stream::Security, RESTORE_APPLIED, Outcome::Succeeded)
            .record(RESTORE_RECORD, marker.restore_id)
            .new_revision(1)
            .detail("set_id", set_id_detail(&marker.set_id)?)
            .detail(
                "snapshot_sha256",
                AuditDetail::sha256(&marker.snapshot_sha256)?,
            )
            .detail("anchors_compared", AuditDetail::unsigned(anchors.compared))
            .detail("anchors_matched", AuditDetail::unsigned(anchors.matched))
            .detail(
                "anchors_lost_interval",
                AuditDetail::unsigned(anchors.lost_interval),
            )
            .detail(
                "anchors_unexplained",
                AuditDetail::unsigned(
                    anchors.mismatched
                        + anchors.local_events_missing
                        + anchors.conflicts
                        + anchors.unverified_restore_boundary,
                ),
            )
            .detail(
                "replaced_database",
                AuditDetail::boolean(marker.replaced_database_sha256.is_some()),
            )
            .detail(RESTORE_DIGEST, AuditDetail::sha256(digest)?),
    )
}

/// A stored restored head: stream code, sequence number and hash.
type StoredHead = (String, i64, String);

/// SHA-256 (hex) over the canonical JSON array of the format tag, `applied_at`, the set
/// ID, the snapshot digest and the heads sorted by stream.
fn restore_digest(
    applied_at: UtcTimestamp,
    set_id: &str,
    snapshot_sha256: &str,
    heads: &[StoredHead],
) -> String {
    let mut heads = heads.to_vec();
    heads.sort();
    let input = serde_json::json!([
        RESTORE_DIGEST_FORMAT,
        applied_at.to_string(),
        set_id,
        snapshot_sha256,
        heads,
    ]);
    hex::encode(Sha256::digest(input.to_string()))
}

/// Every restore this database records that is bound to its own `ops.restore_applied`
/// event, for explaining anchors (§4): when it was applied, the heads it restored and the
/// event. A record that does not match its event is left out and logged, so it explains
/// nothing.
pub fn restore_records(conn: &Connection) -> Result<Vec<RestoreRecord>, BackupError> {
    let rows = conn
        .prepare_cached(
            "SELECT id, applied_at, set_id, snapshot_sha256, applied_event_seq, \
             applied_event_hash FROM ops_restores ORDER BY applied_at, id",
        )?
        .query_map([], |row| {
            Ok(RestoreRow {
                id: row.get(0)?,
                applied_at: row.get(1)?,
                set_id: row.get(2)?,
                snapshot_sha256: row.get(3)?,
                event: ChainLink {
                    stream: Stream::Security,
                    seq: row.get(4)?,
                    hash: row.get(5)?,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let heads = stored_heads(conn, row.id)?;
        let digest = restore_digest(row.applied_at, &row.set_id, &row.snapshot_sha256, &heads);
        if !bound_to_event(conn, &row, &digest)? {
            tracing::warn!(restore_id = %row.id, "restore record does not match its audit event");
            continue;
        }
        records.push(RestoreRecord {
            applied_at: row.applied_at,
            restored_heads: heads
                .into_iter()
                .filter_map(|(stream, seq, hash)| {
                    super::stream_named(&stream).map(|stream| ChainLink { stream, seq, hash })
                })
                .collect(),
            applied_event: row.event,
        });
    }
    Ok(records)
}

/// One `ops_restores` row, as far as explaining anchors needs it.
struct RestoreRow {
    id: RestoreId,
    applied_at: UtcTimestamp,
    set_id: String,
    snapshot_sha256: String,
    event: ChainLink,
}

fn stored_heads(conn: &Connection, id: RestoreId) -> Result<Vec<StoredHead>, BackupError> {
    Ok(conn
        .prepare_cached("SELECT stream, seq, hash FROM ops_restore_heads WHERE restore_id = ?1")?
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// True when the security event at the record's position is this restore's succeeded
/// `ops.restore_applied` event, with the recorded hash and `digest` in its details.
fn bound_to_event(conn: &Connection, row: &RestoreRow, digest: &str) -> Result<bool, BackupError> {
    // Deviation (coding standards, table ownership): `lis-audit` publishes no read of one
    // event's hash and details yet; this read-only query moves there when it does.
    let event = conn
        .prepare_cached(
            "SELECT hash, action, outcome, record_type, record_id, details FROM audit_events \
             WHERE stream = 'security' AND seq = ?1",
        )?
        .query_row([row.event.seq], |event| {
            Ok((
                event.get::<_, String>(0)?,
                event.get::<_, String>(1)?,
                event.get::<_, String>(2)?,
                event.get::<_, Option<String>>(3)?,
                event.get::<_, Option<String>>(4)?,
                event.get::<_, String>(5)?,
            ))
        })
        .optional()?;
    let Some((hash, action, outcome, record_type, record_id, details)) = event else {
        return Ok(false);
    };
    let carried = match serde_json::from_str::<Value>(&details) {
        Ok(details) => details.get(RESTORE_DIGEST).and_then(Value::as_str) == Some(digest),
        Err(_) => false,
    };
    Ok(carried
        && hash == row.event.hash
        && action == RESTORE_APPLIED
        && outcome == "succeeded"
        && record_type.as_deref() == Some(RESTORE_RECORD)
        && record_id == Some(row.id.to_string()))
}

/// Removes the staging folder and its marker once the restore is recorded; the engine
/// calls it after the recording transaction commits.
pub fn finish_recorded_restore(data_dir: &Path) -> Result<(), BackupError> {
    Ok(remove_staging(data_dir)?)
}
