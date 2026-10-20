//! Subject search, summaries and the open-hold review list (REQ-IDN-001).

use lis_core::{Capability, ErrorCode, Phi};
use lis_identity::{
    AssertDemographics, IdentityError, SubjectId, SubjectMatchQuery, SubjectSearch,
    assert_demographics, get_subject, list_all_open_identity_holds, match_subjects,
    search_subjects, subject_summary,
};
use lis_store::testing::TestDatabase;

use crate::{assert_code, demographics, identifier, open, read, register, run};

fn search(t: &TestDatabase, input: SubjectSearch) -> Result<Vec<SubjectId>, IdentityError> {
    t.db.read_blocking(move |conn| search_subjects(conn, &input))
        .map(|found| found.into_iter().map(|s| s.subject_id).collect())
}

fn by_name(family: &str, given: Option<&str>, birth: Option<&str>) -> SubjectSearch {
    SubjectSearch {
        query: None,
        offset: None,
        identifier: None,
        family_name: Some(Phi::new(family.to_owned())),
        given_names: given.map(|name| Phi::new(name.to_owned())),
        birth_date: birth.map(|date| Phi::new(date.to_owned())),
        limit: 10,
    }
}

/// Three synthetic subjects; the first carries an MRN.
fn three(t: &TestDatabase) -> [SubjectId; 3] {
    let a = register(
        t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("SYN-CLINIC", "MR", "SYN-0001")],
        vec![],
    )
    .unwrap();
    let b = register(
        t,
        demographics("Testpersonson", "Beta", "1991-02-02"),
        vec![],
        vec![],
    )
    .unwrap();
    let c = register(
        t,
        demographics("García", "Gamma", "1990-01-01"),
        vec![],
        vec![],
    )
    .unwrap();
    [a.subject_id, b.subject_id, c.subject_id]
}

#[test]
fn search_finds_by_identifier_name_prefix_and_birth_date() {
    let t = open();
    let [a, b, c] = three(&t);
    let by_identifier = SubjectSearch {
        query: None,
        offset: None,
        identifier: Some(identifier("SYN-CLINIC", "MR", "SYN-0001")),
        family_name: None,
        given_names: None,
        birth_date: None,
        limit: 10,
    };
    assert_eq!(search(&t, by_identifier).unwrap(), [a]);
    assert_eq!(
        search(&t, by_name("testperson", None, None)).unwrap(),
        [a, b]
    );
    assert_eq!(search(&t, by_name(" TESTP", None, None)).unwrap(), [a, b]);
    assert_eq!(
        search(&t, by_name("Testperson", Some("al"), None)).unwrap(),
        [a]
    );
    assert_eq!(
        search(&t, by_name("Test", None, Some("1991-02-02"))).unwrap(),
        [b]
    );
    // Unicode composition does not matter: decomposed input finds the composed name.
    assert_eq!(
        search(&t, by_name("Garci\u{301}a", None, None)).unwrap(),
        [c]
    );
    assert!(
        search(&t, by_name("Nobody", None, None))
            .unwrap()
            .is_empty()
    );
    let mut limited = by_name("Test", None, None);
    limited.limit = 1;
    assert_eq!(search(&t, limited).unwrap(), [a]);
}

#[test]
fn search_finds_former_names_and_shows_the_current_one() {
    let t = open();
    let [a, ..] = three(&t);
    run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(
            ctx,
            AssertDemographics {
                subject_id: a,
                expected_revision: 1,
                demographics: demographics("Renamedperson", "Alpha", "1990-01-01"),
                source: "requisition".to_owned(),
                reason: "Name corrected on the requisition".to_owned(),
                acknowledged_candidates: vec![],
            },
        )
    })
    .unwrap();
    let found = read(&t, |conn| {
        search_subjects(conn, &by_name("Testperson", Some("Alpha"), None))
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].family_name.expose(), "Renamedperson");
    assert_eq!(found[0].revision, 2);
}

