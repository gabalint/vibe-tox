//! Paging through one stream by sequence number and through every stream by append time.
use super::*;
use crate::{EventCursor, EventFilter, StoredEvent};

fn seqs(events: &[StoredEvent]) -> Vec<i64> {
    events.iter().map(|e| e.seq).collect()
}

/// Appends one `specimen.receive` event to `stream`, recorded at `time`.
fn append_at(conn: &mut Connection, stream: Stream, time: &str) {
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at(time),
        recorded_at: at(time),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    append(&tx, &attribution, &event(stream, 1)).unwrap();
    tx.commit().unwrap();
}

/// Up to two events of every stream after `before`, as (stream, seq), and the next cursor.
fn page(
    conn: &Connection,
    before: Option<EventCursor>,
) -> (Vec<(Stream, i64)>, Option<EventCursor>) {
    let filter = EventFilter {
        stream: None,
        before,
        ..EventFilter::new(Stream::Clinical, 2)
    };
    let events = crate::list(conn, &filter).unwrap();
    let next = events.last().map(|e| EventCursor {
        recorded_at: e.recorded_at,
        stream: e.stream,
        seq: e.seq,
    });
    (events.iter().map(|e| (e.stream, e.seq)).collect(), next)
}

#[test]
fn list_pages_newest_first_with_filters() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 5);
    let recorded_at =
        crate::list(&conn, &EventFilter::new(Stream::Clinical, 1)).unwrap()[0].recorded_at;
    let before = |seq| EventCursor {
        recorded_at,
        stream: Stream::Clinical,
        seq,
    };
    let filter = |before, record_id| EventFilter {
        before,
        action: Some("specimen.receive"),
        record_id,
        ..EventFilter::new(Stream::Clinical, 2)
    };
    assert_eq!(
        seqs(&crate::list(&conn, &filter(None, None)).unwrap()),
        [5, 4]
    );
    assert_eq!(
        seqs(&crate::list(&conn, &filter(Some(before(4)), None)).unwrap()),
        [3, 2]
    );
    assert_eq!(
        seqs(&crate::list(&conn, &filter(None, Some("specimen-2"))).unwrap()),
        [2]
    );
    let first = &crate::list(&conn, &filter(Some(before(2)), None)).unwrap()[0];
    assert_eq!(
        (
            first.stream,
            first.principal_kind.as_str(),
            first.outcome.as_str(),
            first.authority.as_deref()
        ),
        (Stream::Clinical, "user", "succeeded", Some("accession"))
    );
    let mut other = filter(None, None);
    other.stream = Some(Stream::Security);
    assert!(crate::list(&conn, &other).unwrap().is_empty());
}

#[test]
fn list_reads_every_stream_by_append_time_with_a_stable_cursor() {
    let (_dir, mut conn) = open();
    append_at(
        &mut conn,
        Stream::Clinical,
        "2026-10-12T10:00:00.000000000Z",
    );
    append_at(
        &mut conn,
        Stream::Security,
        "2026-10-12T10:00:01.000000000Z",
    );
    append_at(
        &mut conn,
        Stream::Clinical,
        "2026-10-12T10:00:02.000000000Z",
    );
    // The same instant: the stream code breaks the tie.
    append_at(&mut conn, Stream::Access, "2026-10-12T10:00:02.000000000Z");
    append_at(
        &mut conn,
        Stream::Security,
        "2026-10-12T10:00:03.000000000Z",
    );
    let (first, next) = page(&conn, None);
    assert_eq!(first, [(Stream::Security, 2), (Stream::Clinical, 2)]);
    // An event appended after the first page does not move the next one.
    append_at(
        &mut conn,
        Stream::Clinical,
        "2026-10-12T10:00:04.000000000Z",
    );
    let (second, next) = page(&conn, next);
    assert_eq!(second, [(Stream::Access, 1), (Stream::Security, 1)]);
    let (third, next) = page(&conn, next);
    assert_eq!(third, [(Stream::Clinical, 1)]);
    assert!(page(&conn, next).0.is_empty());
    assert_eq!(
        crate::actions(&conn, None).unwrap(),
        ["specimen.receive".to_owned()]
    );
    assert_eq!(
        crate::record_types(&conn, Some(Stream::Access)).unwrap(),
        ["specimen".to_owned()]
    );
}

