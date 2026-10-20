//! DM-01: ambiguous legacy evidence stays unresolved, timestamps stay untouched.
use rusqlite::Connection;

#[test]
fn migration_links_only_unique_audit_evidence_and_preserves_legacy_rows() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE spc_specimens(id TEXT PRIMARY KEY, revision INTEGER);
        CREATE TABLE spc_containers(id TEXT PRIMARY KEY, revision INTEGER);
        CREATE TABLE spc_receipt_decisions(id TEXT PRIMARY KEY, specimen_id TEXT, decision TEXT, decided_by TEXT, decided_at TEXT, reason TEXT);
        CREATE TABLE spc_location_events(id TEXT PRIMARY KEY, container_id TEXT, location_id TEXT, moved_by TEXT, moved_at TEXT, reason TEXT);
        CREATE TABLE audit_events(stream TEXT, action TEXT, outcome TEXT, record_id TEXT, principal_id TEXT, occurred_at TEXT, details TEXT, reason TEXT, new_revision INTEGER);
        INSERT INTO spc_specimens VALUES ('s', 3);
        INSERT INTO spc_containers VALUES ('c', 4);
        INSERT INTO spc_receipt_decisions VALUES ('held', 's', 'held', 'u', '2026-10-14T10:00:00Z', 'investigation');
        INSERT INTO spc_receipt_decisions VALUES ('accepted', 's', 'accepted', 'u', '2026-10-13T10:00:00Z', NULL);
        INSERT INTO audit_events VALUES ('clinical', 'specimen.record_receipt_decision', 'succeeded', 's', 'u', '2026-10-14T10:00:00Z', '{\"decision\":\"held\"}', 'investigation', 2);
        INSERT INTO audit_events VALUES ('clinical', 'specimen.record_receipt_decision', 'succeeded', 's', 'u', '2026-10-13T10:00:00Z', '{\"decision\":\"accepted\"}', NULL, 3);
        INSERT INTO spc_location_events VALUES ('move1','c','L','u','equal',NULL), ('move2','c','L','u','equal',NULL);
        INSERT INTO audit_events VALUES ('clinical','specimen.move_container','succeeded','c','u','equal','{\"location\":\"L\"}',NULL,2), ('clinical','specimen.move_container','succeeded','c','u','equal','{\"location\":\"L\"}',NULL,3);").unwrap();
    conn.execute_batch(lis_specimens::MIGRATIONS[3].sql)
        .unwrap();
    let latest: String = conn
        .query_row(
            "SELECT decision_id FROM spc_receipt_order ORDER BY sequence DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(latest, "accepted");
    let unknown: i64 = conn
        .query_row("SELECT count(*) FROM spc_location_order", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        unknown, 0,
        "equal matching evidence must not invent ordering"
    );
    let timestamp: String = conn
        .query_row(
            "SELECT decided_at FROM spc_receipt_decisions WHERE id='accepted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(timestamp, "2026-10-13T10:00:00Z");
    conn.execute("UPDATE spc_containers SET revision=5 WHERE id='c'", [])
        .unwrap();
    conn.execute("INSERT INTO spc_location_events VALUES ('move3','c','verified','u','older', 'current location checked')", []).unwrap();
    let seq: i64 = conn
        .query_row(
            "SELECT sequence FROM spc_location_order WHERE event_id='move3'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(seq, 5);
}
