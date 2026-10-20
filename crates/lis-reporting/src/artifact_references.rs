//! Artifact references owned by report candidates, issued revisions, withdrawals,
//! preservation incidents and exports (NFR-07, REQ-OPS-007/010).

use std::collections::BTreeSet;

use rusqlite::Connection;

/// Columns in this module that hold content-addressed artifact digests.
pub const ARTIFACT_COLUMNS: &[(&str, &str)] = &[
    ("rpt_candidates", "artifact_sha256"),
    ("rpt_revisions", "artifact_sha256"),
    ("rpt_withdrawals", "notice_artifact_sha256"),
    ("rpt_withdrawal_notice_artifacts", "artifact_sha256"),
    ("rpt_preservation_incidents", "artifact_sha256"),
    ("rpt_exports", "artifact_sha256"),
];

/// Every distinct artifact digest referenced by reporting records.
pub fn referenced_artifacts(conn: &Connection) -> rusqlite::Result<BTreeSet<String>> {
    let mut digests = BTreeSet::new();
    for (table, column) in ARTIFACT_COLUMNS {
        let sql = format!(
            "SELECT DISTINCT \"{column}\" FROM \"{table}\" \
             WHERE \"{column}\" IS NOT NULL"
        );
        for digest in conn
            .prepare(&sql)?
            .query_map([], |row| row.get::<_, String>(0))?
        {
            digests.insert(digest?);
        }
    }
    Ok(digests)
}
