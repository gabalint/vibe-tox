//! Registration, matching, assertions and identifiers (REQ-IDN-001, INV-02).

use lis_core::{Capability, ErrorCode, Phi};
use lis_identity::{
    AddSubjectIdentifier, AssertDemographics, DeactivateSubjectIdentifier, IdentityError,
    SubjectMatchQuery, add_subject_identifier, assert_demographics, deactivate_subject_identifier,
    find_subject_matches, get_subject, list_open_identity_holds,
};

use crate::{
    assert_code, audit_actions, counts, demographics, identifier, open, read, register, run,
};

#[test]
fn register_subject_writes_assertion_identifiers_and_one_audit_event() {
    let t = open();
    let registered = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    )
    .unwrap();
    assert_eq!(registered.revision, 1);
    assert_eq!(registered.identity_hold_id, None);
    let subject = read(&t, move |conn| get_subject(conn, registered.subject_id));
    assert_eq!(subject.assertions.len(), 1);
    assert_eq!(subject.assertions[0].family_name.expose(), "Testperson");
    assert_eq!(subject.assertions[0].source, "requisition");
    assert_eq!(subject.assertions[0].asserted_by_kind, "user");
    assert_eq!(subject.identifiers.len(), 1);
    assert!(subject.identifiers[0].active);
    assert_eq!(audit_actions(&t), ["identity.register_subject"]);
}

#[test]
fn the_same_value_from_two_authorities_never_links_subjects() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "12345")],
        vec![],
    )
    .unwrap();
    let b = register(
        &t,
        demographics("Sampleperson", "Beta", "1985-05-05"),
        vec![identifier("CLINIC-B", "MR", "12345")],
        vec![],
    )
    .unwrap();
    assert_ne!(a.subject_id, b.subject_id);
    assert_eq!(b.identity_hold_id, None);
    let matches = read(&t, |conn| {
        find_subject_matches(
            conn,
            &SubjectMatchQuery {
                demographics: None,
                identifiers: vec![identifier("CLINIC-A", "MR", "12345")],
            },
        )
    });
    assert_eq!(matches.identifier_matches, [a.subject_id]);
    assert!(matches.candidates.is_empty());
}

#[test]
fn a_duplicate_active_identifier_is_refused_in_code_and_in_the_database() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    )
    .unwrap();
    let before = counts(&t);
    let again = register(
        &t,
        demographics("Sampleperson", "Beta", "1985-05-05"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    );
    assert!(matches!(again, Err(IdentityError::IdentifierInUse(id)) if id == a.subject_id));
    assert_eq!(counts(&t), before);

    let b = register(
        &t,
        demographics("Sampleperson", "Beta", "1985-05-05"),
        vec![],
        vec![],
    )
    .unwrap();
    let added = run(&t, &[Capability::EnterOrders], move |ctx| {
        add_subject_identifier(
            ctx,
            AddSubjectIdentifier {
                subject_id: b.subject_id,
                expected_revision: 1,
                identifier: identifier("CLINIC-A", "MR", "MRN-0001"),
            },
        )
    });
    assert_code(added, ErrorCode::Conflict);

    let conn = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    let unique = conn.execute(
        "INSERT INTO idn_subject_identifiers (id, subject_id, assigning_authority, \
         identifier_type, value, status, added_at) \
         VALUES ('x', ?1, 'CLINIC-A', 'MR', 'MRN-0001', 'active', 'now')",
        [b.subject_id],
    );
    let message = unique.unwrap_err().to_string();
    assert!(message.contains("cannot be transferred"), "{message}");
}

