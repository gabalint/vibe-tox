//! Append and verification tests on a real SQLite file. The tamper tests act
//! as an OS administrator who edits the database directly: they drop the
//! append-only triggers first, then change rows with plain SQL.

mod anchors;
mod record_events;
mod seals;

use lis_core::{
    Capability, CorrelationId, InstrumentId, Principal, SessionId, UserId, UtcTimestamp,
    WorkstationId,
};
use rusqlite::{Connection, TransactionBehavior};
use tempfile::TempDir;

use crate::chain::{EventRow, GENESIS_HASH};
use crate::{
    Attribution, AuditDetail, AuditError, AuditEvent, ChainLink, Finding, MIGRATIONS, Outcome,
    Stream, append, verify,
};

fn open() -> (TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = Connection::open(dir.path().join("audit.sqlite3")).unwrap();
    conn.pragma_update(None, "journal_mode", "WAL").unwrap();
    conn.pragma_update(None, "synchronous", "FULL").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    for migration in MIGRATIONS {
        conn.execute_batch(migration.sql).unwrap();
    }
    (dir, conn)
}

fn at(text: &str) -> UtcTimestamp {
    UtcTimestamp::parse(text).unwrap()
}

fn user() -> Principal {
    Principal::user(
        UserId::generate(),
        SessionId::generate(),
        WorkstationId::generate(),
        [Capability::Accession],
    )
}

fn event(stream: Stream, n: i64) -> AuditEvent {
    AuditEvent::new(stream, "specimen.receive", Outcome::Succeeded)
        .record("specimen", format!("specimen-{n}"))
        .new_revision(n)
        .detail("container", AuditDetail::integer(n))
}

/// Appends `count` clinical events in one transaction and returns their links.
fn append_clinical(conn: &mut Connection, count: i64) -> Vec<ChainLink> {
    append_events(conn, Stream::Clinical, count)
}

/// Appends `count` events to `stream` in one transaction and returns their links.
fn append_events(conn: &mut Connection, stream: Stream, count: i64) -> Vec<ChainLink> {
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T10:00:00.000000000Z"),
        recorded_at: at("2026-10-12T10:00:00.000000001Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let links = (1..=count)
        .map(|n| append(&tx, &attribution, &event(stream, n)).unwrap())
        .collect();
    tx.commit().unwrap();
    links
}

/// Acts as the OS administrator: removes the append-only triggers, then runs `sql`.
fn tamper(conn: &Connection, sql: &str) {
    conn.execute_batch("DROP TRIGGER audit_events_no_update; DROP TRIGGER audit_events_no_delete;")
        .unwrap();
    conn.execute_batch(sql).unwrap();
}

fn read_row(conn: &Connection, seq: i64) -> EventRow {
    let sql = format!(
        "SELECT {} FROM audit_events WHERE stream = 'clinical' AND seq = ?1",
        crate::chain::COLUMNS
    );
    conn.query_row(&sql, [seq], EventRow::from_row).unwrap()
}

/// Rewrites the stored hash of `seq` so the event is self-consistent again.
fn recompute_hash(conn: &Connection, seq: i64) {
    let hash = read_row(conn, seq).compute_hash();
    conn.execute(
        "UPDATE audit_events SET hash = ?1 WHERE stream = 'clinical' AND seq = ?2",
        rusqlite::params![hash, seq],
    )
    .unwrap();
}

/// Seals every stream at `time` in its own transaction.
fn seal_at(conn: &mut Connection, time: &str) -> Vec<crate::Seal> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let seals = crate::seal(&tx, at(time)).unwrap();
    tx.commit().unwrap();
    seals
}

fn findings(conn: &Connection) -> Vec<Finding> {
    verify(conn, Stream::Clinical).unwrap().findings
}

