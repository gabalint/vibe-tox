//! Real migrated SQLite navigation; synthetic retained history, not job execution.
#![cfg(test)]
use lis_core::UtcTimestamp;
use lis_ops::{backup, restore};
use lis_store::testing::TestDatabase;
use rusqlite::{Connection, params};
use std::collections::HashSet;

#[test]
fn backup_and_restore_history_reaches_equal_times_rollback_and_exact_targets() {
    let test = TestDatabase::open(&[lis_ops::SCHEMA]).unwrap();
    let conn = Connection::open(test.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    let (run_ids, test_ids, earlier) = seed_history(&conn);
    let runs = all_runs(&conn);
    assert_eq!(runs.len(), 205);
    assert_eq!(
        runs.iter()
            .map(|row| row.run_id)
            .collect::<HashSet<_>>()
            .len(),
        205
    );
    assert_eq!(runs.last().unwrap().run_id, run_ids[204]);
    assert_eq!(
        backup::get_run(&conn, run_ids[204]).unwrap().requested_at,
        earlier
    );
    let records = all_restore_tests(&conn);
    assert_eq!(records.len(), 205);
    assert_eq!(
        records
            .iter()
            .map(|row| row.test_id)
            .collect::<HashSet<_>>()
            .len(),
        205
    );
    assert_eq!(records.last().unwrap().test_id, test_ids[204]);
    assert_eq!(
        restore::get_restore_test(&conn, test_ids[204])
            .unwrap()
            .requested_at,
        earlier
    );
    assert!(matches!(
        backup::list_runs_page(&conn, 100, None, Some(backup::RunId::generate())),
        Err(backup::BackupError::NotFound(_))
    ));
    assert!(matches!(
        restore::list_restore_tests_page(&conn, 100, Some(restore::RestoreTestId::generate())),
        Err(backup::BackupError::NotFound(_))
    ));
    assert!(matches!(
        restore::get_restore_test(&conn, restore::RestoreTestId::generate()),
        Err(backup::BackupError::NotFound(_))
    ));
}

fn seed_history(
    conn: &Connection,
) -> (
    Vec<backup::RunId>,
    Vec<restore::RestoreTestId>,
    UtcTimestamp,
) {
    let now = UtcTimestamp::parse("2026-10-14T08:00:00.000000000Z").unwrap();
    let earlier = UtcTimestamp::parse("2026-10-13T08:00:00.000000000Z").unwrap();
    conn.execute("INSERT INTO ops_targets (id,label,kind,purpose,path,created_by,created_at,revision) VALUES ('00000000-0000-7000-8000-000000000001','Synthetic','local_disk','backup_and_anchors','/synthetic','user',?1,1)", [now]).unwrap();
    let mut run_ids = Vec::new();
    let mut test_ids = Vec::new();
    for index in 0..205 {
        let at = if index == 204 { earlier } else { now };
        let run_id = backup::RunId::generate();
        let test_id = restore::RestoreTestId::generate();
        conn.execute("INSERT INTO ops_backup_runs (id,kind,state,requested_by,requested_at,revision) VALUES (?1,'manual','requested','user',?2,1)", params![run_id,at]).unwrap();
        conn.execute("INSERT INTO ops_restore_tests (id,target_id,state,requested_by,requested_at,revision) VALUES (?1,'00000000-0000-7000-8000-000000000001','requested','user',?2,1)", params![test_id,at]).unwrap();
        run_ids.push(run_id);
        test_ids.push(test_id);
    }
    (run_ids, test_ids, earlier)
}
fn all_runs(conn: &Connection) -> Vec<backup::RunRecord> {
    let mut runs = Vec::new();
    let mut cursor = None;
    loop {
        let mut page = backup::list_runs_page(conn, 100, None, cursor).unwrap();
        let more = page.len() > 100;
        page.truncate(100);
        cursor = page.last().map(|row| row.run_id);
        runs.extend(page);
        if !more {
            break;
        }
    }
    runs
}
fn all_restore_tests(conn: &Connection) -> Vec<restore::RestoreTestRecord> {
    let mut records = Vec::new();
    let mut cursor = None;
    loop {
        let page = restore::list_restore_tests_page(conn, 100, cursor).unwrap();
        let more = page.len() == 100;
        cursor = page.last().map(|row| row.test_id);
        records.extend(page);
        if !more {
            break;
        }
    }
    records
}
