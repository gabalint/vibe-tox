//! Anchor lines and their comparison with the local chain (NFR-18, ADR 0008,
//! M4 design brief §4). Expected statuses come from the brief's table.

use lis_core::InstallationId;
use rusqlite::Connection;

use super::{
    Capability, CorrelationId, append_clinical, at, open, read_row, recompute_hash, seal_at,
    tamper, user,
};
use crate::{
    Anchor, AnchorStatus, Attribution, AuditDetail, AuditError, AuditEvent, Outcome, Seal, Stream,
    append, compare_anchors, parse_line, seals, to_line,
};

const T1: &str = "2026-10-13T08:00:00.000000000Z";
const T2: &str = "2026-10-13T08:15:00.000000000Z";
const T3: &str = "2026-10-13T08:30:00.000000000Z";
const INSTALLATION: &str = "01900000-0000-7000-8000-0000000000aa";

fn installation() -> InstallationId {
    INSTALLATION.parse().unwrap()
}

fn anchors(seals: &[Seal]) -> Vec<Anchor> {
    seals
        .iter()
        .map(|seal| Anchor {
            installation: installation(),
            seal: seal.clone(),
        })
        .collect()
}

/// Three clinical seals of two events each; returns them.
fn three_seals(conn: &mut Connection) -> Vec<Seal> {
    for time in [T1, T2, T3] {
        append_clinical(conn, 2);
        seal_at(conn, time);
    }
    seals(conn, Stream::Clinical, 0).unwrap()
}

fn sample_seal() -> Seal {
    Seal {
        stream: Stream::Clinical,
        seal_no: 12,
        first_seq: 1201,
        last_seq: 1350,
        prev_hash: "ab".repeat(32),
        head_hash: "cd".repeat(32),
        sealed_at: at(T1),
        hash_alg: "sha256-v1".to_owned(),
    }
}

/// Written out by hand from the format in the M4 design brief §4.
const SAMPLE_LINE: &str = concat!(
    r#"{"format":"vibe-tox-anchor-1","installation":"01900000-0000-7000-8000-0000000000aa","#,
    r#""stream":"clinical","seal":12,"firstSeq":1201,"lastSeq":1350,"#,
    r#""prevHash":"abababababababababababababababababababababababababababababababab","#,
    r#""headHash":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd","#,
    r#""sealedAt":"2026-10-13T08:00:00.000000000Z","hashAlg":"sha256-v1"}"#
);

#[test]
fn anchor_line_has_the_documented_form_and_round_trips() {
    let line = to_line(&sample_seal(), installation()).unwrap();
    assert_eq!(line, SAMPLE_LINE);
    assert_eq!(
        parse_line(&line).unwrap(),
        Anchor {
            installation: installation(),
            seal: sample_seal()
        }
    );
}