#[test]
fn append_links_events_per_stream() {
    let (_dir, mut conn) = open();
    let links = append_clinical(&mut conn, 3);
    assert_eq!(links.iter().map(|l| l.seq).collect::<Vec<_>>(), [1, 2, 3]);
    let first = read_row(&conn, 1);
    assert_eq!(first.prev_hash, GENESIS_HASH);
    assert_eq!(read_row(&conn, 2).prev_hash, links[0].hash);
    assert_eq!(first.authority.as_deref(), Some("accession"));
    assert_eq!(first.principal_kind, "user");
    assert_eq!(first.details, r#"{"container":1}"#);

    // A second stream starts its own sequence at 1.
    let principal = Principal::instrument(InstrumentId::generate());
    let tx = conn.transaction().unwrap();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T10:00:00.000000000Z"),
        recorded_at: at("2026-10-12T10:00:00.000000000Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[],
    };
    let security = AuditEvent::new(Stream::Security, "auth.sign_in", Outcome::Failed);
    let link = append(&tx, &attribution, &security).unwrap();
    tx.commit().unwrap();
    assert_eq!((link.stream, link.seq), (Stream::Security, 1));

    for stream in Stream::ALL {
        let report = verify(&conn, stream).unwrap();
        assert!(report.is_intact(), "{stream}: {:?}", report.findings);
    }
    let clinical = verify(&conn, Stream::Clinical).unwrap();
    assert_eq!(clinical.events_checked, 3);
    assert_eq!(clinical.head, Some(links[2].clone()));
    assert_eq!(verify(&conn, Stream::Access).unwrap().head, None);
}

#[test]
fn append_refuses_invalid_actions_and_details_and_writes_nothing() {
    let (_dir, mut conn) = open();
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T10:00:00.000000000Z"),
        recorded_at: at("2026-10-12T10:00:00.000000000Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[],
    };
    let tx = conn.transaction().unwrap();
    let bad_action = AuditEvent::new(Stream::Clinical, "Receive", Outcome::Succeeded);
    assert!(matches!(
        append(&tx, &attribution, &bad_action),
        Err(AuditError::InvalidAction)
    ));
    let oversized =
        event(Stream::Clinical, 1).detail("codes", AuditDetail::codes(vec!["code"; 1000]));
    assert!(matches!(
        append(&tx, &attribution, &oversized),
        Err(AuditError::InvalidDetails)
    ));
    tx.commit().unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM audit_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn triggers_block_update_and_delete() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 2);
    for sql in [
        "UPDATE audit_events SET reason = 'edited' WHERE seq = 1",
        "DELETE FROM audit_events WHERE seq = 2",
        "DELETE FROM audit_events",
    ] {
        let error = conn.execute(sql, []).unwrap_err();
        assert!(error.to_string().contains("append-only"), "{sql}: {error}");
    }
    assert!(verify(&conn, Stream::Clinical).unwrap().is_intact());
}

#[test]
fn verify_detects_modified_event() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 4);
    tamper(
        &conn,
        "UPDATE audit_events SET record_id = 'specimen-9' WHERE seq = 2",
    );
    assert_eq!(findings(&conn), [Finding::HashMismatch { seq: 2 }]);
}

#[test]
fn verify_detects_edit_with_recomputed_hash_by_the_broken_link() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 4);
    tamper(
        &conn,
        "UPDATE audit_events SET outcome = 'failed' WHERE seq = 2",
    );
    recompute_hash(&conn, 2);
    assert_eq!(findings(&conn), [Finding::BrokenLink { seq: 3 }]);
}

#[test]
fn verify_detects_deleted_event() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 4);
    tamper(&conn, "DELETE FROM audit_events WHERE seq = 2");
    assert_eq!(
        findings(&conn),
        [
            Finding::MissingEvents { first: 2, last: 2 },
            Finding::BrokenLink { seq: 3 }
        ]
    );
}

#[test]
fn verify_detects_deleted_first_event() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 3);
    tamper(&conn, "DELETE FROM audit_events WHERE seq = 1");
    assert_eq!(
        findings(&conn),
        [
            Finding::MissingEvents { first: 1, last: 1 },
            Finding::BrokenLink { seq: 2 }
        ]
    );
}

#[test]
fn verify_detects_inserted_forged_event() {
    let (_dir, mut conn) = open();
    let links = append_clinical(&mut conn, 4);
    // Make room at seq 3 by renumbering the later events, then insert a forged
    // event that links correctly to seq 2 and carries a correctly computed hash.
    tamper(
        &conn,
        "UPDATE audit_events SET seq = 5 WHERE seq = 4;
         UPDATE audit_events SET seq = 4 WHERE seq = 3;",
    );
    let mut forged = read_row(&conn, 2);
    forged.seq = 3;
    forged.prev_hash.clone_from(&links[1].hash);
    forged.record_id = Some("specimen-forged".into());
    let hash = forged.compute_hash();
    conn.execute(
        &format!(
            "INSERT INTO audit_events ({}, hash) SELECT hash_alg, stream, 3, ?1, occurred_at, \
             recorded_at, principal_kind, principal_id, session_id, workstation_id, action, \
             outcome, record_type, ?2, old_revision, new_revision, reason, authority, \
             correlation_id, details, ?3 FROM audit_events WHERE seq = 2",
            crate::chain::COLUMNS
        ),
        rusqlite::params![forged.prev_hash, forged.record_id, hash],
    )
    .unwrap();
    assert_eq!(
        findings(&conn),
        [
            Finding::BrokenLink { seq: 4 },
            Finding::HashMismatch { seq: 4 },
            Finding::HashMismatch { seq: 5 },
        ]
    );
}

