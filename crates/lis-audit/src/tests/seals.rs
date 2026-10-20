//! Seals: the chain of seals, its trigger, and verification against the
//! event chain (NFR-18, ADR 0008).

use rusqlite::{Connection, params};

use super::{
    GENESIS_HASH, append_clinical, append_events, at, findings, open, recompute_hash, seal_at,
    tamper,
};
use crate::{ChainLink, Finding, Seal, Stream, event_recorded_at, heads, last_seal, seals, verify};

const T1: &str = "2026-10-13T08:00:00.000000000Z";
const T2: &str = "2026-10-13T08:15:00.000000000Z";

fn expected_seal(
    stream: Stream,
    seal_no: i64,
    (first_seq, last_seq): (i64, i64),
    prev_hash: &str,
    head_hash: &str,
    time: &str,
) -> Seal {
    Seal {
        stream,
        seal_no,
        first_seq,
        last_seq,
        prev_hash: prev_hash.to_owned(),
        head_hash: head_hash.to_owned(),
        sealed_at: at(time),
        hash_alg: "sha256-v1".to_owned(),
    }
}

/// Inserts a seal row directly, as a buggy caller would.
fn insert_seal(
    conn: &Connection,
    seal_no: i64,
    (first_seq, last_seq): (i64, i64),
    prev_hash: &str,
    head_hash: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO audit_seals VALUES ('clinical', ?1, ?2, ?3, ?4, ?5, ?6, 'sha256-v1')",
        params![seal_no, first_seq, last_seq, prev_hash, head_hash, T2],
    )
}

#[test]
fn seals_chain_across_calls_and_streams() {
    let (_dir, mut conn) = open();
    let first = append_clinical(&mut conn, 3);
    let seal_1 = expected_seal(
        Stream::Clinical,
        1,
        (1, 3),
        GENESIS_HASH,
        &first[2].hash,
        T1,
    );
    assert_eq!(seal_at(&mut conn, T1), std::slice::from_ref(&seal_1));

    let more = append_clinical(&mut conn, 2);
    let security = append_events(&mut conn, Stream::Security, 1);
    let seal_2 = expected_seal(
        Stream::Clinical,
        2,
        (4, 5),
        &first[2].hash,
        &more[1].hash,
        T2,
    );
    let security_1 = expected_seal(
        Stream::Security,
        1,
        (1, 1),
        GENESIS_HASH,
        &security[0].hash,
        T2,
    );
    assert_eq!(seal_at(&mut conn, T2), [seal_2.clone(), security_1.clone()]);

    assert_eq!(
        seals(&conn, Stream::Clinical, 0).unwrap(),
        [seal_1, seal_2.clone()]
    );
    assert_eq!(
        seals(&conn, Stream::Clinical, 1).unwrap(),
        std::slice::from_ref(&seal_2)
    );
    assert_eq!(seals(&conn, Stream::Security, 0).unwrap(), [security_1]);
    assert!(seals(&conn, Stream::Access, 0).unwrap().is_empty());
    assert_eq!(last_seal(&conn, Stream::Clinical).unwrap(), Some(seal_2));
    assert_eq!(last_seal(&conn, Stream::Access).unwrap(), None);
    for stream in Stream::ALL {
        assert!(verify(&conn, stream).unwrap().is_intact(), "{stream}");
    }
}

#[test]
fn seal_does_nothing_when_no_stream_has_new_events() {
    let (_dir, mut conn) = open();
    assert!(
        seal_at(&mut conn, T1).is_empty(),
        "empty streams get no seal"
    );
    append_clinical(&mut conn, 2);
    assert_eq!(seal_at(&mut conn, T1).len(), 1);
    assert!(seal_at(&mut conn, T2).is_empty());
    assert_eq!(seals(&conn, Stream::Clinical, 0).unwrap().len(), 1);
}

