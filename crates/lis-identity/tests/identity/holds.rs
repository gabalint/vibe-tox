//! Possible-duplicate hold review (REQ-IDN-001).

use lis_core::{Capability, ErrorCode, Principal};
use lis_identity::{
    AddSubjectIdentifier, ConfirmDistinctSubjects, IdentityError, IdentityHoldId,
    add_subject_identifier, confirm_distinct_subjects, identity_holds_touching,
    list_open_identity_holds,
};
use lis_store::testing::TestDatabase;

use crate::{
    assert_code, audit_actions, counts, demographics, identifier, open, read, register, run, run_as,
};

/// Two subjects with the same name and birth date and the hold between them.
fn duplicate_pair(
    t: &TestDatabase,
) -> (
    lis_identity::SubjectId,
    lis_identity::SubjectId,
    IdentityHoldId,
) {
    let a = register(
        t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let b = register(
        t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![a.subject_id],
    )
    .unwrap();
    (a.subject_id, b.subject_id, b.identity_hold_id.unwrap())
}

fn confirm(
    t: &TestDatabase,
    hold_id: IdentityHoldId,
    expected_revision: i64,
) -> Result<lis_identity::IdentityHoldResolved, IdentityError> {
    run(t, &[Capability::ManageQuality], move |ctx| {
        confirm_distinct_subjects(
            ctx,
            ConfirmDistinctSubjects {
                hold_id,
                expected_revision,
                reason: "Different addresses and insurance on file".to_owned(),
            },
        )
    })
}

#[test]
fn confirming_distinct_subjects_resolves_the_hold() {
    let t = open();
    let (a, b, hold_id) = duplicate_pair(&t);
    let resolved = confirm(&t, hold_id, 1).unwrap();
    assert_eq!(resolved.revision, 2);
    for subject in [a, b] {
        assert!(read(&t, move |conn| list_open_identity_holds(conn, subject)).is_empty());
    }
    assert_eq!(
        audit_actions(&t).last().unwrap(),
        "identity.confirm_distinct"
    );
    let (resolution, by_kind): (String, String) = read(&t, move |conn| {
        Ok(conn.query_row(
            "SELECT resolution, resolved_by_kind FROM idn_identity_holds WHERE id = ?1",
            [hold_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    assert_eq!(
        (resolution.as_str(), by_kind.as_str()),
        ("confirmed_distinct", "user")
    );
}

#[test]
fn a_resolved_hold_cannot_be_resolved_again_or_edited() {
    let t = open();
    let (_, _, hold_id) = duplicate_pair(&t);
    confirm(&t, hold_id, 1).unwrap();
    let before = counts(&t);
    assert!(matches!(
        confirm(&t, hold_id, 2),
        Err(IdentityError::HoldResolved)
    ));
    assert_eq!(counts(&t), before);
    let conn = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    for sql in [
        "UPDATE idn_identity_holds SET status = 'open', resolution = NULL, resolution_reason = NULL, \
         resolved_at = NULL, resolved_by_kind = NULL, resolved_by_id = NULL",
        "DELETE FROM idn_identity_holds",
        "DELETE FROM idn_identity_hold_candidates",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn a_stale_hold_revision_is_a_conflict() {
    let t = open();
    let (_, b, hold_id) = duplicate_pair(&t);
    assert!(matches!(
        confirm(&t, hold_id, 7),
        Err(IdentityError::RevisionConflict)
    ));
    assert_eq!(
        read(&t, move |conn| list_open_identity_holds(conn, b)).len(),
        1
    );
}

#[test]
fn participant_changes_make_the_hold_evidence_stale() {
    for mutate_primary in [true, false] {
        let t = open();
        let (candidate, primary, hold_id) = duplicate_pair(&t);
        let changed = if mutate_primary { primary } else { candidate };
        run(&t, &[Capability::EnterOrders], move |ctx| {
            add_subject_identifier(
                ctx,
                AddSubjectIdentifier {
                    subject_id: changed,
                    expected_revision: 1,
                    identifier: identifier("CLINIC-A", "MR", "SYN-NEW"),
                },
            )
        })
        .unwrap();
        assert!(matches!(
            confirm(&t, hold_id, 1),
            Err(IdentityError::RevisionConflict)
        ));
        for subject_id in [candidate, primary] {
            assert_eq!(
                read(&t, move |conn| list_open_identity_holds(conn, subject_id)).len(),
                1
            );
        }
    }
}

#[test]
fn a_system_principal_cannot_resolve_an_identity_hold() {
    let t = open();
    let (_, primary, hold_id) = duplicate_pair(&t);
    let before = counts(&t);
    let result = run_as(
        &t,
        Principal::system("identity-review-worker", [Capability::ManageQuality]),
        move |ctx| {
            confirm_distinct_subjects(
                ctx,
                ConfirmDistinctSubjects {
                    hold_id,
                    expected_revision: 1,
                    reason: "Automated decision".to_owned(),
                },
            )
        },
    );
    assert_code(result, ErrorCode::Forbidden);
    assert_eq!(counts(&t), before);
    assert_eq!(
        read(&t, move |conn| list_open_identity_holds(conn, primary)).len(),
        1
    );

    let conn = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    let direct = conn.execute(
        "UPDATE idn_identity_holds SET status = 'resolved', resolution = 'confirmed_distinct', \
         revision = 2, resolved_at = 'now', resolution_reason = 'Automated decision', \
         resolved_by_kind = 'system', resolved_by_id = 'worker' WHERE id = ?1",
        [hold_id],
    );
    assert!(direct.is_err());
}

#[test]
fn an_unknown_hold_is_not_found() {
    let t = open();
    let result = confirm(&t, IdentityHoldId::generate(), 1);
    assert!(matches!(
        result,
        Err(IdentityError::NotFound("identity hold"))
    ));
}

#[test]
fn hold_history_keeps_resolved_holds_for_every_participant() {
    let t = open();
    let (a, b, hold_id) = duplicate_pair(&t);
    let unrelated = register(
        &t,
        demographics("Otherperson", "Beta", "1985-05-05"),
        vec![],
        vec![],
    )
    .unwrap()
    .subject_id;
    confirm(&t, hold_id, 1).unwrap();
    for subject in [a, b] {
        let history = read(&t, move |conn| identity_holds_touching(conn, subject));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].hold.id, hold_id);
        assert_eq!(history[0].hold.candidate_subject_ids, [a]);
        assert!(history[0].resolved_at.unwrap() >= history[0].hold.opened_at);
    }
    assert!(read(&t, move |conn| identity_holds_touching(conn, unrelated)).is_empty());
    assert!(read(&t, move |conn| list_open_identity_holds(conn, a)).is_empty());
}
