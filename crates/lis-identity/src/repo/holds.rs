//! SQL for identity holds and their candidate subjects.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{ActorRef, IdentityHoldId, SubjectId};
use crate::views::{HoldKind, IdentityHoldRecord, IdentityHoldView};

/// The state a hold decision depends on.
pub(crate) struct HoldState {
    pub(crate) subject_id: SubjectId,
    pub(crate) revision: i64,
    pub(crate) open: bool,
}

pub(crate) fn insert_hold(
    conn: &Connection,
    id: IdentityHoldId,
    subject_id: SubjectId,
    subject_revision: i64,
    kind: HoldKind,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_identity_holds (id, subject_id, subject_revision, kind, status, revision, opened_at) \
         VALUES (?1, ?2, ?3, ?4, 'open', 1, ?5)",
    )?
    .execute(params![id, subject_id, subject_revision, kind.as_str(), now])?;
    Ok(())
}

pub(crate) fn insert_candidate(
    conn: &Connection,
    hold_id: IdentityHoldId,
    subject_id: SubjectId,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_identity_hold_candidates (hold_id, subject_id, subject_revision) \
         SELECT ?1, id, revision FROM idn_subjects WHERE id = ?2",
    )?
    .execute(params![hold_id, subject_id])?;
    Ok(())
}

/// Whether every participant still has the revision captured when the hold opened.
pub(crate) fn evidence_is_current(
    conn: &Connection,
    hold_id: IdentityHoldId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM idn_identity_holds h \
         JOIN idn_subjects s ON s.id = h.subject_id \
         WHERE h.id = ?1 AND h.subject_revision = s.revision \
         AND NOT EXISTS (SELECT 1 FROM idn_identity_hold_candidates c \
         JOIN idn_subjects cs ON cs.id = c.subject_id \
         WHERE c.hold_id = h.id AND c.subject_revision <> cs.revision))",
    )?
    .query_row([hold_id], |row| row.get(0))
}

pub(crate) fn hold_state(
    conn: &Connection,
    id: IdentityHoldId,
) -> rusqlite::Result<Option<HoldState>> {
    conn.prepare_cached(
        "SELECT subject_id, revision, status = 'open' FROM idn_identity_holds WHERE id = ?1",
    )?
    .query_row([id], |row| {
        Ok(HoldState {
            subject_id: row.get(0)?,
            revision: row.get(1)?,
            open: row.get(2)?,
        })
    })
    .optional()
}

/// Resolves an open hold as "confirmed distinct".
pub(crate) fn resolve_confirmed_distinct(
    conn: &Connection,
    id: IdentityHoldId,
    revision: i64,
    now: UtcTimestamp,
    reason: &str,
    actor: &ActorRef,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE idn_identity_holds SET status = 'resolved', resolution = 'confirmed_distinct', \
         revision = ?2, resolved_at = ?3, resolution_reason = ?4, resolved_by_kind = ?5, \
         resolved_by_id = ?6 WHERE id = ?1",
    )?
    .execute(params![id, revision, now, reason, actor.kind, actor.id])?;
    Ok(())
}

/// Open holds whose subject or candidate is `subject_id` (every open hold
/// when `None`), oldest first and at most `limit` when given, with empty
/// candidate lists (see [`candidates`]).
pub(crate) fn open_holds_touching(
    conn: &Connection,
    subject_id: Option<SubjectId>,
    limit: Option<u32>,
) -> rusqlite::Result<Vec<IdentityHoldView>> {
    conn.prepare_cached(
        "SELECT id, revision, kind, subject_id, opened_at FROM idn_identity_holds \
         WHERE status = 'open' AND (?1 IS NULL OR subject_id = ?1 OR id IN \
         (SELECT hold_id FROM idn_identity_hold_candidates WHERE subject_id = ?1)) \
         ORDER BY opened_at, id LIMIT coalesce(?2, -1)",
    )?
    .query_map(rusqlite::params![subject_id, limit], |row| {
        Ok(IdentityHoldView {
            id: row.get(0)?,
            revision: row.get(1)?,
            kind: row.get(2)?,
            subject_id: row.get(3)?,
            candidate_subject_ids: Vec::new(),
            opened_at: row.get(4)?,
        })
    })?
    .collect()
}

/// Every hold, open or resolved, whose subject or candidate is
/// `subject_id`, oldest first, with empty candidate lists.
pub(crate) fn holds_touching(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<Vec<IdentityHoldRecord>> {
    conn.prepare_cached(
        "SELECT id, revision, kind, subject_id, opened_at, resolved_at FROM idn_identity_holds \
         WHERE subject_id = ?1 OR id IN \
         (SELECT hold_id FROM idn_identity_hold_candidates WHERE subject_id = ?1) \
         ORDER BY opened_at, id",
    )?
    .query_map([subject_id], |row| {
        Ok(IdentityHoldRecord {
            hold: IdentityHoldView {
                id: row.get(0)?,
                revision: row.get(1)?,
                kind: row.get(2)?,
                subject_id: row.get(3)?,
                candidate_subject_ids: Vec::new(),
                opened_at: row.get(4)?,
            },
            resolved_at: row.get(5)?,
        })
    })?
    .collect()
}

pub(crate) fn candidates(
    conn: &Connection,
    hold_id: IdentityHoldId,
) -> rusqlite::Result<Vec<SubjectId>> {
    conn.prepare_cached(
        "SELECT subject_id FROM idn_identity_hold_candidates WHERE hold_id = ?1 \
         ORDER BY subject_id",
    )?
    .query_map([hold_id], |row| row.get(0))?
    .collect()
}
