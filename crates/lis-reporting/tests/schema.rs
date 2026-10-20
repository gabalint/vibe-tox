//! Tamper tests of the `rpt_` schema on its own (brief §2.1): the
//! candidate state machine, revisions bound to their release event, the
//! second-person verification (unless the separation-of-duties policy allows
//! the creator, ADR 0015) and append-only records. Foreign keys are off so the
//! other modules' rows are not needed; the store enables them in use. The
//! `auth_` schema supplies the policy view.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use rusqlite::{Connection, params};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AT: &str = "2026-10-12T14:00:00.000000000Z";

fn schema() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    for migration in [
        lis_audit::MIGRATIONS,
        lis_auth::MIGRATIONS,
        lis_reporting::MIGRATIONS,
    ]
    .concat()
    {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn.execute(
        "INSERT INTO rpt_reports VALUES ('r1', 's1', 'A1-UR', 'u1', ?1)",
        [AT],
    )
    .unwrap();
    conn
}

fn candidate(conn: &Connection, id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO rpt_candidates (id, report_id, proposed_revision_no, relationship, \
         completeness, signatory_user_id, prepared_at, content_json, content_sha256, data_json, \
         data_sha256, template_id, template_version, state, revision) \
         VALUES (?1, 'r1', 1, 'initial', 'final', 'u1', ?2, '{}', ?3, '{}', ?3, 'tox-report', 2, \
         'rendering', 1)",
        params![id, AT, SHA],
    )
}

fn set_ready(conn: &Connection, id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE rpt_candidates SET state = 'ready', artifact_sha256 = ?2, artifact_size = 10, \
         template_sha256 = ?2, renderer_version = 'typst 0.15.1', fonts_json = '[]', \
         rendered_at = ?3, revision = revision + 1 WHERE id = ?1",
        params![id, SHA, AT],
    )
}

fn release(conn: &Connection, id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE rpt_candidates SET state = 'released', revision = revision + 1 WHERE id = ?1",
        [id],
    )
}

fn insert_revision(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO rpt_revisions (id, report_id, revision_no, candidate_id, relationship, \
         completeness, content_sha256, artifact_sha256, released_by, released_at) \
         VALUES ('v1', 'r1', 1, 'c1', 'initial', 'final', ?1, ?1, 'u1', ?2)",
        params![SHA, AT],
    )
}

fn release_event(conn: &Connection, authority: &str) {
    conn.execute(
        "INSERT INTO audit_events (stream, seq, prev_hash, hash, hash_alg, occurred_at, \
         recorded_at, principal_kind, principal_id, action, outcome, record_type, record_id, \
         new_revision, authority, correlation_id, details) \
         VALUES ('clinical', 1, ?1, ?1, 'sha256', ?2, ?2, 'user', 'u1', 'report.release', \
         'succeeded', 'report_revision', 'v1', 1, ?3, 'k1', '{}')",
        params![SHA, AT, authority],
    )
    .unwrap();
}

#[test]
fn candidate_content_is_immutable_and_states_move_forward_only() {
    let conn = schema();
    candidate(&conn, "c1").unwrap();
    // Content never changes.
    assert!(
        conn.execute(
            "UPDATE rpt_candidates SET content_sha256 = ?1, revision = 2 WHERE id = 'c1'",
            [SHA.replace('a', "b")],
        )
        .is_err()
    );
    // Rendering cannot jump to released.
    assert!(release(&conn, "c1").is_err());
    // One open candidate per report.
    assert!(candidate(&conn, "c2").is_err());
    set_ready(&conn, "c1").unwrap();
    // Render columns are set once.
    assert!(set_ready(&conn, "c1").is_err());
    release(&conn, "c1").unwrap();
    assert!(
        conn.execute(
            "UPDATE rpt_candidates SET state = 'superseded', revision = revision + 1 WHERE id = 'c1'",
            [],
        )
        .is_err()
    );
    assert!(conn.execute("DELETE FROM rpt_candidates", []).is_err());
}

// NFR-01, brief §2.1: a revision needs its released candidate and a
// succeeded `report.release` event by the releaser under `release_reports`.
#[test]
fn a_revision_needs_its_released_candidate_and_release_event() {
    let conn = schema();
    candidate(&conn, "c1").unwrap();
    set_ready(&conn, "c1").unwrap();
    assert!(insert_revision(&conn).is_err(), "candidate not released");
    release(&conn, "c1").unwrap();
    assert!(insert_revision(&conn).is_err(), "no release event");
    release_event(&conn, "manage_configuration");
    assert!(insert_revision(&conn).is_err(), "wrong authority");
    conn.execute("DELETE FROM audit_events", []).unwrap_err();
}

