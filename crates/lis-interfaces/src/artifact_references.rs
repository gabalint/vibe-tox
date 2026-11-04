//! Artifact references owned by analyzer-interface receipts and outgoing
//! partner messages (REQ-OPS-007/010).

use std::collections::BTreeSet;

use rusqlite::Connection;

/// Columns in this module that hold content-addressed artifact digests.
pub const ARTIFACT_COLUMNS: &[(&str, &str)] = &[
    ("ifc_receipts", "artifact_digest"),
    ("ifc_partner_messages", "artifact_sha256"),
];

/// Every distinct artifact digest referenced by interface records.
pub fn referenced_artifacts(conn: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    conn.prepare_cached(
        "SELECT artifact_digest FROM ifc_receipts WHERE artifact_digest IS NOT NULL \
         UNION SELECT artifact_sha256 FROM ifc_partner_messages ORDER BY 1",
    )?
    .query_map([], |row| row.get(0))?
    .collect()
}