#[test]
fn a_deactivated_identifier_cannot_be_assigned_to_another_subject() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    )
    .unwrap();
    let identifier_id = read(&t, move |conn| get_subject(conn, a.subject_id)).identifiers[0].id;
    let changed = run(&t, &[Capability::ManageQuality], move |ctx| {
        deactivate_subject_identifier(
            ctx,
            DeactivateSubjectIdentifier {
                identifier_id,
                expected_subject_revision: 1,
                reason: "Mistyped at entry".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(changed.revision, 2);
    let twice = run(&t, &[Capability::ManageQuality], move |ctx| {
        deactivate_subject_identifier(
            ctx,
            DeactivateSubjectIdentifier {
                identifier_id,
                expected_subject_revision: 2,
                reason: "Again".to_owned(),
            },
        )
    });
    assert!(matches!(twice, Err(IdentityError::IdentifierInactive)));

    let b = register(
        &t,
        demographics("Sampleperson", "Beta", "1985-05-05"),
        vec![],
        vec![],
    )
    .unwrap();
    let reassigned = run(&t, &[Capability::EnterOrders], move |ctx| {
        add_subject_identifier(
            ctx,
            AddSubjectIdentifier {
                subject_id: b.subject_id,
                expected_revision: 1,
                identifier: identifier("CLINIC-A", "MR", "MRN-0001"),
            },
        )
    });
    assert!(matches!(
        reassigned,
        Err(IdentityError::IdentifierInUse(owner)) if owner == a.subject_id
    ));
    let subject = read(&t, move |conn| get_subject(conn, a.subject_id));
    assert!(!subject.identifiers[0].active);
    assert_eq!(
        subject.identifiers[0].deactivation_reason.as_deref(),
        Some("Mistyped at entry")
    );
    let conn = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    let direct = conn.execute(
        "INSERT INTO idn_subject_identifiers (id, subject_id, assigning_authority, \
         identifier_type, value, status, added_at) \
         VALUES ('reassigned', ?1, 'CLINIC-A', 'MR', 'MRN-0001', 'active', 'now')",
        [b.subject_id],
    );
    assert!(direct.is_err());
    assert_eq!(
        audit_actions(&t),
        [
            "identity.register_subject",
            "identity.deactivate_identifier",
            "identity.register_subject"
        ]
    );
}

#[test]
fn registering_with_candidates_needs_acknowledgement_then_opens_a_hold() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let before = counts(&t);
    // Case and surrounding spaces do not hide a candidate.
    let unacknowledged = register(
        &t,
        demographics("TESTPERSON ", "alpha", "1990-01-01"),
        vec![],
        vec![],
    );
    assert!(matches!(
        unacknowledged,
        Err(IdentityError::CandidatesNotAcknowledged)
    ));
    assert_eq!(counts(&t), before);

    let stale = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![lis_identity::SubjectId::generate()],
    );
    assert!(matches!(
        stale,
        Err(IdentityError::CandidatesNotAcknowledged)
    ));

    let b = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![a.subject_id],
    )
    .unwrap();
    let hold_id = b.identity_hold_id.unwrap();
    for subject in [a.subject_id, b.subject_id] {
        let open = read(&t, move |conn| list_open_identity_holds(conn, subject));
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, hold_id);
        assert_eq!(open[0].subject_id, b.subject_id);
        assert_eq!(open[0].candidate_subject_ids, [a.subject_id]);
    }
    assert_eq!(
        audit_actions(&t),
        [
            "identity.register_subject",
            "identity.register_subject",
            "identity.open_hold"
        ]
    );
}

#[test]
fn canonically_equivalent_names_are_candidates_and_preserve_entered_spelling() {
    let t = open();
    let composed = register(
        &t,
        demographics("García", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let decomposed_name = "Garci\u{301}a";
    let unacknowledged = register(
        &t,
        demographics(decomposed_name, "Alpha", "1990-01-01"),
        vec![],
        vec![],
    );
    assert!(matches!(
        unacknowledged,
        Err(IdentityError::CandidatesNotAcknowledged)
    ));
    let decomposed = register(
        &t,
        demographics(decomposed_name, "Alpha", "1990-01-01"),
        vec![],
        vec![composed.subject_id],
    )
    .unwrap();
    assert!(decomposed.identity_hold_id.is_some());
    let stored = read(&t, move |conn| get_subject(conn, decomposed.subject_id));
    assert_eq!(stored.assertions[0].family_name.expose(), decomposed_name);
}

#[test]
fn acknowledgement_is_not_needed_for_unrelated_subjects() {
    let t = open();
    register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let other_day = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-02"),
        vec![],
        vec![],
    )
    .unwrap();
    assert_eq!(other_day.identity_hold_id, None);
    let none = read(&t, move |conn| {
        list_open_identity_holds(conn, other_day.subject_id)
    });
    assert!(none.is_empty());
}

#[test]
fn a_demographic_change_adds_an_assertion_and_keeps_the_first() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let changed = run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(
            ctx,
            AssertDemographics {
                subject_id: a.subject_id,
                expected_revision: 1,
                demographics: demographics("Testperson-Renamed", "Alpha", "1990-01-01"),
                source: "client update".to_owned(),
                reason: "Name change reported by client".to_owned(),
                acknowledged_candidates: vec![],
            },
        )
    })
    .unwrap();
    assert_eq!(changed.revision, 2);
    let subject = read(&t, move |conn| get_subject(conn, a.subject_id));
    assert_eq!(subject.revision, 2);
    assert_eq!(subject.assertions.len(), 2);
    assert_eq!(subject.assertions[0].family_name.expose(), "Testperson");
    assert_eq!(subject.assertions[0].reason, None);
    assert_eq!(
        subject.assertions[1].family_name.expose(),
        "Testperson-Renamed"
    );
    assert_eq!(
        subject.assertions[1].reason.as_deref(),
        Some("Name change reported by client")
    );
    // An earlier name still lists the subject as a candidate.
    let matches = read(&t, |conn| {
        find_subject_matches(
            conn,
            &SubjectMatchQuery {
                demographics: Some(demographics("Testperson", "Alpha", "1990-01-01")),
                identifiers: vec![],
            },
        )
    });
    assert_eq!(matches.candidates, [a.subject_id]);
}

