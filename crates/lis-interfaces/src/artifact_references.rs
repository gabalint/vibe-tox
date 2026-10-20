//! Artifact references owned by analyzer-interface receipts (REQ-OPS-007/010).

use std::collections::BTreeSet;

use rusqlite::Connection;

/// Columns in this module that hold content-addressed artifact digests.
pub const ARTIFACT_COLUMNS: &[(&str, &str)] = &[("ifc_receipts", "artifact_digest")];

/// Every distinct artifact digest referenced by interface records.
pub fn referenced_artifacts(conn: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    conn.prepare_cached(
        "SELECT DISTINCT artifact_digest FROM ifc_receipts \
         WHERE artifact_digest IS NOT NULL ORDER BY artifact_digest",
    )?
    .query_map([], |row| row.get(0))?
    .collect()
}
