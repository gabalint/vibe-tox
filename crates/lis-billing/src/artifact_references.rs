//! Artifact references owned by billing handoffs (REQ-OPS-007/010).

use std::collections::BTreeSet;

use rusqlite::Connection;

/// Columns in this module that hold content-addressed artifact digests.
pub const ARTIFACT_COLUMNS: &[(&str, &str)] = &[("bil_handoffs", "artifact_sha256")];

/// Every distinct artifact digest referenced by billing records.
pub fn referenced_artifacts(conn: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    conn.prepare_cached(
        "SELECT DISTINCT artifact_sha256 FROM bil_handoffs ORDER BY artifact_sha256",
    )?
    .query_map([], |row| row.get(0))?
    .collect()
}