#[test]
fn every_stream_pages_through_the_time_index_without_sorting() {
    let (_dir, conn) = open();
    let plan: Vec<String> = conn
        .prepare(
            "EXPLAIN QUERY PLAN SELECT action FROM audit_events \
             WHERE (recorded_at, stream, seq) < ('~', '~', 9) AND (?1 IS NULL OR action = ?1) \
             ORDER BY recorded_at DESC, stream DESC, seq DESC LIMIT 2",
        )
        .unwrap()
        .query_map([None::<String>], |row| row.get(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter()
            .any(|step| step.contains("audit_events_by_time")),
        "{plan:?}"
    );
    assert!(
        !plan.iter().any(|step| step.contains("TEMP B-TREE")),
        "{plan:?}"
    );
}

#[test]
fn list_can_show_only_self_approvals() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 2);
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T11:00:00.000000000Z"),
        recorded_at: at("2026-10-12T11:00:00.000000001Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::ApproveConfiguration],
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let marked = AuditEvent::new(
        Stream::Clinical,
        "catalog.approve_method_version",
        Outcome::Succeeded,
    )
    .record("method_version", "method-1")
    .self_approval();
    append(&tx, &attribution, &marked).unwrap();
    tx.commit().unwrap();
    let filter = |self_approvals_only| EventFilter {
        self_approvals_only,
        ..EventFilter::new(Stream::Clinical, 10)
    };
    assert_eq!(
        seqs(&crate::list(&conn, &filter(false)).unwrap()),
        [3, 2, 1]
    );
    assert_eq!(seqs(&crate::list(&conn, &filter(true)).unwrap()), [3]);
    let mut every = filter(true);
    every.stream = None;
    assert_eq!(seqs(&crate::list(&conn, &every).unwrap()), [3]);
}

/// Appends one clinical event by `principal` with `outcome`, recorded at `time`.
fn append_by(conn: &mut Connection, principal: &Principal, outcome: Outcome, time: &str) {
    let attribution = Attribution {
        principal,
        occurred_at: at(time),
        recorded_at: at(time),
        correlation_id: CorrelationId::generate(),
        authority: &[],
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let event = AuditEvent::new(Stream::Clinical, "specimen.receive", outcome);
    append(&tx, &attribution, &event).unwrap();
    tx.commit().unwrap();
}

#[test]
fn list_filters_by_recorded_time_principal_and_outcome() {
    let (_dir, mut conn) = open();
    let person = user();
    let other = user();
    let instrument = Principal::instrument(InstrumentId::generate());
    append_by(
        &mut conn,
        &person,
        Outcome::Succeeded,
        "2026-10-12T10:00:00.000000000Z",
    );
    append_by(
        &mut conn,
        &instrument,
        Outcome::Failed,
        "2026-10-13T10:00:00.000000000Z",
    );
    append_by(
        &mut conn,
        &other,
        Outcome::Denied,
        "2026-10-14T10:00:00.000000000Z",
    );
    let person_id = person.actor().id_text();
    let read = |change: &dyn Fn(&mut EventFilter<'_>)| {
        let mut filter = EventFilter::new(Stream::Clinical, 10);
        change(&mut filter);
        seqs(&crate::list(&conn, &filter).unwrap())
    };
    assert_eq!(read(&|f| f.by = Some(crate::ByFilter::People)), [3, 1]);
    assert_eq!(read(&|f| f.by = Some(crate::ByFilter::System)), [2]);
    let one_user = EventFilter {
        by: Some(crate::ByFilter::User(&person_id)),
        ..EventFilter::new(Stream::Clinical, 10)
    };
    assert_eq!(seqs(&crate::list(&conn, &one_user).unwrap()), [1]);
    assert_eq!(read(&|f| f.outcome = Some(Outcome::Failed)), [2]);
    assert_eq!(read(&|f| f.outcome = Some(Outcome::Denied)), [3]);
    let from = at("2026-10-13T00:00:00.000000000Z");
    let before = at("2026-10-14T00:00:00.000000000Z");
    assert_eq!(read(&|f| f.recorded_from = Some(from)), [3, 2]);
    assert_eq!(read(&|f| f.recorded_before = Some(before)), [2, 1]);
    // Filters combine, in one stream and across every stream.
    let both = |f: &mut EventFilter<'_>| {
        f.recorded_from = Some(from);
        f.recorded_before = Some(before);
        f.outcome = Some(Outcome::Failed);
        f.by = Some(crate::ByFilter::System);
    };
    assert_eq!(read(&both), [2]);
    assert_eq!(
        read(&|f| {
            both(f);
            f.stream = None;
        }),
        [2]
    );
    assert!(
        read(&|f| {
            both(f);
            f.by = Some(crate::ByFilter::People);
        })
        .is_empty()
    );
}