#[test]
fn search_refuses_invalid_or_unbounded_criteria() {
    let t = open();
    for (input, field) in [
        (by_name("", Some("Alpha"), None), "familyName"),
        (by_name("Test", None, Some("1990-13-01")), "birthDate"),
        (
            SubjectSearch {
                limit: 51,
                ..by_name("Test", None, None)
            },
            "limit",
        ),
        (
            SubjectSearch {
                limit: 0,
                ..by_name("Test", None, None)
            },
            "limit",
        ),
    ] {
        let error = search(&t, input).unwrap_err();
        let IdentityError::Invalid(found) = &error else {
            panic!("{error:?}")
        };
        assert_eq!(found.field, field);
    }
}

#[test]
fn summaries_show_current_demographics_active_identifiers_and_holds() {
    let t = open();
    let [a, ..] = three(&t);
    let duplicate = register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![a],
    )
    .unwrap();
    let summary = read(&t, move |conn| subject_summary(conn, a));
    assert_eq!(summary.given_names.unwrap().expose(), "Alpha");
    assert_eq!(summary.identifiers.len(), 1);
    assert_eq!(summary.open_identity_holds, 1);

    let detail = read(&t, move |conn| get_subject(conn, a));
    assert_eq!(detail.open_holds.len(), 1);
    assert_eq!(detail.open_holds[0].id, duplicate.identity_hold_id.unwrap());
    let all = read(&t, |conn| list_all_open_identity_holds(conn, 10));
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].candidate_subject_ids, [a]);
    // The review list is bounded and keeps the oldest hold first.
    register(
        &t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![a, duplicate.subject_id],
    )
    .unwrap();
    let oldest = read(&t, |conn| list_all_open_identity_holds(conn, 1));
    assert_eq!(oldest.len(), 1);
    assert_eq!(oldest[0].id, duplicate.identity_hold_id.unwrap());
    assert_eq!(
        read(&t, |conn| list_all_open_identity_holds(conn, 10)).len(),
        2
    );
    assert_code(
        t.db.read_blocking(|conn| subject_summary(conn, SubjectId::generate())),
        ErrorCode::NotFound,
    );
}

#[test]
fn match_candidates_are_summarized_separately_from_identifier_matches() {
    let t = open();
    let [a, _, c] = three(&t);
    let query = SubjectMatchQuery {
        demographics: Some(demographics("García", "Gamma", "1990-01-01")),
        identifiers: vec![identifier("SYN-CLINIC", "MR", "SYN-0001")],
    };
    let matches = read(&t, move |conn| match_subjects(conn, &query));
    let ids = |list: &[lis_identity::SubjectSummary]| -> Vec<SubjectId> {
        list.iter().map(|s| s.subject_id).collect()
    };
    assert_eq!(ids(&matches.identifier_matches), [a]);
    assert_eq!(ids(&matches.candidates), [c]);
}

fn directory(query: &str, offset: u32, limit: u32) -> SubjectSearch {
    SubjectSearch {
        query: Some(Phi::new(query.to_owned())),
        offset: Some(offset),
        family_name: None,
        limit,
        ..by_name("", None, None)
    }
}

#[test]
fn directory_browses_all_patients_in_current_name_order_without_duplicates() {
    let t = open();
    assert!(search(&t, directory("", 0, 2)).unwrap().is_empty());
    let [a, b, c] = three(&t);
    assert_eq!(search(&t, directory("", 0, 2)).unwrap(), [c, a]);
    assert_eq!(search(&t, directory("  ", 2, 2)).unwrap(), [b]);
    assert!(search(&t, directory("", 3, 2)).unwrap().is_empty());
    assert!(search(&t, directory("", u32::MAX, 2)).unwrap().is_empty());
    let mut birth_only = directory("", 0, 10);
    birth_only.birth_date = Some(Phi::new("1990-01-01".to_owned()));
    assert_eq!(search(&t, birth_only).unwrap(), [c, a]);
}

