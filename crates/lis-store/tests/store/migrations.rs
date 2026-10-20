//! Migration runner, checksums and the single-instance lock.

use std::path::Path;
use std::sync::Arc;

use lis_core::{CorrelationId, FixedClock, Migration, ModuleSchema};
use lis_store::testing::{principal, test_start};
use lis_store::{
    AppliedMigration, DATABASE_FILE, Database, MigrationProblem, StoreError, applied_migrations,
    check_applied,
};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::DEMO_V1;

const DEMO_V1_EDITED: Migration = Migration {
    version: 1,
    name: "demo_items",
    sql: "CREATE TABLE demo_items (id INTEGER PRIMARY KEY, label TEXT NOT NULL, note TEXT) STRICT;",
};
const DEMO_V1_RENAMED: Migration = Migration {
    version: 1,
    name: "demo_renamed",
    sql: DEMO_V1.sql,
};
const DEMO_V1_COMMITS: Migration = Migration {
    version: 1,
    name: "demo_commits",
    sql: "CREATE TABLE demo_items (id INTEGER PRIMARY KEY) STRICT; COMMIT;",
};
const DEMO_V1_CORRECTED: Migration = Migration {
    version: 1,
    name: "demo_commits",
    sql: "CREATE TABLE demo_items (id INTEGER PRIMARY KEY) STRICT;",
};
const DEMO_V2: Migration = Migration {
    version: 2,
    name: "demo_more",
    sql: "CREATE TABLE demo_more (id INTEGER PRIMARY KEY) STRICT;",
};
const DEMO_V2_BROKEN: Migration = Migration {
    version: 2,
    name: "demo_broken",
    sql: "CREATE TABLE demo_broken (id INTEGER PRIMARY KEY) STRICT; NOT VALID SQL;",
};
const DEMO_V3: Migration = Migration {
    version: 3,
    name: "demo_third",
    sql: "CREATE TABLE demo_third (id INTEGER PRIMARY KEY) STRICT;",
};

const FAMILY_V1: Migration = Migration {
    version: 1,
    name: "demo_family",
    sql: "CREATE TABLE demo_parents (id INTEGER PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('a'))) STRICT;
          CREATE TABLE demo_children (id INTEGER PRIMARY KEY,
              parent_id INTEGER NOT NULL REFERENCES demo_parents (id)) STRICT;
          INSERT INTO demo_parents VALUES (1, 'a');
          INSERT INTO demo_children VALUES (1, 1);",
};
/// Widens a CHECK on a table that another table references, by rebuilding it.
const FAMILY_V2_REBUILD: Migration = Migration {
    version: 2,
    name: "demo_widen",
    sql: "CREATE TABLE demo_parents_next (id INTEGER PRIMARY KEY,
              kind TEXT NOT NULL CHECK (kind IN ('a', 'b'))) STRICT;
          INSERT INTO demo_parents_next SELECT id, kind FROM demo_parents;
          DROP TABLE demo_parents;
          ALTER TABLE demo_parents_next RENAME TO demo_parents;",
};
const FAMILY_V2_ORPHANS: Migration = Migration {
    version: 2,
    name: "demo_orphans",
    sql: "DELETE FROM demo_parents;",
};

fn demo(migrations: &'static [Migration]) -> ModuleSchema {
    ModuleSchema {
        module: "demo",
        table_prefix: "demo_",
        migrations,
    }
}

fn open(dir: &Path, modules: &[ModuleSchema]) -> Result<Database, StoreError> {
    Database::open(dir, modules, Arc::new(FixedClock::new(test_start())))
}

fn refused(result: Result<Database, StoreError>) -> (String, u32, MigrationProblem) {
    match result {
        Err(StoreError::Migration {
            module,
            version,
            problem,
        }) => (module, version, problem),
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("opened despite a migration problem"),
    }
}

/// Opens `first`, closes it, and returns the result of opening `second` in the same directory.
fn reopen_with(first: &[ModuleSchema], second: &[ModuleSchema]) -> Result<Database, StoreError> {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), first).unwrap());
    open(dir.path(), second)
}

