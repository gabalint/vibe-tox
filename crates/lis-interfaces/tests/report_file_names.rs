//! Migration 0017: a report connection's `{accession}` meant the report number and
//! reads `{report}` now (ticket #4703). Synthetic data only.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_interfaces::MIGRATIONS;
use rusqlite::{Connection, params};

fn connection(conn: &Connection, id: &str, message_type: &str, pattern: &str) {
    conn.execute(
        "INSERT INTO ifc_partner_connections (id, name, partner, message_type, protocol, \
         folder, folder_set_by, rules, schedule, file_name_pattern, active, revision, \
         destination, acceptance_key) \
         VALUES (?1, ?1, 'Synthetic partner', ?2, 'folder', '/tmp/out', 'user-1', '[]', \
         'immediately', ?3, 1, 1, '/tmp/out', 'key')",
        params![id, message_type, pattern],
    )
    .unwrap();
}

fn pattern(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT file_name_pattern FROM ifc_partner_connections WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn report_patterns_name_the_report_number_after_the_migration() {
    let conn = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..16] {
        conn.execute_batch(migration.sql).unwrap();
    }
    connection(
        &conn,
        "pdf",
        "report_pdf",
        "{accession}_r{revision}_{date}.pdf",
    );
    connection(&conn, "hl7", "hl7_oru", "lab-{accession}_r{revision}.hl7");
    connection(&conn, "billing", "billing_json", "billing-{handoff}.json");
    assert_eq!(MIGRATIONS[16].version, 17);
    conn.execute_batch(MIGRATIONS[16].sql).unwrap();
    assert_eq!(pattern(&conn, "pdf"), "{report}_r{revision}_{date}.pdf");
    assert_eq!(pattern(&conn, "hl7"), "lab-{report}_r{revision}.hl7");
    assert_eq!(pattern(&conn, "billing"), "billing-{handoff}.json");
}
