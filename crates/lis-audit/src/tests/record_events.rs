//! Exact causal lookups retain sequence order through an index-only migration.
use super::*;

fn record(conn: &mut Connection, event: AuditEvent, time: &str) -> ChainLink {
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at(time),
        recorded_at: at(time),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn.transaction().unwrap();
    let link = append(&tx, &attribution, &event).unwrap();
    tx.commit().unwrap();
    link
}

#[test]
fn indexed_record_lookup_preserves_history_and_exact_causal_selection() {
    let mut conn = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..2] {
        conn.execute_batch(migration.sql).unwrap();
    }
    let same = "2026-10-15T12:00:00.000000000Z";
    let rollback = "2026-10-15T11:00:00.000000000Z";
    let event = |action, outcome, id| {
        AuditEvent::new(Stream::Clinical, action, outcome)
            .record("technical_review", id)
            .new_revision(2)
    };
    let first = record(
        &mut conn,
        event("result.submit_review", Outcome::Succeeded, "target"),
        same,
    );
    record(
        &mut conn,
        event("result.submit_review", Outcome::Denied, "target"),
        same,
    );
    record(
        &mut conn,
        event("result.other", Outcome::Succeeded, "target"),
        same,
    );
    let last = record(
        &mut conn,
        event("result.submit_review", Outcome::Succeeded, "target"),
        rollback,
    );
    record(
        &mut conn,
        event("result.submit_review", Outcome::Succeeded, "different"),
        rollback,
    );
    let before = crate::heads(&conn).unwrap();
    conn.execute_batch(MIGRATIONS[2].sql).unwrap();
    assert_eq!(crate::heads(&conn).unwrap(), before);
    assert!(verify(&conn, Stream::Clinical).unwrap().findings.is_empty());
    let events = crate::succeeded_record_events(
        &conn,
        Stream::Clinical,
        "result.submit_review",
        "technical_review",
        "target",
    )
    .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.seq, e.new_revision))
            .collect::<Vec<_>>(),
        [(first.seq, Some(2)), (last.seq, Some(2))]
    );
    assert!(
        conn.execute("UPDATE audit_events SET reason = 'changed'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM audit_events", []).is_err());
    assert_lookup_index(&conn);
}

fn assert_lookup_index(conn: &Connection) {
    let plan: Vec<String> = conn
        .prepare(
            "EXPLAIN QUERY PLAN SELECT seq,new_revision FROM audit_events \
         WHERE stream='clinical' AND action='result.submit_review' AND outcome='succeeded' \
           AND record_type='technical_review' AND record_id='target' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| row.get(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter()
            .any(|step| step.contains("COVERING INDEX audit_record_events")),
        "{plan:?}"
    );
}
