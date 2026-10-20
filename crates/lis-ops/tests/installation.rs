//! The installation row: written once, audited, and never changed.

use lis_core::{CorrelationId, Principal};
use lis_store::StoreError;
use lis_store::testing::TestDatabase;
use rusqlite::Connection;

fn system() -> Principal {
    Principal::system("test", [])
}

#[test]
fn the_first_start_creates_one_audited_installation_and_later_starts_reuse_it() {
    let test = TestDatabase::open(&[lis_ops::SCHEMA]).unwrap();
    let ensure = || {
        test.db.write_blocking(
            system(),
            CorrelationId::generate(),
            lis_ops::ensure_installation,
        )
    };
    let first = ensure().unwrap();
    assert_eq!(ensure().unwrap(), first);
    let (stored, created) = test
        .db
        .read_blocking(|conn| -> Result<_, StoreError> {
            let created: i64 = conn.query_row(
                "SELECT count(*) FROM audit_events WHERE action = ?1",
                [lis_ops::INSTALLATION_CREATE],
                |row| row.get(0),
            )?;
            Ok((lis_ops::installation_id(conn)?, created))
        })
        .unwrap();
    assert_eq!((stored, created), (Some(first), 1));
}

#[test]
fn the_installation_row_cannot_be_changed_or_deleted() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(lis_ops::MIGRATIONS[0].sql).unwrap();
    conn.execute(
        "INSERT INTO ops_installation (singleton, installation_id, created_at) \
         VALUES (1, '0199e2a0-0000-7000-8000-000000000001', '2026-10-13T00:00:00.000000000Z')",
        [],
    )
    .unwrap();
    for sql in [
        "UPDATE ops_installation SET created_at = created_at",
        "DELETE FROM ops_installation",
        "INSERT INTO ops_installation (singleton, installation_id, created_at) \
         VALUES (2, '0199e2a0-0000-7000-8000-000000000002', '2026-10-13T00:00:00.000000000Z')",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

fn open(
    test: &TestDatabase,
    kind: lis_ops::InstallationKind,
) -> Result<lis_core::InstallationId, lis_ops::InstallationError> {
    test.db
        .write_blocking(system(), CorrelationId::generate(), move |ctx| {
            lis_ops::open_installation(ctx, kind)
        })
}

// Proposed REQ-OPS-016: the kind is fixed at the first start, and a database never opens
// as the other kind.
#[test]
fn an_installation_opens_only_as_the_kind_it_was_created_as() {
    use lis_ops::InstallationKind::{Demo, Laboratory};
    for (created, other) in [(Demo, Laboratory), (Laboratory, Demo)] {
        let test = TestDatabase::open(&[lis_ops::SCHEMA]).unwrap();
        let id = open(&test, created).unwrap();
        assert_eq!(open(&test, created).unwrap(), id);
        let refused = open(&test, other).unwrap_err();
        assert!(
            matches!(refused, lis_ops::InstallationError::KindMismatch { .. }),
            "{refused}"
        );
        assert_eq!(refused.code(), lis_core::ErrorCode::Conflict);
        let kind = test.db.read_blocking(lis_ops::installation_kind).unwrap();
        assert_eq!(kind, Some(created));
    }
}

#[test]
fn an_installation_created_before_the_kind_existed_is_a_laboratory() {
    let conn = Connection::open_in_memory().unwrap();
    for migration in &lis_ops::MIGRATIONS[..4] {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute(
        "INSERT INTO ops_installation (singleton, installation_id, created_at) \
         VALUES (1, '0199e2a0-0000-7000-8000-000000000001', '2026-10-13T00:00:00.000000000Z')",
        [],
    )
    .unwrap();
    conn.execute_batch(lis_ops::MIGRATIONS[4].sql).unwrap();
    assert_eq!(
        lis_ops::installation_kind(&conn).unwrap(),
        Some(lis_ops::InstallationKind::Laboratory)
    );
    assert!(
        conn.execute("UPDATE ops_installation SET kind = 'demo'", [])
            .is_err(),
        "the kind is append-only"
    );
}
