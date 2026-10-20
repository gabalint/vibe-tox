//! Failure streaks and snapshot integrity read from synthetic run and copy rows (§3.1
//! step 4, §3.5): the rows follow the tables' state machines, so the triggers accept them.

use std::time::Duration;

use lis_core::{CorrelationId, Principal, UtcTimestamp};
use lis_store::testing::TestDatabase;

use super::health_facts;
use crate::backup::{BackupError, RunFailure, RunId};
use crate::health::BackupFacts;
use crate::targets::TargetId;

/// How one synthetic run ends.
struct Outcome {
    state: &'static str,
    failure: Option<RunFailure>,
    /// The snapshot passed its integrity check and was recorded.
    snapshot: bool,
    /// Per target: `true` for a verified copy, `false` for a failed one.
    copies: Vec<(TargetId, bool)>,
}

fn succeeded(copies: Vec<(TargetId, bool)>) -> Outcome {
    Outcome {
        state: "succeeded",
        failure: None,
        snapshot: true,
        copies,
    }
}

fn failed(failure: RunFailure, snapshot: bool, copies: Vec<(TargetId, bool)>) -> Outcome {
    Outcome {
        state: "failed",
        failure: Some(failure),
        snapshot,
        copies,
    }
}

struct Fixture {
    test: TestDatabase,
}

impl Fixture {
    fn new() -> Self {
        Self {
            test: TestDatabase::open(&[crate::SCHEMA]).unwrap(),
        }
    }

    fn write(&self, sql: &'static str, values: impl rusqlite::Params + Send + 'static) {
        self.test
            .db
            .write_blocking(
                Principal::system("test", []),
                CorrelationId::generate(),
                move |ctx| -> Result<(), BackupError> {
                    ctx.tx().execute(sql, values)?;
                    Ok(())
                },
            )
            .unwrap();
    }

    fn at(minute: u64) -> UtcTimestamp {
        lis_store::testing::test_start()
            .checked_add(Duration::from_secs(minute * 60))
            .unwrap()
    }

    fn target(&self, path: &'static str) -> TargetId {
        let id = TargetId::generate();
        self.write(
            "INSERT INTO ops_targets (id, label, kind, purpose, path, created_by, created_at, \
             revision) VALUES (?1, 'Synthetic target', 'local_disk', 'backup_and_anchors', ?2, \
             'test', ?3, 1)",
            (id, path, Self::at(0)),
        );
        id
    }

    /// A scheduled run started at `minute` that ends as `outcome`.
    fn run(&self, minute: u64, outcome: Outcome) {
        let run_id = RunId::generate();
        let at = Self::at(minute);
        self.write(
            "INSERT INTO ops_backup_runs (id, kind, state, requested_at, started_at, revision) \
             VALUES (?1, 'scheduled', 'running', ?2, ?2, 1)",
            (run_id, at),
        );
        for (target_id, verified) in outcome.copies {
            self.write(
                "INSERT INTO ops_backup_copies (run_id, target_id, set_id, state, failure_code, \
                 written_bytes, verified_at) VALUES (?1, ?2, \
                 CASE WHEN ?3 THEN 'synthetic-set' END, \
                 CASE WHEN ?3 THEN 'succeeded' ELSE 'failed' END, \
                 CASE WHEN ?3 THEN NULL ELSE 'write_failed' END, 0, CASE WHEN ?3 THEN ?4 END)",
                (run_id, target_id, verified, at),
            );
        }
        let digest = outcome.snapshot.then(|| "a".repeat(64));
        self.write(
            "UPDATE ops_backup_runs SET state = ?2, failure_code = ?3, finished_at = ?4, \
             snapshot_sha256 = ?5, revision = 2 WHERE id = ?1",
            (run_id, outcome.state, outcome.failure, at, digest),
        );
    }

    fn facts(&self) -> BackupFacts {
        let now = Self::at(1_000);
        self.test
            .db
            .read_blocking(move |conn| health_facts(conn, now, None))
            .unwrap()
    }

    /// `(failed runs in a row, failed copies in a row, snapshot intact)`.
    fn streaks(&self) -> (u32, u32, Option<bool>) {
        let facts = self.facts();
        (
            facts.failed_runs_in_a_row,
            facts.failed_copies_in_a_row,
            facts.snapshot_intact,
        )
    }
}

#[test]
fn failure_streaks_and_snapshot_integrity_follow_the_newest_success() {
    let f = Fixture::new();
    assert_eq!(f.streaks(), (0, 0, None));
    let (a, b) = (f.target("/synthetic/a"), f.target("/synthetic/b"));

    f.run(1, succeeded(vec![(a, true), (b, true)]));
    assert_eq!(f.streaks(), (0, 0, Some(true)));
    f.run(2, succeeded(vec![(a, true), (b, false)]));
    assert_eq!(f.streaks(), (0, 1, Some(true)), "one target failed");
    let all_failed = vec![(a, false), (b, false)];
    f.run(3, failed(RunFailure::AllCopiesFailed, true, all_failed));
    assert_eq!(f.streaks(), (1, 2, Some(true)));

    f.run(4, failed(RunFailure::SnapshotNotIntact, false, Vec::new()));
    assert_eq!(f.streaks(), (2, 2, Some(false)), "damage raised");
    f.write(
        "INSERT INTO ops_backup_runs (id, kind, state, requested_at, started_at, revision) \
         VALUES (?1, 'scheduled', 'running', ?2, ?2, 1)",
        (RunId::generate(), Fixture::at(5)),
    );
    f.write(
        "UPDATE ops_backup_runs SET state = 'interrupted', finished_at = ?1, revision = 2 \
         WHERE state = 'running'",
        (Fixture::at(5),),
    );
    f.run(6, failed(RunFailure::InsufficientSpace, false, Vec::new()));
    assert_eq!(
        f.streaks(),
        (3, 2, Some(false)),
        "an interrupted run neither counts nor clears; an earlier failure keeps the damage"
    );

    f.run(7, succeeded(vec![(a, true), (b, false)]));
    assert_eq!(f.streaks(), (0, 3, Some(true)), "an intact snapshot clears");
    f.run(7, succeeded(vec![(b, true)]));
    assert_eq!(f.streaks(), (0, 0, Some(true)), "a verified copy clears");
}
