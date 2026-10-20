//! SQL for `ops_restore_tests` and `ops_restore_test_findings`, and the list query.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::anchors::{counts, summary_at};
use super::findings::{FindingCode, Findings};
use super::stage::StagedRestore;
use super::test_runs::{RestoreTestId, TestFailure, TestState};
use crate::anchors::AnchorSummary;
use crate::backup::BackupError;
use crate::targets::TargetId;

/// A test waiting for the job runner.
pub(super) struct Request {
    pub(super) id: RestoreTestId,
    pub(super) target_id: TargetId,
    pub(super) set_id: Option<String>,
}

pub(super) fn oldest_request(conn: &Connection) -> rusqlite::Result<Option<Request>> {
    conn.prepare_cached(
        "SELECT id, target_id, set_id FROM ops_restore_tests WHERE state = 'requested' \
         ORDER BY requested_at, id LIMIT 1",
    )?
    .query_row([], |row| {
        Ok(Request {
            id: row.get(0)?,
            target_id: row.get(1)?,
            set_id: row.get(2)?,
        })
    })
    .optional()
}

pub(super) fn insert_request(
    conn: &Connection,
    id: RestoreTestId,
    target_id: TargetId,
    set_id: Option<&str>,
    requested_by: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ops_restore_tests (id, target_id, set_id, state, requested_by, \
         requested_at, revision) VALUES (?1, ?2, ?3, 'requested', ?4, ?5, 1)",
    )?
    .execute(params![id, target_id, set_id, requested_by, now])?;
    Ok(())
}

/// Moves a requested test to `running`; returns its new revision.
pub(super) fn start(
    conn: &Connection,
    id: RestoreTestId,
    now: UtcTimestamp,
) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "UPDATE ops_restore_tests SET state = 'running', started_at = ?2, \
         revision = revision + 1 WHERE id = ?1 RETURNING revision",
    )?
    .query_row(params![id, now], |row| row.get(0))
}

pub(super) fn insert_finding(
    conn: &Connection,
    id: RestoreTestId,
    code: FindingCode,
    count: u64,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO ops_restore_test_findings (test_id, code, count) VALUES (?1, ?2, ?3)",
    )?
    .execute(params![id, code, i64::try_from(count).unwrap_or(i64::MAX)])?;
    Ok(())
}

/// How a running test ended.
pub(super) struct Finished<'a> {
    pub(super) state: TestState,
    pub(super) failure: Option<TestFailure>,
    pub(super) staged: Option<&'a StagedRestore>,
    pub(super) elapsed_ms: u64,
}

/// Records the end of a running test; returns its new revision.
pub(super) fn finish(
    conn: &Connection,
    id: RestoreTestId,
    finished: &Finished<'_>,
    now: UtcTimestamp,
) -> rusqlite::Result<i64> {
    let staged = finished.staged;
    let anchors = staged
        .and_then(|staged| staged.anchors.as_ref())
        .map(counts);
    let count =
        |index: usize| anchors.map(|counts| i64::try_from(counts[index]).unwrap_or(i64::MAX));
    conn.prepare_cached(
        "UPDATE ops_restore_tests SET state = ?2, failure_code = ?3, finished_at = ?4, \
         tested_set_id = ?5, set_created_at = ?6, elapsed_ms = ?7, anchors_compared = ?8, \
         anchors_matched = ?9, anchors_lost_interval = ?10, anchors_unverified = ?11, \
         anchors_mismatched = ?12, anchors_missing = ?13, anchor_conflicts = ?14, \
         anchor_gaps = ?15, anchors_unanchored = ?16, revision = revision + 1 \
         WHERE id = ?1 RETURNING revision",
    )?
    .query_row(
        params![
            id,
            finished.state,
            finished.failure,
            now,
            staged.map(|staged| staged.set_id.as_str()),
            staged
                .and_then(|staged| staged.manifest.as_ref())
                .map(|manifest| manifest.created_at),
            i64::try_from(finished.elapsed_ms).unwrap_or(i64::MAX),
            count(0),
            count(1),
            count(2),
            count(3),
            count(4),
            count(5),
            count(6),
            count(7),
            count(8),
        ],
        |row| row.get(0),
    )
}

