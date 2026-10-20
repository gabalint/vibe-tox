//! The copy kept before migrations change an existing database (NFR-14, M4
//! design brief §6.4 step 3).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use lis_core::{FixedClock, Migration, ModuleSchema, UtcTimestamp};
use lis_store::{DATABASE_FILE, Database, PRE_MIGRATION_DIR, StoreError};
use rusqlite::Connection;
use tempfile::TempDir;

const V1: Migration = Migration {
    version: 1,
    name: "demo_counts",
    sql: "CREATE TABLE demo_counts (n INTEGER NOT NULL CHECK (n > 0)) STRICT;",
};
const V2: Migration = Migration {
    version: 2,
    name: "demo_two",
    sql: "CREATE TABLE demo_two (id INTEGER PRIMARY KEY) STRICT;",
};
const V3: Migration = Migration {
    version: 3,
    name: "demo_three",
    sql: "CREATE TABLE demo_three (id INTEGER PRIMARY KEY) STRICT;",
};
const V4: Migration = Migration {
    version: 4,
    name: "demo_four",
    sql: "CREATE TABLE demo_four (id INTEGER PRIMARY KEY) STRICT;",
};
const V5: Migration = Migration {
    version: 5,
    name: "demo_five",
    sql: "CREATE TABLE demo_five (id INTEGER PRIMARY KEY) STRICT;",
};
const VERSIONS: [Migration; 5] = [V1, V2, V3, V4, V5];

/// The demo module with its first `count` migrations.
fn demo(count: usize) -> ModuleSchema {
    ModuleSchema {
        module: "demo",
        table_prefix: "demo_",
        migrations: &VERSIONS[..count],
    }
}

fn at(text: &str) -> UtcTimestamp {
    UtcTimestamp::parse(text).unwrap()
}

/// Opens `dir` with the demo module's first `count` migrations at `time`, then closes it.
fn open_at(dir: &Path, count: usize, time: &str) -> Result<(), StoreError> {
    let clock = Arc::new(FixedClock::new(at(time)));
    Database::open(dir, &[demo(count)], clock).map(drop)
}

/// File names in the pre-migration folder, sorted; empty when it does not exist.
fn copies(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir.join(PRE_MIGRATION_DIR)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn demo_versions(path: &Path) -> Vec<u32> {
    let conn = Connection::open(path).unwrap();
    let mut statement = conn
        .prepare("SELECT version FROM store_migrations WHERE module = 'demo' ORDER BY version")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn new_database_and_reopen_without_pending_migrations_take_no_copy() {
    let dir = TempDir::new().unwrap();
    open_at(dir.path(), 1, "2026-10-13T08:00:00.000000000Z").unwrap();
    open_at(dir.path(), 1, "2026-10-13T09:00:00.000000000Z").unwrap();
    assert_eq!(copies(dir.path()), Vec::<String>::new());
}

#[test]
fn pending_migrations_on_an_existing_database_keep_an_intact_copy_first() {
    let dir = TempDir::new().unwrap();
    open_at(dir.path(), 1, "2026-10-13T08:00:00.000000000Z").unwrap();
    open_at(dir.path(), 2, "2026-10-13T09:30:15.000000007Z").unwrap();
    assert_eq!(copies(dir.path()), ["20261013T093015.000000007Z.sqlite3"]);
    let copy = dir
        .path()
        .join(PRE_MIGRATION_DIR)
        .join("20261013T093015.000000007Z.sqlite3");
    assert_eq!(demo_versions(&copy), [1], "the copy is the state before");
    assert_eq!(demo_versions(&dir.path().join(DATABASE_FILE)), [1, 2]);
    let check: String = Connection::open(&copy)
        .unwrap()
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(check, "ok");
}

#[test]
fn only_the_newest_three_copies_are_kept() {
    let dir = TempDir::new().unwrap();
    open_at(dir.path(), 1, "2026-10-13T08:00:00.000000000Z").unwrap();
    fs::create_dir(dir.path().join(PRE_MIGRATION_DIR)).unwrap();
    let folder = dir.path().join(PRE_MIGRATION_DIR);
    fs::write(
        folder.join("20261013T080000.000000000Z.sqlite3.tmp"),
        b"crash",
    )
    .unwrap();
    fs::write(folder.join("README.txt"), b"left alone").unwrap();
    for (count, hour) in [(2, "09"), (3, "10"), (4, "11"), (5, "12")] {
        open_at(
            dir.path(),
            count,
            &format!("2026-10-13T{hour}:00:00.000000000Z"),
        )
        .unwrap();
    }
    assert_eq!(
        copies(dir.path()),
        [
            "20261013T100000.000000000Z.sqlite3",
            "20261013T110000.000000000Z.sqlite3",
            "20261013T120000.000000000Z.sqlite3",
            "README.txt",
        ]
    );
    assert_eq!(
        demo_versions(&folder.join("20261013T120000.000000000Z.sqlite3")),
        [1, 2, 3, 4]
    );
}

#[test]
fn a_copy_that_is_not_intact_refuses_the_migration() {
    let dir = TempDir::new().unwrap();
    open_at(dir.path(), 1, "2026-10-13T08:00:00.000000000Z").unwrap();
    // A row that breaks its CHECK constraint: the file opens, but
    // `PRAGMA integrity_check` (and so the copy's check) reports it.
    let conn = Connection::open(dir.path().join(DATABASE_FILE)).unwrap();
    conn.execute_batch(
        "PRAGMA ignore_check_constraints = ON; INSERT INTO demo_counts (n) VALUES (-1);",
    )
    .unwrap();
    drop(conn);

    let refused = open_at(dir.path(), 2, "2026-10-13T09:00:00.000000000Z");
    assert!(
        matches!(refused, Err(StoreError::PreMigrationSnapshotNotIntact)),
        "{refused:?}"
    );
    assert_eq!(demo_versions(&dir.path().join(DATABASE_FILE)), [1]);
    assert_eq!(copies(dir.path()), Vec::<String>::new());
}

#[test]
fn a_copy_that_cannot_be_written_refuses_the_migration() {
    let dir = TempDir::new().unwrap();
    open_at(dir.path(), 1, "2026-10-13T08:00:00.000000000Z").unwrap();
    fs::write(dir.path().join(PRE_MIGRATION_DIR), b"a file, not a folder").unwrap();
    let refused = open_at(dir.path(), 2, "2026-10-13T09:00:00.000000000Z");
    assert!(
        matches!(refused, Err(StoreError::PreMigrationSnapshotFailed(_))),
        "{refused:?}"
    );
    assert_eq!(demo_versions(&dir.path().join(DATABASE_FILE)), [1]);
}
