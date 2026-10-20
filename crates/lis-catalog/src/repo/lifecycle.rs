//! Lifecycle SQL shared by `cat_method_versions` and `cat_orderable_versions`,
//! which have the same lifecycle columns. Table names come from
//! [`Versioned`] constants, never from input; values are always bound.

use lis_core::{Id, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::lifecycle::{VersionLifecycle, VersionSummary, Versioned};

/// The lifecycle columns, in the order [`lifecycle_at`] reads them.
/// Queries alias the version table as `v`.
pub(crate) const LIFECYCLE_COLUMNS: &str = "v.status, v.revision, v.created_by, v.created_at, \
     v.approved_by, v.effective_from, v.effective_to, v.replaced_by_id, v.retired_by, \
     v.retired_at, v.retire_reason";

/// Reads [`LIFECYCLE_COLUMNS`] starting at column `start`.
pub(crate) fn lifecycle_at<K>(
    row: &Row<'_>,
    start: usize,
) -> rusqlite::Result<VersionLifecycle<K>> {
    Ok(VersionLifecycle {
        status: row.get(start)?,
        revision: row.get(start + 1)?,
        created_by: row.get(start + 2)?,
        created_by_display: None,
        created_at: row.get(start + 3)?,
        approved_by: row.get(start + 4)?,
        approved_by_display: None,
        effective_from: row.get(start + 5)?,
        effective_to: row.get(start + 6)?,
        replaced_by: row.get(start + 7)?,
        retired_by: row.get(start + 8)?,
        retired_at: row.get(start + 9)?,
        retire_reason: row.get(start + 10)?,
    })
}

fn summary_at<K>(row: &Row<'_>) -> rusqlite::Result<VersionSummary<K>> {
    Ok(VersionSummary {
        id: row.get(0)?,
        version: row.get(1)?,
        lifecycle: lifecycle_at(row, 2)?,
    })
}

pub(crate) fn parent_exists<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> rusqlite::Result<bool> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE id = ?1)",
        K::PARENT_TABLE
    );
    conn.prepare_cached(&sql)?
        .query_row([parent], |row| row.get(0))
}

/// A stored version with its parent.
pub(crate) struct StoredVersion<K: Versioned> {
    pub(crate) parent: Id<K::Parent>,
    pub(crate) version: VersionSummary<K>,
}

pub(crate) fn version_state<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
) -> rusqlite::Result<Option<StoredVersion<K>>> {
    let sql = format!(
        "SELECT v.id, v.version, {LIFECYCLE_COLUMNS}, v.{} FROM {} v WHERE v.id = ?1",
        K::PARENT_COLUMN,
        K::TABLE
    );
    conn.prepare_cached(&sql)?
        .query_row([id], |row| {
            Ok(StoredVersion {
                parent: row.get(13)?,
                version: summary_at(row)?,
            })
        })
        .optional()
}

/// Every version of `parent`, oldest first.
pub(crate) fn history<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> rusqlite::Result<Vec<VersionSummary<K>>> {
    let sql = format!(
        "SELECT v.id, v.version, {LIFECYCLE_COLUMNS} FROM {} v WHERE v.{} = ?1 \
         ORDER BY v.version",
        K::TABLE,
        K::PARENT_COLUMN
    );
    conn.prepare_cached(&sql)?
        .query_map([parent], summary_at)?
        .collect()
}

/// The version in effect, for a version table aliased `v`: approved, with its
/// effective period still open. The period starts at approval (never in the
/// future) and ends when a newer version is approved or this one is retired.
/// The `*_one_current` unique indexes allow at most one per parent.
const IN_EFFECT: &str = "v.status = 'approved' AND v.effective_to IS NULL";

/// The approved version of `parent` whose effective period is open.
pub(crate) fn current<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> rusqlite::Result<Option<VersionSummary<K>>> {
    let sql = format!(
        "SELECT v.id, v.version, {LIFECYCLE_COLUMNS} FROM {} v WHERE v.{} = ?1 AND {IN_EFFECT}",
        K::TABLE,
        K::PARENT_COLUMN
    );
    conn.prepare_cached(&sql)?
        .query_row([parent], summary_at)
        .optional()
}

/// Two select columns for a parent row aliased `p`: the number of its version
/// in effect, then the number of its open draft, each NULL when there is none.
/// The unique indexes allow at most one of each, so each subquery returns at
/// most one row and the directory stays one query.
pub(crate) fn version_state_columns<K: Versioned>() -> String {
    format!(
        "(SELECT v.version FROM {table} v WHERE v.{parent} = p.id AND {IN_EFFECT}), \
         (SELECT v.version FROM {table} v WHERE v.{parent} = p.id AND v.status = 'draft')",
        table = K::TABLE,
        parent = K::PARENT_COLUMN
    )
}

pub(crate) fn has_draft<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> rusqlite::Result<bool> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE {} = ?1 AND status = 'draft')",
        K::TABLE,
        K::PARENT_COLUMN
    );
    conn.prepare_cached(&sql)?
        .query_row([parent], |row| row.get(0))
}

pub(crate) fn next_version<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> rusqlite::Result<i64> {
    let sql = format!(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM {} WHERE {} = ?1",
        K::TABLE,
        K::PARENT_COLUMN
    );
    conn.prepare_cached(&sql)?
        .query_row([parent], |row| row.get(0))
}

pub(crate) fn set_revision<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
    revision: i64,
) -> rusqlite::Result<()> {
    let sql = format!("UPDATE {} SET revision = ?2 WHERE id = ?1", K::TABLE);
    conn.prepare_cached(&sql)?.execute(params![id, revision])?;
    Ok(())
}

/// Ends the effective period of `id` because `replaced_by` was approved.
pub(crate) fn supersede<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
    revision: i64,
    now: UtcTimestamp,
    replaced_by: Id<K>,
) -> rusqlite::Result<()> {
    let sql = format!(
        "UPDATE {} SET revision = ?2, effective_to = ?3, replaced_by_id = ?4 WHERE id = ?1",
        K::TABLE
    );
    conn.prepare_cached(&sql)?
        .execute(params![id, revision, now, replaced_by])?;
    Ok(())
}

pub(crate) fn approve<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
    revision: i64,
    approver: UserId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    let sql = format!(
        "UPDATE {} SET status = 'approved', revision = ?2, approved_by = ?3, \
         effective_from = ?4 WHERE id = ?1",
        K::TABLE
    );
    conn.prepare_cached(&sql)?
        .execute(params![id, revision, approver, now])?;
    Ok(())
}

/// Retires `id`, closing its effective period if it is still open.
pub(crate) fn retire<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
    revision: i64,
    actor: UserId,
    now: UtcTimestamp,
    reason: &str,
) -> rusqlite::Result<()> {
    let sql = format!(
        "UPDATE {} SET status = 'retired', revision = ?2, retired_by = ?3, retired_at = ?4, \
         retire_reason = ?5, effective_to = COALESCE(effective_to, ?4) WHERE id = ?1",
        K::TABLE
    );
    conn.prepare_cached(&sql)?
        .execute(params![id, revision, actor, now, reason])?;
    Ok(())
}