#[test]
fn verify_detects_swapped_events() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 4);
    tamper(
        &conn,
        "UPDATE audit_events SET seq = 100 WHERE seq = 2;
         UPDATE audit_events SET seq = 2 WHERE seq = 3;
         UPDATE audit_events SET seq = 3 WHERE seq = 100;",
    );
    assert_eq!(
        findings(&conn),
        [
            Finding::BrokenLink { seq: 2 },
            Finding::HashMismatch { seq: 2 },
            Finding::BrokenLink { seq: 3 },
            Finding::HashMismatch { seq: 3 },
            Finding::BrokenLink { seq: 4 },
        ]
    );
}

#[test]
fn verify_reports_unknown_algorithm() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 2);
    tamper(
        &conn,
        "UPDATE audit_events SET hash_alg = 'md5-v0' WHERE seq = 2",
    );
    assert_eq!(findings(&conn), [Finding::UnknownAlgorithm { seq: 2 }]);
}

/// Documents the on-host limit (ADR 0008): only off-host anchors of the chain
/// head (M4) detect a recomputed chain or a truncated tail.
#[test]
fn verify_cannot_detect_full_recomputation_or_tail_truncation_without_anchors() {
    let (_dir, mut conn) = open();
    let links = append_clinical(&mut conn, 3);
    tamper(&conn, "DELETE FROM audit_events WHERE seq = 3");
    let report = verify(&conn, Stream::Clinical).unwrap();
    assert!(report.is_intact());
    assert_ne!(
        report.head,
        Some(links[2].clone()),
        "an anchor of the head would differ"
    );

    conn.execute(
        "UPDATE audit_events SET reason = 'rewritten' WHERE seq = 1",
        [],
    )
    .unwrap();
    recompute_hash(&conn, 1);
    let new_hash = read_row(&conn, 1).compute_hash();
    conn.execute(
        "UPDATE audit_events SET prev_hash = ?1 WHERE seq = 2",
        [&new_hash],
    )
    .unwrap();
    recompute_hash(&conn, 2);
    assert!(verify(&conn, Stream::Clinical).unwrap().is_intact());
}

#[test]
fn list_pages_newest_first_with_filters() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 5);
    let filter = |before_seq, record_id| crate::EventFilter {
        stream: Stream::Clinical,
        before_seq,
        limit: 2,
        action: Some("specimen.receive"),
        record_type: None,
        record_id,
        self_approvals_only: false,
    };
    let seqs = |events: Vec<crate::StoredEvent>| events.iter().map(|e| e.seq).collect::<Vec<_>>();
    assert_eq!(
        seqs(crate::list(&conn, &filter(None, None)).unwrap()),
        [5, 4]
    );
    assert_eq!(
        seqs(crate::list(&conn, &filter(Some(4), None)).unwrap()),
        [3, 2]
    );
    assert_eq!(
        seqs(crate::list(&conn, &filter(None, Some("specimen-2"))).unwrap()),
        [2]
    );
    let first = &crate::list(&conn, &filter(Some(2), None)).unwrap()[0];
    assert_eq!(
        (
            first.principal_kind.as_str(),
            first.outcome.as_str(),
            first.authority.as_deref()
        ),
        ("user", "succeeded", Some("accession"))
    );
    let mut other = filter(None, None);
    other.stream = Stream::Security;
    assert!(crate::list(&conn, &other).unwrap().is_empty());
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
    let filter = |self_approvals_only| crate::EventFilter {
        stream: Stream::Clinical,
        before_seq: None,
        limit: 10,
        action: None,
        record_type: None,
        record_id: None,
        self_approvals_only,
    };
    let seqs = |events: Vec<crate::StoredEvent>| events.iter().map(|e| e.seq).collect::<Vec<_>>();
    assert_eq!(seqs(crate::list(&conn, &filter(false)).unwrap()), [3, 2, 1]);
    assert_eq!(seqs(crate::list(&conn, &filter(true)).unwrap()), [3]);
}