#[test]
fn a_released_revision_is_append_only() {
    let conn = schema();
    candidate(&conn, "c1").unwrap();
    set_ready(&conn, "c1").unwrap();
    release(&conn, "c1").unwrap();
    release_event(&conn, "release_reports");
    insert_revision(&conn).unwrap();
    assert!(
        conn.execute("UPDATE rpt_revisions SET amendment_reason = 'x'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM rpt_revisions", []).is_err());
    let availability: String = conn
        .query_row("SELECT availability FROM rpt_v_revisions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(availability, "current");
}

// REQ-RPT-012, L10: under the default separation-of-duties policy the
// creator cannot verify; a destination changes only by retirement, once.
#[test]
fn destinations_are_verified_by_a_second_person_and_only_retired() {
    let conn = schema();
    conn.execute(
        "INSERT INTO rpt_destinations (id, client_id, channel, address, label, created_by, \
         created_at, revision) VALUES ('d1', 'k1', 'fax', '555-0100', 'Fax', 'u1', ?1, 1)",
        [AT],
    )
    .unwrap();
    let verify = |by: &str| {
        conn.execute(
            "INSERT INTO rpt_destination_verifications VALUES ('d1', 'Test fax', ?1, ?2)",
            params![by, AT],
        )
    };
    assert!(verify("u1").is_err());
    assert!(
        conn.execute("UPDATE rpt_destinations SET address = '555-0199'", [])
            .is_err()
    );
    conn.execute(
        "UPDATE rpt_destinations SET retired_by = 'u2', retired_at = ?1, retire_reason = 'x', \
         revision = 2",
        [AT],
    )
    .unwrap();
    assert!(
        verify("u2").is_err(),
        "a retired destination is not verified"
    );
    // A pickup has no address; a fax needs one.
    assert!(
        conn.execute(
            "INSERT INTO rpt_destinations (id, client_id, channel, label, created_by, created_at, \
             revision) VALUES ('d2', 'k1', 'fax', 'Fax', 'u1', ?1, 1)",
            [AT],
        )
        .is_err()
    );
}

/// Appends separation-of-duties policy version `version`, allowing only
/// destination self-verification when `self_verify` is set.
fn set_policy(conn: &Connection, version: i64, self_verify: bool) {
    conn.execute(
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (?1, 1, 1, ?2, 'Synthetic policy', 'u9', ?3)",
        params![version, self_verify, AT],
    )
    .unwrap();
}

// REQ-RPT-012, ADR 0015: a direct insert of a creator's verification passes
// the guard only while the policy in force allows self-verification, and
// never for a retired destination.
#[test]
fn a_creator_verifies_directly_only_while_the_policy_allows_it() {
    let conn = schema();
    let destination = |id: &str| {
        conn.execute(
            "INSERT INTO rpt_destinations (id, client_id, channel, label, created_by, \
             created_at, revision) VALUES (?1, 'k1', 'pickup', 'Pickup', 'u1', ?2, 1)",
            params![id, AT],
        )
        .unwrap();
    };
    let verify = |id: &str| {
        conn.execute(
            "INSERT INTO rpt_destination_verifications VALUES (?1, 'Seen in person', 'u1', ?2)",
            params![id, AT],
        )
    };
    destination("d1");
    assert!(verify("d1").is_err(), "the default needs a second person");
    set_policy(&conn, 1, false);
    assert!(
        verify("d1").is_err(),
        "other duties' switches do not allow it"
    );
    set_policy(&conn, 2, true);
    verify("d1").unwrap();

    destination("d2");
    conn.execute(
        "UPDATE rpt_destinations SET retired_by = 'u1', retired_at = ?1, retire_reason = 'x', \
         revision = 2 WHERE id = 'd2'",
        [AT],
    )
    .unwrap();
    assert!(
        verify("d2").is_err(),
        "a retired destination is not verified"
    );
    set_policy(&conn, 3, false);
    destination("d3");
    assert!(verify("d3").is_err(), "the newest version governs");
}

// REQ-RPT-011: a failed attempt has a failure code; none is observed after
// it is recorded.
#[test]
fn attempts_carry_their_evidence_rules() {
    let conn = schema();
    let attempt = |evidence: &str, code: Option<&str>, observed: &str| {
        conn.execute(
            "INSERT INTO rpt_delivery_attempts (id, obligation_id, evidence, signal_source, \
             observed_at, failure_code, recorded_by, recorded_at) \
             VALUES (?1, 'o1', ?2, 'staff_attestation', ?3, ?4, 'u1', ?5)",
            params![
                format!("a-{evidence}-{observed}"),
                evidence,
                observed,
                code,
                AT
            ],
        )
    };
    assert!(attempt("failed", None, AT).is_err());
    assert!(attempt("handed_off", Some("returned"), AT).is_err());
    assert!(attempt("handed_off", None, "2026-10-12T15:00:00.000000000Z").is_err());
    assert!(attempt("delivered", None, AT).is_err());
    attempt("failed", Some("no_answer_or_busy"), AT).unwrap();
    assert!(
        conn.execute("DELETE FROM rpt_delivery_attempts", [])
            .is_err()
    );
}

#[test]
fn delivery_foreign_keys_have_lookup_indexes() {
    let conn = schema();
    for (index, expected_column) in [
        ("rpt_delivery_attempts_by_obligation", "obligation_id"),
        ("rpt_delivery_obligations_by_destination", "destination_id"),
    ] {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA index_info('{index}')"))
            .unwrap()
            .query_map([], |row| row.get(2))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(columns, [expected_column]);
    }
}

#[test]
fn staged_withdrawal_notice_reference_is_append_only() {
    let conn = schema();
    conn.execute(
        "INSERT INTO rpt_withdrawal_notice_artifacts (artifact_sha256, report_id, revision_no, \
         template_version, staged_by, staged_at) VALUES (?1, 'r1', 1, 1, 'u1', ?2)",
        params![SHA, AT],
    )
    .unwrap();
    assert!(
        conn.execute(
            "UPDATE rpt_withdrawal_notice_artifacts SET revision_no = 2",
            [],
        )
        .is_err()
    );
    assert!(
        conn.execute("DELETE FROM rpt_withdrawal_notice_artifacts", [])
            .is_err()
    );
}
