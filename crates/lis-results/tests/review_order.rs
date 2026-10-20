//! Historical reviews keep their insertion order through migration and VACUUM.

use rusqlite::{Connection, params};

#[test]
fn review_order_migration_preserves_history_and_append_only_sequence() {
    let conn = Connection::open_in_memory().unwrap();
    // This schema-only fixture isolates migration/ordering guards. Clinical
    // references use synthetic placeholders; the engine scenario supplies
    // complete records and exercises the production foreign-key checks.
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    for schema in [
        lis_audit::SCHEMA,
        lis_store::SCHEMA,
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
    ] {
        for migration in schema.migrations {
            conn.execute_batch(migration.sql).unwrap();
        }
    }
    for migration in &lis_results::MIGRATIONS[..6] {
        conn.execute_batch(migration.sql).unwrap();
    }
    // Sparse rowids simulate a database whose rowids VACUUM can renumber.
    for (rowid, id, timestamp) in [(7, "first", "later"), (12, "second", "earlier")] {
        conn.execute(
            "INSERT INTO res_technical_reviews \
             (rowid, id, specimen_id, decision, reviewed_by, reviewed_at) \
             VALUES (?1, ?2, 'synthetic-specimen', 'approved', 'synthetic-reviewer', ?3)",
            params![rowid, id, timestamp],
        )
        .unwrap();
    }
    conn.execute_batch(lis_results::MIGRATIONS[6].sql).unwrap();
    conn.execute_batch("VACUUM").unwrap();
    let ordered: Vec<(String, i64)> = conn
        .prepare("SELECT id, sequence FROM res_technical_reviews ORDER BY sequence")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        ordered,
        [("first".to_owned(), 7), ("second".to_owned(), 12)]
    );
    assert!(
        conn.execute("UPDATE res_technical_reviews SET sequence = 99", [])
            .is_err()
    );
    assert!(
        conn.execute("DELETE FROM res_technical_reviews", [])
            .is_err()
    );
    assert!(
        conn.execute(
            "INSERT INTO res_technical_reviews \
             (id, specimen_id, decision, reviewed_by, reviewed_at, sequence) \
             VALUES ('third', 'synthetic-specimen', 'approved', 'synthetic-reviewer', 'now', 5)",
            [],
        )
        .is_err()
    );
}