fn ledger(dir: &Path) -> Vec<(String, u32, String)> {
    let conn = Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT module, version, applied_at FROM store_migrations ORDER BY module, version",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn reopen_is_idempotent() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2])]).unwrap());
    let first = ledger(dir.path());
    let modules: Vec<_> = first.iter().map(|(m, v, _)| (m.as_str(), *v)).collect();
    assert_eq!(
        modules,
        [
            ("audit", 1),
            ("audit", 2),
            ("audit", 3),
            ("demo", 1),
            ("demo", 2),
            ("store", 1)
        ]
    );
    drop(open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2])]).unwrap());
    assert_eq!(ledger(dir.path()), first);
}

#[test]
fn edited_migration_is_refused() {
    let result = reopen_with(&[demo(&[DEMO_V1])], &[demo(&[DEMO_V1_EDITED])]);
    assert_eq!(
        refused(result),
        ("demo".into(), 1, MigrationProblem::ChecksumChanged)
    );
    let result = reopen_with(&[demo(&[DEMO_V1])], &[demo(&[DEMO_V1_RENAMED])]);
    assert_eq!(
        refused(result),
        ("demo".into(), 1, MigrationProblem::ChecksumChanged)
    );
}

#[test]
fn migration_that_ends_its_own_transaction_is_refused() {
    let dir = TempDir::new().unwrap();
    let result = open(dir.path(), &[demo(&[DEMO_V1_COMMITS])]);
    assert!(matches!(result, Err(StoreError::TransactionEnded)));
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let schema: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'demo_items'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let ledger: i64 = conn
        .query_row(
            "SELECT count(*) FROM store_migrations WHERE module = 'demo'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((schema, ledger), (0, 0));
    drop(conn);
    drop(open(dir.path(), &[demo(&[DEMO_V1_CORRECTED])]).unwrap());
}

#[test]
fn applied_migration_missing_from_the_build_is_refused() {
    let result = reopen_with(&[demo(&[DEMO_V1, DEMO_V2])], &[demo(&[DEMO_V1])]);
    assert_eq!(
        refused(result),
        ("demo".into(), 2, MigrationProblem::Missing)
    );
    let result = reopen_with(&[demo(&[DEMO_V1])], &[]);
    assert_eq!(
        refused(result),
        ("demo".into(), 1, MigrationProblem::Missing)
    );
}

#[test]
fn the_ledger_reads_back_and_is_checked_like_an_open() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2])]).unwrap());
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let applied = applied_migrations(&conn).unwrap();
    let names: Vec<_> = applied
        .iter()
        .filter(|row| row.module == "demo")
        .map(|row| (row.module.as_str(), row.version, row.name.as_str()))
        .collect();
    assert_eq!(names, [("demo", 1, "demo_items"), ("demo", 2, "demo_more")]);
    assert!(applied.iter().all(|row| row.checksum.len() == 64));

    assert!(check_applied(&[demo(&[DEMO_V1, DEMO_V2])], &applied).is_ok());
    assert!(check_applied(&[demo(&[DEMO_V1, DEMO_V2, DEMO_V3])], &applied).is_ok());
    let older_build = check_applied(&[demo(&[DEMO_V1])], &applied);
    assert!(matches!(
        older_build,
        Err(StoreError::Migration {
            version: 2,
            problem: MigrationProblem::Missing,
            ..
        })
    ));
    let mut edited: Vec<AppliedMigration> = applied.clone();
    edited[2].checksum = "0".repeat(64);
    let changed = check_applied(&[demo(&[DEMO_V1, DEMO_V2])], &edited);
    assert!(matches!(
        changed,
        Err(StoreError::Migration {
            problem: MigrationProblem::ChecksumChanged,
            ..
        })
    ));
}

#[test]
fn gap_in_the_applied_history_is_refused() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2])]).unwrap());
    Connection::open(dir.path().join(DATABASE_FILE))
        .unwrap()
        .execute(
            "DELETE FROM store_migrations WHERE module = 'demo' AND version = 1",
            [],
        )
        .unwrap();
    let result = open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2])]);
    assert_eq!(
        refused(result),
        ("demo".into(), 1, MigrationProblem::Missing)
    );
}