#[test]
fn demographic_change_to_an_existing_identity_needs_acknowledgement_and_opens_hold() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let b = register(
        &t,
        demographics("Sampleperson", "Beta", "1985-05-05"),
        vec![],
        vec![],
    )
    .unwrap();
    let before = counts(&t);
    let unacknowledged_input = AssertDemographics {
        subject_id: b.subject_id,
        expected_revision: 1,
        demographics: demographics("Testperson", "Alpha", "1990-01-01"),
        source: "client update".to_owned(),
        reason: "Supplied demographic correction".to_owned(),
        acknowledged_candidates: vec![],
    };
    let unacknowledged = run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(ctx, unacknowledged_input)
    });
    assert!(matches!(
        unacknowledged,
        Err(IdentityError::CandidatesNotAcknowledged)
    ));
    assert_eq!(counts(&t), before);

    let acknowledged_input = AssertDemographics {
        subject_id: b.subject_id,
        expected_revision: 1,
        demographics: demographics("Testperson", "Alpha", "1990-01-01"),
        source: "client update".to_owned(),
        reason: "Supplied demographic correction".to_owned(),
        acknowledged_candidates: vec![a.subject_id],
    };
    let changed = run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(ctx, acknowledged_input)
    })
    .unwrap();
    assert_eq!(changed.revision, 2);
    for subject_id in [a.subject_id, b.subject_id] {
        let holds = read(&t, move |conn| list_open_identity_holds(conn, subject_id));
        assert_eq!(holds.len(), 1);
        assert_eq!(holds[0].subject_id, b.subject_id);
        assert_eq!(holds[0].candidate_subject_ids, [a.subject_id]);
    }
}

#[test]
fn a_stale_revision_is_a_conflict_and_writes_nothing() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let before = counts(&t);
    let stale = run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(
            ctx,
            AssertDemographics {
                subject_id: a.subject_id,
                expected_revision: 0,
                demographics: demographics("Testperson", "Gamma", "1990-01-01"),
                source: "client update".to_owned(),
                reason: "Correction".to_owned(),
                acknowledged_candidates: vec![],
            },
        )
    });
    assert!(matches!(stale, Err(IdentityError::RevisionConflict)));
    assert_eq!(counts(&t), before);
}

#[test]
fn a_demographic_change_needs_a_reason() {
    let t = open();
    let a = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    let result = run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(
            ctx,
            AssertDemographics {
                subject_id: a.subject_id,
                expected_revision: 1,
                demographics: demographics("Testperson", "Gamma", "1990-01-01"),
                source: "client update".to_owned(),
                reason: "  ".to_owned(),
                acknowledged_candidates: vec![],
            },
        )
    });
    assert!(matches!(result, Err(IdentityError::Invalid(field)) if field.field == "reason"));
}

#[test]
fn assertions_cannot_be_edited_with_sql() {
    let t = open();
    register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    )
    .unwrap();
    let conn = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    for sql in [
        "UPDATE idn_identity_assertions SET family_name = 'Other'",
        "DELETE FROM idn_identity_assertions",
        "DELETE FROM idn_subject_identifiers",
        "UPDATE idn_subject_identifiers SET value = 'MRN-9999'",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn audit_events_and_errors_hold_no_phi() {
    let t = open();
    let canary = "SYNTHETIC-CANARY";
    let a = register(
        &t,
        demographics(canary, canary, "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", canary)],
        vec![],
    )
    .unwrap();
    let error = register(
        &t,
        demographics(canary, canary, "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", canary)],
        vec![a.subject_id],
    )
    .unwrap_err();
    assert!(!format!("{error} {error:?}").contains("CANARY"));
    let input = demographics(canary, canary, "1990-01-01");
    assert!(!format!("{input:?}").contains("CANARY"));
    let leaked: i64 = read(&t, |conn| {
        Ok(conn.query_row(
            "SELECT count(*) FROM audit_events WHERE instr(details || coalesce(reason, '') \
             || coalesce(record_id, ''), 'CANARY') > 0",
            [],
            |row| row.get(0),
        )?)
    });
    assert_eq!(leaked, 0);
    assert_eq!(Phi::new(canary).to_string(), Phi::<&str>::REDACTED);
}