#[test]
fn parse_line_refuses_anything_but_a_well_formed_line() {
    let hash = "ab".repeat(32);
    let replaced = |from: &str, to: &str| SAMPLE_LINE.replacen(from, to, 1);
    let bad = [
        replaced(
            r#""hashAlg":"sha256-v1"}"#,
            r#""hashAlg":"sha256-v1","note":"x"}"#,
        ),
        replaced(r#","hashAlg":"sha256-v1""#, ""),
        replaced(r#""seal":12,"#, r#""seal":12,"seal":13,"#),
        replaced("vibe-tox-anchor-1", "vibe-tox-anchor-2"),
        replaced(r#""hashAlg":"sha256-v1""#, r#""hashAlg":"sha256-v2""#),
        replaced(&hash, &hash.to_uppercase()),
        replaced(&hash, &hash[..62]),
        replaced(&hash, &format!("{}zz", &hash[..62])),
        replaced("2026-10-13T08:00:00.000000000Z", "2026-10-13T08:00:00Z"),
        replaced(
            "2026-10-13T08:00:00.000000000Z",
            "2026-10-13T10:00:00.000000000+02:00",
        ),
        replaced(r#""stream":"clinical""#, r#""stream":"billing""#),
        replaced(r#""seal":12"#, r#""seal":0"#),
        replaced(r#""seal":12"#, r#""seal":12.0"#),
        replaced(r#""seal":12"#, r#""seal":"12""#),
        replaced(r#""firstSeq":1201"#, r#""firstSeq":1351"#),
        replaced(INSTALLATION, "01900000-0000-4000-8000-0000000000aa"),
        replaced(INSTALLATION, "01900000-0000-7000-8000-0000000000AA"),
        format!("{SAMPLE_LINE} {{}}"),
        format!("[{SAMPLE_LINE}]"),
        String::new(),
        "not json".to_owned(),
    ];
    for line in bad {
        assert!(
            matches!(parse_line(&line), Err(AuditError::InvalidAnchor)),
            "accepted: {line}"
        );
    }
    assert!(parse_line(&format!("{SAMPLE_LINE}\n")).is_ok());
}

#[test]
fn to_line_refuses_a_seal_it_could_not_parse_back() {
    let edited = Seal {
        head_hash: "not a digest".to_owned(),
        ..sample_seal()
    };
    assert!(matches!(
        to_line(&edited, installation()),
        Err(AuditError::InvalidAnchor)
    ));
}

#[test]
fn anchor_lines_carry_no_event_content() {
    const CANARY: &str = "canary-7f3e9";
    let (_dir, mut conn) = open();
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at(T1),
        recorded_at: at(T1),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn.transaction().unwrap();
    let event = AuditEvent::new(Stream::Clinical, "specimen.receive", Outcome::Succeeded)
        .record("specimen", CANARY)
        .reason(format!("typed note {CANARY}"))
        .detail("token", AuditDetail::machine_token(CANARY).unwrap());
    append(&tx, &attribution, &event).unwrap();
    tx.commit().unwrap();
    let seal = seal_at(&mut conn, T2).remove(0);
    let line = to_line(&seal, installation()).unwrap();
    for forbidden in [
        CANARY,
        "specimen",
        "accession",
        &principal.actor().id_text(),
    ] {
        assert!(!line.contains(forbidden), "{forbidden} in {line}");
    }
    let object: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&line).unwrap();
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    let mut expected = [
        "format",
        "installation",
        "stream",
        "seal",
        "firstSeq",
        "lastSeq",
        "prevHash",
        "headHash",
        "sealedAt",
        "hashAlg",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);
}

#[test]
fn intact_chain_matches_its_anchors_and_reports_newer_seals_as_unanchored() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    append_clinical(&mut conn, 1);
    seal_at(&mut conn, "2026-10-13T08:45:00.000000000Z");
    assert_eq!(
        compare_anchors(&conn, &anchors(&seals)).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::Matched { anchor: 1 },
            AnchorStatus::Matched { anchor: 2 },
            AnchorStatus::Unanchored {
                stream: Stream::Clinical,
                first_seal_no: 4,
                last_seal_no: 4
            },
        ]
    );
    assert_eq!(
        compare_anchors(&conn, &[]).unwrap(),
        [AnchorStatus::Unanchored {
            stream: Stream::Clinical,
            first_seal_no: 1,
            last_seal_no: 4
        }]
    );
}

/// Rewrites every event from `seq` to the end so the chain verifies again.
fn recompute_chain_from(conn: &Connection, seq: i64, last: i64) {
    for current in seq..=last {
        if current > seq {
            let prev = read_row(conn, current - 1).compute_hash();
            conn.execute(
                "UPDATE audit_events SET prev_hash = ?1 WHERE stream = 'clinical' AND seq = ?2",
                rusqlite::params![prev, current],
            )
            .unwrap();
        }
        recompute_hash(conn, current);
    }
}

#[test]
fn edited_row_with_recomputed_chain_is_an_anchor_mismatch() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    tamper(
        &conn,
        "UPDATE audit_events SET outcome = 'denied' WHERE seq = 3",
    );
    recompute_chain_from(&conn, 3, 6);
    // Only the local seals still disagree; an OS administrator could rewrite them too.
    assert_eq!(
        crate::verify(&conn, Stream::Clinical).unwrap().findings,
        [
            crate::Finding::SealMismatch { seal_no: 2 },
            crate::Finding::SealMismatch { seal_no: 3 }
        ]
    );
    assert_eq!(
        compare_anchors(&conn, &anchors(&seals)).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::AnchorMismatch { anchor: 1 },
            AnchorStatus::AnchorMismatch { anchor: 2 },
        ]
    );
}

#[test]
fn edited_row_without_recomputation_breaks_every_later_anchor() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    tamper(
        &conn,
        "UPDATE audit_events SET outcome = 'denied' WHERE seq = 3",
    );
    assert_eq!(
        compare_anchors(&conn, &anchors(&seals)).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::AnchorMismatch { anchor: 1 },
            AnchorStatus::AnchorMismatch { anchor: 2 },
        ]
    );
}

#[test]
fn deleted_tail_is_local_events_missing() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    tamper(&conn, "DELETE FROM audit_events WHERE seq > 4");
    assert_eq!(
        compare_anchors(&conn, &anchors(&seals)).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::Matched { anchor: 1 },
            AnchorStatus::LocalEventsMissing { anchor: 2 },
        ]
    );
}

