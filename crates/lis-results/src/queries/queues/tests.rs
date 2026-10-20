use super::*;
use rusqlite::params;

#[test]
fn review_candidates_page_oldest_first_without_gaps() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE res_results (id TEXT PRIMARY KEY, current_revision_id TEXT) STRICT;
         CREATE TABLE res_result_revisions (
            id TEXT PRIMARY KEY, result_id TEXT, specimen_id TEXT, created_at TEXT
         ) STRICT;",
    )?;
    // Two specimens share an instant, so the page key needs the specimen ID too.
    let mut expected = Vec::new();
    for created_at in [
        "2026-10-13T09:00:00.000000000Z",
        "2026-10-13T08:00:00.000000000Z",
        "2026-10-13T08:00:00.000000000Z",
    ] {
        let specimen = SpecimenId::generate();
        let (result, revision) = (crate::ResultId::generate(), ResultRevisionId::generate());
        conn.execute(
            "INSERT INTO res_results VALUES (?1, ?2)",
            params![result, revision],
        )?;
        conn.execute(
            "INSERT INTO res_result_revisions VALUES (?1, ?2, ?3, ?4)",
            params![revision, result, specimen, created_at],
        )?;
        expected.push((created_at.to_owned(), specimen.to_string()));
    }
    expected.sort();

    let mut seen = Vec::new();
    let mut after: Option<ReviewCandidate> = None;
    loop {
        let mut statement = candidate_statement(&conn, ReviewQueueFilter::Blocked)?;
        let page = statement
            .query_map(
                params![after.map(|c| c.since), after.map(|c| c.specimen_id)],
                candidate_row,
            )?
            .take(2)
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert!(page.len() <= 2);
        seen.extend(
            page.iter()
                .map(|c| (c.since.to_string(), c.specimen_id.to_string())),
        );
        if page.len() < 2 {
            break;
        }
        after = page.last().copied();
    }
    let expected_ids: Vec<&String> = expected.iter().map(|(_, id)| id).collect();
    let seen_ids: Vec<&String> = seen.iter().map(|(_, id)| id).collect();
    assert_eq!(seen_ids, expected_ids);
    Ok(())
}
