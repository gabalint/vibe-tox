//! Query-plan regressions for history-bounded result reads (NFR-05).

use lis_store::testing::TestDatabase;
use rusqlite::Connection;

const MODULES: &[lis_core::ModuleSchema] = &[
    lis_auth::SCHEMA,
    lis_identity::SCHEMA,
    lis_catalog::SCHEMA,
    lis_interfaces::SCHEMA,
    lis_orders::SCHEMA,
    lis_specimens::SCHEMA,
    lis_analytical::SCHEMA,
    lis_results::SCHEMA,
];

#[test]
fn schema_passes_lint() {
    assert_eq!(lis_store::lint_schema(MODULES).unwrap(), Vec::new());
}

#[test]
fn queue_and_specimen_reads_use_current_state_indexes() -> Result<(), Box<dyn std::error::Error>> {
    let database = TestDatabase::open(MODULES)?;
    database.db.read_blocking(|conn| {
        assert_uses(
            conn,
            "SELECT specimen_id, min(since) FROM res_unreviewed_results \
             GROUP BY specimen_id ORDER BY min(since), specimen_id LIMIT 50",
            "res_unreviewed_results_queue",
        )?;
        assert_uses(
            conn,
            "SELECT r.id FROM res_result_revisions rr \
             JOIN res_results r ON r.current_revision_id = rr.id \
             WHERE rr.specimen_id = 'synthetic-specimen'",
            "res_result_revisions_specimen_result",
        )?;
        assert_uses(
            conn,
            "SELECT item.review_id FROM res_review_items item \
             WHERE item.result_revision_id = 'synthetic-revision'",
            "res_review_items_revision",
        )?;
        Ok::<_, lis_results::ResultError>(())
    })?;
    Ok(())
}

fn assert_uses(conn: &Connection, sql: &str, index: &str) -> Result<(), lis_results::ResultError> {
    let plan: Vec<String> = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
        .query_map([], |row| row.get(3))?
        .collect::<Result<_, _>>()?;
    assert!(
        plan.iter().any(|step| step.contains(index)),
        "expected {index} in query plan: {plan:?}"
    );
    Ok(())
}