/// Tests still `requested` or `running`, with their revisions.
pub(crate) fn unfinished(conn: &Connection) -> rusqlite::Result<Vec<(RestoreTestId, i64)>> {
    conn.prepare_cached(
        "SELECT id, revision FROM ops_restore_tests WHERE state IN ('requested', 'running') \
         ORDER BY requested_at, id",
    )?
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
    .collect()
}

/// Moves a requested or running test to `interrupted`.
pub(crate) fn interrupt(
    conn: &Connection,
    id: RestoreTestId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE ops_restore_tests SET state = 'interrupted', finished_at = ?2, \
         revision = revision + 1 WHERE id = ?1",
    )?
    .execute(params![id, now])?;
    Ok(())
}

/// A restore test as the backups screen lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreTestRecord {
    /// The test.
    pub test_id: RestoreTestId,
    /// The target tested.
    pub target_id: TargetId,
    /// Its label.
    pub target_label: String,
    /// The set tested, or the one requested (`None`: the newest) until it runs.
    pub set_id: Option<String>,
    /// When the tested set was taken.
    pub set_created_at: Option<UtcTimestamp>,
    /// Where the test stands.
    pub state: TestState,
    /// Who asked for it (stored actor text).
    pub requested_by: String,
    /// When.
    pub requested_at: UtcTimestamp,
    /// When it started.
    pub started_at: Option<UtcTimestamp>,
    /// When it ended.
    pub finished_at: Option<UtcTimestamp>,
    /// How long the check took: the measured restore time (NFR-30).
    pub elapsed_ms: Option<u64>,
    /// Why it could not check its set.
    pub failure: Option<TestFailure>,
    /// The anchor comparison, when the database opened.
    pub anchors: Option<AnchorSummary>,
    /// What it found.
    pub findings: Findings,
}

impl RestoreTestRecord {
    /// True for a test that passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.state == TestState::Passed
    }
}

const RECORD_COLUMNS: &str = "t.id, t.target_id, g.label, coalesce(t.tested_set_id, t.set_id), \
    t.set_created_at, t.state, t.requested_by, t.requested_at, t.started_at, t.finished_at, \
    t.elapsed_ms, t.failure_code, t.anchors_compared, t.anchors_matched, \
    t.anchors_lost_interval, t.anchors_unverified, t.anchors_mismatched, t.anchors_missing, \
    t.anchor_conflicts, t.anchor_gaps, t.anchors_unanchored";

