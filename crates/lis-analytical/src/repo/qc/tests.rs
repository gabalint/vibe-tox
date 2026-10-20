//! An index-only migration preserves QC supersession, including rolled-back clocks.
use super::*;

fn assessment(conn: &Connection, key: ScopeKey, old: Option<QcAssessmentId>) -> QcAssessmentId {
    let id = QcAssessmentId::generate();
    let (run, analyte, batch) = key.columns();
    let at = if old.is_some() {
        "2026-10-12T09:00:00.000000000Z"
    } else {
        "2026-10-12T10:00:00.000000000Z"
    };
    conn.execute(
        "INSERT INTO ana_qc_assessments (id,scope,run_id,analyte_id,prep_batch_id,\
         outcome,supersedes_id,reason,assessed_by,assessed_at) \
         VALUES (?1,?2,?3,?4,?5,'accepted',?6,'Synthetic reassessment','synthetic',?7)",
        params![id, key.scope(), run, analyte, batch, old, at],
    )
    .unwrap();
    id
}

fn assert_scope_plan(conn: &Connection, key: ScopeKey) {
    let (run, analyte, batch) = key.columns();
    let plan = conn
        .prepare(
            "EXPLAIN QUERY PLAN SELECT a.id FROM ana_qc_assessments a \
             WHERE a.scope = ?1 AND a.run_id IS ?2 AND a.analyte_id IS ?3 \
             AND a.prep_batch_id IS ?4 AND NOT EXISTS \
             (SELECT 1 FROM ana_qc_assessments s WHERE s.supersedes_id = a.id)",
        )
        .unwrap()
        .query_map(params![key.scope(), run, analyte, batch], |row| {
            row.get::<_, String>(3)
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        plan.iter().any(|step| {
            step.contains("ana_qc_assessments_scope_lookup")
                && step.contains("scope=? AND run_id=? AND analyte_id=? AND prep_batch_id=?")
        }),
        "{plan:?}"
    );
}

#[test]
fn qc_scope_index_preserves_current_and_historical_assessments() {
    let conn = Connection::open_in_memory().unwrap();
    // Schema-level fixture: foreign owners are not needed to test this index.
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch(include_str!("../../../migrations/0001_analytical.sql"))
        .unwrap();
    let run = RunId::generate();
    let scopes = [
        ScopeKey::RunAnalyte(run, AnalyteId::generate()),
        ScopeKey::Run(run),
        ScopeKey::PrepBatch(PrepBatchId::generate()),
    ];
    let mut before = Vec::new();
    for key in scopes {
        let first = assessment(&conn, key, None);
        let second = assessment(&conn, key, Some(first));
        let current = current_assessment(&conn, key).unwrap().unwrap();
        assert_eq!(current.assessment_id, second);
        before.push((current, assessments(&conn, key).unwrap()));
    }
    conn.execute_batch(include_str!("../../../migrations/0008_qc_scope_lookup.sql"))
        .unwrap();
    for (key, (current, history)) in scopes.into_iter().zip(before) {
        assert_eq!(current_assessment(&conn, key).unwrap(), Some(current));
        assert_eq!(assessments(&conn, key).unwrap(), history);
        assert_scope_plan(&conn, key);
    }
    assert!(
        conn.execute("UPDATE ana_qc_assessments SET reason='changed'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM ana_qc_assessments", []).is_err());
}