#[test]
fn misnumbered_or_repeated_modules_are_refused() {
    let dir = TempDir::new().unwrap();
    let cases = [
        (vec![demo(&[DEMO_V2])], 2),
        (vec![demo(&[DEMO_V1, DEMO_V3])], 3),
        (vec![demo(&[DEMO_V1]), demo(&[DEMO_V1])], 0),
    ];
    for (modules, version) in cases {
        let (_, found, problem) = refused(open(dir.path(), &modules));
        assert_eq!((found, problem), (version, MigrationProblem::OutOfOrder));
    }
    let (module, _, problem) = refused(open(dir.path(), &[lis_audit::SCHEMA]));
    assert_eq!(
        (module.as_str(), problem),
        ("audit", MigrationProblem::OutOfOrder)
    );
}

#[test]
fn each_migration_commits_in_its_own_transaction() {
    let dir = TempDir::new().unwrap();
    let failed = open(dir.path(), &[demo(&[DEMO_V1, DEMO_V2_BROKEN])]);
    assert!(matches!(failed, Err(StoreError::Sql(_))));
    // Version 1 committed with its record; the broken version 2 left nothing.
    let db = open(dir.path(), &[demo(&[DEMO_V1])]).unwrap();
    drop(db);
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE name LIKE 'demo_%' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(tables, ["demo_items"]);
}

#[test]
fn second_instance_is_refused_while_open() {
    let dir = TempDir::new().unwrap();
    let first = open(dir.path(), &[]).unwrap();
    assert!(matches!(
        open(dir.path(), &[]),
        Err(StoreError::AlreadyOpen)
    ));
    drop(first);
    assert!(open(dir.path(), &[]).is_ok());
}

#[test]
fn a_referenced_table_can_be_rebuilt_and_keys_stay_enforced() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[FAMILY_V1])]).unwrap());
    let db = open(dir.path(), &[demo(&[FAMILY_V1, FAMILY_V2_REBUILD])]).unwrap();
    let orphan = db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
        ctx.tx()
            .execute("INSERT INTO demo_parents VALUES (2, 'b')", [])
            .map_err(StoreError::from)?;
        ctx.tx()
            .execute("INSERT INTO demo_children VALUES (2, 99)", [])
            .map_err(StoreError::from)
    });
    assert!(matches!(orphan, Err(StoreError::Sql(_))), "{orphan:?}");
    let children: i64 = db
        .read_blocking(|conn| {
            conn.query_row(
                "SELECT count(*) FROM demo_children c JOIN demo_parents p ON p.id = c.parent_id",
                [],
                |row| row.get(0),
            )
            .map_err(StoreError::from)
        })
        .unwrap();
    assert_eq!(children, 1);
}

#[test]
fn a_migration_that_orphans_rows_is_rolled_back() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[FAMILY_V1])]).unwrap());
    let (module, version, problem) =
        refused(open(dir.path(), &[demo(&[FAMILY_V1, FAMILY_V2_ORPHANS])]));
    assert_eq!(
        (module.as_str(), version, problem),
        ("demo", 2, MigrationProblem::ForeignKeyViolation)
    );
    let applied: Vec<u32> = ledger(dir.path())
        .into_iter()
        .filter(|row| row.0 == "demo")
        .map(|row| row.1)
        .collect();
    assert_eq!(applied, [1]);
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    let parents: i64 = conn
        .query_row("SELECT count(*) FROM demo_parents", [], |row| row.get(0))
        .unwrap();
    assert_eq!(parents, 1);
}

#[test]
fn an_older_inconsistency_does_not_block_an_unrelated_migration() {
    let dir = TempDir::new().unwrap();
    drop(open(dir.path(), &[demo(&[FAMILY_V1])]).unwrap());
    // A connection without enforcement leaves an orphan, as an older or
    // hand-repaired database might hold.
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute("INSERT INTO demo_children VALUES (2, 99)", [])
        .unwrap();
    drop(conn);
    drop(open(dir.path(), &[demo(&[FAMILY_V1, FAMILY_V2_REBUILD])]).unwrap());
    let applied: Vec<u32> = ledger(dir.path())
        .into_iter()
        .filter(|row| row.0 == "demo")
        .map(|row| row.1)
        .collect();
    assert_eq!(applied, [1, 2]);
}