/// The newest `limit` restore tests, newest first, with their findings.
pub fn list_restore_tests(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<RestoreTestRecord>, BackupError> {
    list_restore_tests_page(conn, limit, None)
}

/// Stable timestamp-and-ID continuation; missing anchors are explicit errors.
pub fn list_restore_tests_page(
    conn: &Connection,
    limit: u32,
    before_test_id: Option<RestoreTestId>,
) -> Result<Vec<RestoreTestRecord>, BackupError> {
    if let Some(id) = before_test_id {
        get_restore_test(conn, id)?;
    }
    let sql = format!(
        "SELECT {RECORD_COLUMNS} FROM ops_restore_tests t JOIN ops_targets g ON g.id = t.target_id \
         WHERE (?1 IS NULL OR (t.requested_at, t.id) < \
         (SELECT requested_at, id FROM ops_restore_tests WHERE id = ?1)) \
         ORDER BY t.requested_at DESC, t.id DESC LIMIT ?2"
    );
    let mut records = conn
        .prepare_cached(&sql)?
        .query_map(params![before_test_id, limit.min(500)], record_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    load_findings(conn, &mut records)?;
    Ok(records)
}

/// An exact retained test, including its findings, independent of history pagination.
pub fn get_restore_test(
    conn: &Connection,
    id: RestoreTestId,
) -> Result<RestoreTestRecord, BackupError> {
    let sql = format!(
        "SELECT {RECORD_COLUMNS} FROM ops_restore_tests t JOIN ops_targets g ON g.id=t.target_id WHERE t.id=?1"
    );
    let record = conn
        .prepare_cached(&sql)?
        .query_row([id], record_from_row)
        .optional()?
        .ok_or(BackupError::NotFound("restore test"))?;
    let mut records = [record];
    load_findings(conn, &mut records)?;
    let [record] = records;
    Ok(record)
}
fn load_findings(conn: &Connection, records: &mut [RestoreTestRecord]) -> Result<(), BackupError> {
    let mut findings = conn.prepare_cached(
        "SELECT code, count FROM ops_restore_test_findings WHERE test_id = ?1 ORDER BY rowid",
    )?;
    for record in records {
        let rows = findings.query_map([record.test_id], |row| {
            Ok((row.get::<_, FindingCode>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (code, count) = row?;
            record.findings.add(code, u64::try_from(count).unwrap_or(0));
        }
    }
    Ok(())
}

fn record_from_row(row: &Row<'_>) -> rusqlite::Result<RestoreTestRecord> {
    let elapsed_ms: Option<i64> = row.get(10)?;
    Ok(RestoreTestRecord {
        test_id: row.get(0)?,
        target_id: row.get(1)?,
        target_label: row.get(2)?,
        set_id: row.get(3)?,
        set_created_at: row.get(4)?,
        state: row.get(5)?,
        requested_by: row.get(6)?,
        requested_at: row.get(7)?,
        started_at: row.get(8)?,
        finished_at: row.get(9)?,
        elapsed_ms: elapsed_ms.map(|value| u64::try_from(value).unwrap_or(0)),
        failure: row.get(11)?,
        anchors: summary_at(row, 12)?,
        findings: Findings::default(),
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::MIGRATIONS;

    fn running_test(conn: &Connection, id: RestoreTestId) {
        conn.execute(
            "INSERT INTO ops_restore_tests (id, target_id, state, requested_by, requested_at, \
             revision) VALUES (?1, 'target', 'requested', 'user', 't0', 1)",
            [id],
        )
        .unwrap();
        conn.execute(
            "UPDATE ops_restore_tests SET state = 'running', started_at = 't1', revision = 2 \
             WHERE id = ?1",
            [id],
        )
        .unwrap();
    }

    fn findings(conn: &Connection) -> Vec<(String, String, i64)> {
        conn.prepare("SELECT test_id, code, count FROM ops_restore_test_findings ORDER BY code")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    // The rebuild keeps every row and the append-only triggers, and only adds the
    // `anchor_gap` code (NFR-30).
    #[test]
    fn migration_4_admits_anchor_gap_and_keeps_rows_and_triggers() {
        let conn = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..3] {
            conn.execute_batch(migration.sql).unwrap();
        }
        // The test row's target is not part of this check.
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        let earlier = RestoreTestId::generate();
        running_test(&conn, earlier);
        insert_finding(&conn, earlier, FindingCode::ChainBroken, 2).unwrap();
        assert!(insert_finding(&conn, earlier, FindingCode::AnchorGap, 1).is_err());
        let before = findings(&conn);
        conn.pragma_update(None, "foreign_keys", true).unwrap();

        conn.execute_batch(MIGRATIONS[3].sql).unwrap();
        assert_eq!(findings(&conn), before);
        insert_finding(&conn, earlier, FindingCode::AnchorGap, 1).unwrap();
        assert_eq!(findings(&conn).len(), 2);
        assert!(
            conn.execute("UPDATE ops_restore_test_findings SET count = 9", [])
                .is_err()
        );
        assert!(
            conn.execute("DELETE FROM ops_restore_test_findings", [])
                .is_err()
        );
        assert!(
            insert_finding(&conn, RestoreTestId::generate(), FindingCode::AnchorGap, 1).is_err(),
            "a finding needs a running test"
        );
        assert!(
            conn.execute(
                "INSERT INTO ops_restore_test_findings VALUES (?1, 'not_a_code', 1)",
                [earlier],
            )
            .is_err()
        );
    }
}