#[test]
fn insert_trigger_refuses_seals_that_do_not_continue_the_chain() {
    let (_dir, mut conn) = open();
    let links = append_clinical(&mut conn, 4);
    seal_at(&mut conn, T1);
    let head_3 = links[2].hash.as_str();
    let head_4 = links[3].hash.as_str();
    let prev = links[3].hash.as_str();
    let refused = [
        (
            "gap in seal numbers",
            insert_seal(&conn, 3, (5, 5), prev, head_4),
        ),
        (
            "seal number repeated",
            insert_seal(&conn, 1, (5, 5), prev, head_4),
        ),
        (
            "gap in sequence",
            insert_seal(&conn, 2, (6, 6), prev, head_4),
        ),
        (
            "wrong previous hash",
            insert_seal(&conn, 2, (5, 5), head_3, head_4),
        ),
        (
            "wrong head hash",
            insert_seal(&conn, 2, (5, 5), prev, head_3),
        ),
        (
            "event does not exist",
            insert_seal(&conn, 2, (5, 9), prev, head_4),
        ),
    ];
    for (case, result) in refused {
        let error = result.expect_err(case);
        assert!(
            error
                .to_string()
                .contains("does not continue the chain of seals"),
            "{case}: {error}"
        );
    }
    // The first seal of an empty-sealed stream must start at 1 from genesis.
    let error = conn
        .execute(
            "INSERT INTO audit_seals VALUES ('security', 1, 1, 1, ?1, ?1, ?2, 'sha256-v1')",
            params![GENESIS_HASH, T2],
        )
        .unwrap_err();
    assert!(error.to_string().contains("audit seal"), "{error}");
    assert_eq!(seals(&conn, Stream::Clinical, 0).unwrap().len(), 1);
}

#[test]
fn seals_are_append_only() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 2);
    seal_at(&mut conn, T1);
    for sql in [
        "UPDATE audit_seals SET last_seq = 1",
        "UPDATE audit_seals SET sealed_at = '2026-10-13T09:00:00.000000000Z'",
        "DELETE FROM audit_seals",
    ] {
        let error = conn.execute(sql, []).unwrap_err();
        assert!(error.to_string().contains("append-only"), "{sql}: {error}");
    }
}

/// Acts as the OS administrator on the seals table.
fn tamper_seals(conn: &Connection, sql: &str) {
    conn.execute_batch(
        "DROP TRIGGER audit_seals_no_update; DROP TRIGGER audit_seals_no_delete; \
         DROP TRIGGER audit_seals_continue_chain;",
    )
    .unwrap();
    conn.execute_batch(sql).unwrap();
}

#[test]
fn verify_reports_a_seal_that_disagrees_with_a_recomputed_chain() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 3);
    seal_at(&mut conn, T1);
    // Edit the sealed head and recompute its hash: the chain itself verifies.
    tamper(
        &conn,
        "UPDATE audit_events SET outcome = 'failed' WHERE seq = 3",
    );
    recompute_hash(&conn, 3);
    assert_eq!(findings(&conn), [Finding::SealMismatch { seal_no: 1 }]);
}

#[test]
fn verify_reports_a_seal_whose_events_were_deleted() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 3);
    seal_at(&mut conn, T1);
    tamper(&conn, "DELETE FROM audit_events WHERE seq = 3");
    assert_eq!(findings(&conn), [Finding::SealMismatch { seal_no: 1 }]);
}

#[test]
fn verify_reports_edited_and_removed_seals() {
    let (_dir, mut conn) = open();
    for time in [T1, T2, "2026-10-13T08:30:00.000000000Z"] {
        append_clinical(&mut conn, 1);
        seal_at(&mut conn, time);
    }
    tamper_seals(&conn, "DELETE FROM audit_seals WHERE seal_no = 2");
    assert_eq!(findings(&conn), [Finding::SealMismatch { seal_no: 3 }]);
    conn.execute(
        "UPDATE audit_seals SET head_hash = prev_hash WHERE seal_no = 1",
        [],
    )
    .unwrap();
    assert_eq!(
        findings(&conn),
        [
            Finding::SealMismatch { seal_no: 1 },
            Finding::SealMismatch { seal_no: 3 }
        ]
    );
}

#[test]
fn heads_cover_every_stream_including_empty_ones() {
    let (_dir, mut conn) = open();
    let empty = |stream| ChainLink {
        stream,
        seq: 0,
        hash: GENESIS_HASH.to_owned(),
    };
    assert_eq!(heads(&conn).unwrap(), Stream::ALL.map(empty).to_vec());
    let links = append_clinical(&mut conn, 2);
    assert_eq!(
        heads(&conn).unwrap(),
        [
            links[1].clone(),
            empty(Stream::Security),
            empty(Stream::Access)
        ]
    );
}

#[test]
fn event_recorded_at_reads_one_event() {
    let (_dir, mut conn) = open();
    append_clinical(&mut conn, 1);
    assert_eq!(
        event_recorded_at(&conn, Stream::Clinical, 1).unwrap(),
        Some(at("2026-10-12T10:00:00.000000001Z"))
    );
    assert_eq!(event_recorded_at(&conn, Stream::Clinical, 2).unwrap(), None);
    assert_eq!(event_recorded_at(&conn, Stream::Access, 1).unwrap(), None);
}
