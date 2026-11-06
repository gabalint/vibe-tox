//! Keeping the matched records that audit events name, by stream.
use super::*;
use crate::{RecordSearch, audit_records};

fn append_record(conn: &mut Connection, stream: Stream, record_type: &'static str, id: &str) {
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T10:00:00.000000000Z"),
        recorded_at: at("2026-10-12T10:00:00.000000000Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let event = AuditEvent::new(stream, "test.touch", Outcome::Succeeded).record(record_type, id);
    append(&tx, &attribution, &event).unwrap();
    tx.commit().unwrap();
}

fn found(
    conn: &Connection,
    stream: Option<Stream>,
    record_type: Option<&str>,
    text: &str,
    candidates: &[(&'static str, String)],
) -> Vec<(String, String, Vec<Stream>)> {
    let search = RecordSearch {
        record_type,
        text,
        limit: 20,
    };
    audit_records(conn, stream, &search, candidates, 50)
        .unwrap()
        .into_iter()
        .map(|record| (record.record_type, record.record_id, record.streams))
        .collect()
}

#[test]
fn audit_records_keep_named_candidates_exact_ids_and_readable_ids() {
    let (_dir, mut conn) = open();
    append_record(&mut conn, Stream::Clinical, "order", "order-1");
    append_record(&mut conn, Stream::Access, "order", "order-1");
    append_record(&mut conn, Stream::Security, "instrument", "instrument-1");
    append_record(
        &mut conn,
        Stream::Security,
        "app_version",
        "0.1.2-nightly.1",
    );
    let candidates = [
        ("order", "order-1".to_owned()),
        ("order", "order-without-events".to_owned()),
    ];
    assert_eq!(
        found(&conn, None, None, "A26", &candidates),
        [(
            "order".to_owned(),
            "order-1".to_owned(),
            vec![Stream::Clinical, Stream::Access]
        )]
    );
    // A stream keeps only the records it has events about.
    assert!(found(&conn, Some(Stream::Security), None, "A26", &candidates).is_empty());
    // The stored record ID itself, of any type.
    assert_eq!(
        found(&conn, None, None, "instrument-1", &[])[0].1,
        "instrument-1"
    );
    // An app version's ID is the version: matched by its start, ignoring `%` as a wildcard.
    assert_eq!(
        found(&conn, None, None, "0.1.2", &[])[0].1,
        "0.1.2-nightly.1"
    );
    assert!(found(&conn, None, None, "%", &[]).is_empty());
    // A record type keeps only its own records.
    assert!(found(&conn, None, Some("instrument"), "0.1.2", &[]).is_empty());
}