#[test]
fn missing_seal_number_below_the_newest_anchor_is_a_gap() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    let without_second = anchors(&[seals[0].clone(), seals[2].clone()]);
    assert_eq!(
        compare_anchors(&conn, &without_second).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::Matched { anchor: 1 },
            AnchorStatus::AnchorGap {
                stream: Stream::Clinical,
                first_seal_no: 2,
                last_seal_no: 2
            },
        ]
    );
    let only_third = anchors(&seals[2..]);
    assert_eq!(
        compare_anchors(&conn, &only_third).unwrap()[1],
        AnchorStatus::AnchorGap {
            stream: Stream::Clinical,
            first_seal_no: 1,
            last_seal_no: 2
        }
    );
}

#[test]
fn two_different_anchors_for_one_seal_conflict_and_identical_copies_do_not() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    let mut copies = anchors(&seals[..1]);
    copies.extend(anchors(&seals[..1]));
    assert_eq!(
        compare_anchors(&conn, &copies).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::Matched { anchor: 1 },
            AnchorStatus::Unanchored {
                stream: Stream::Clinical,
                first_seal_no: 2,
                last_seal_no: 3
            },
        ]
    );
    let mut other = seals[0].clone();
    other.last_seq = 1;
    other.head_hash = read_row(&conn, 1).compute_hash();
    let conflicting = anchors(&[seals[0].clone(), other, seals[1].clone(), seals[2].clone()]);
    assert_eq!(
        compare_anchors(&conn, &conflicting).unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::Matched { anchor: 1 },
            AnchorStatus::Matched { anchor: 2 },
            AnchorStatus::Matched { anchor: 3 },
            AnchorStatus::AnchorConflict {
                stream: Stream::Clinical,
                seal_no: 1
            },
        ]
    );
}

#[test]
fn anchor_with_a_wrong_previous_hash_is_a_mismatch() {
    let (_dir, mut conn) = open();
    let seals = three_seals(&mut conn);
    let mut forged = seals[1].clone();
    forged.prev_hash = "ef".repeat(32);
    assert_eq!(
        compare_anchors(
            &conn,
            &anchors(&[seals[0].clone(), forged, seals[2].clone()])
        )
        .unwrap(),
        [
            AnchorStatus::Matched { anchor: 0 },
            AnchorStatus::AnchorMismatch { anchor: 1 },
            AnchorStatus::Matched { anchor: 2 },
        ]
    );
}