#[test]
fn simple_search_matches_name_forms_and_literal_active_identifier_prefixes() {
    let t = open();
    let [a, b, c] = three(&t);
    for query in [
        "Alpha",
        "ALPHA TESTP",
        "Testperson Al",
        "Testperson, Al",
        "SYN-00",
    ] {
        assert_eq!(search(&t, directory(query, 0, 10)).unwrap(), [a], "{query}");
    }
    assert_eq!(search(&t, directory("GARCÍA", 0, 10)).unwrap(), [c]);
    assert_eq!(search(&t, directory("Garci\u{301}a", 0, 10)).unwrap(), [c]);
    assert_eq!(search(&t, directory("test", 0, 1)).unwrap(), [a]);
    assert_eq!(search(&t, directory("test", 1, 1)).unwrap(), [b]);
    // Identifier search is literal and case-sensitive, never an identity link.
    for query in ["syn-00", "%", "_", "\\", "Test%", "SYN_", "' OR 1=1 --"] {
        assert!(
            search(&t, directory(query, 0, 10)).unwrap().is_empty(),
            "{query}"
        );
    }
    let literal = register(
        &t,
        demographics("Percent%_\\", "Literal", "1992-01-01"),
        vec![identifier("SYN-CLINIC", "MR", "ID%_\\")],
        vec![],
    )
    .unwrap();
    for query in ["Percent%_\\", "ID%_\\"] {
        assert_eq!(
            search(&t, directory(query, 0, 10)).unwrap(),
            [literal.subject_id]
        );
    }
    let summary = read(&t, move |conn| subject_summary(conn, a));
    run(&t, &[Capability::ManageQuality], move |ctx| {
        lis_identity::deactivate_subject_identifier(
            ctx,
            lis_identity::DeactivateSubjectIdentifier {
                identifier_id: summary.identifiers[0].id,
                expected_subject_revision: summary.revision,
                reason: "Synthetic identifier correction".to_owned(),
            },
        )
    })
    .unwrap();
    assert!(search(&t, directory("SYN-00", 0, 10)).unwrap().is_empty());
}

#[test]
fn search_pages_exact_identifier_first_and_historical_names_only_once() {
    let t = open();
    let [a, b, c] = three(&t);
    let mut search_input = by_name("", None, None);
    search_input.identifier = Some(identifier("SYN-CLINIC", "MR", "SYN-0001"));
    // A missing exact identifier must not accidentally turn into browse-all.
    search_input.identifier.as_mut().unwrap().value = Phi::new("MISSING".to_owned());
    assert!(search(&t, search_input.clone()).unwrap().is_empty());
    search_input.identifier.as_mut().unwrap().value = Phi::new("SYN-0001".to_owned());
    search_input.family_name = Some(Phi::new("Gar".to_owned()));
    search_input.limit = 1;
    assert_eq!(search(&t, search_input.clone()).unwrap(), [a]);
    search_input.offset = Some(1);
    assert_eq!(search(&t, search_input).unwrap(), [c]);
    run(&t, &[Capability::ManageQuality], move |ctx| {
        assert_demographics(
            ctx,
            AssertDemographics {
                subject_id: a,
                expected_revision: 1,
                demographics: demographics("Zedperson", "Changed", "1990-01-01"),
                source: "requisition".to_owned(),
                reason: "Corrected synthetic name".to_owned(),
                acknowledged_candidates: vec![],
            },
        )
    })
    .unwrap();
    assert_eq!(search(&t, directory("Alpha Test", 0, 10)).unwrap(), [a]);
    assert_eq!(search(&t, directory("test", 0, 10)).unwrap(), [b, a]);
    assert_eq!(search(&t, directory("", 0, 10)).unwrap(), [c, b, a]);
}

#[test]
fn directory_validates_query_birth_date_and_deserialized_offsets() {
    let t = open();
    let mut invalid = directory(&"x".repeat(403), 0, 10);
    assert_code(search(&t, invalid.clone()), ErrorCode::Validation);
    invalid.query = None;
    invalid.birth_date = Some(Phi::new("not-a-date".to_owned()));
    assert_code(search(&t, invalid), ErrorCode::Validation);
    for offset in ["-1", "4294967296", "1.5"] {
        assert!(
            serde_json::from_str::<SubjectSearch>(&format!("{{\"limit\":10,\"offset\":{offset}}}"))
                .is_err()
        );
    }
    let defaulted: SubjectSearch = serde_json::from_str("{\"limit\":10}").unwrap();
    assert!(search(&t, defaulted).unwrap().is_empty());
}
