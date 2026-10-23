//! Identity IPC through the engine (REQ-IDN-001, REQ-OPS-001): patient search
//! and registration as an accessioner, hold review as a quality manager,
//! clients and practitioners as an administrator. Synthetic data only.

use lis_auth::RoleCode;
use lis_core::{CommandError, ErrorCode, Phi, UserId};
use lis_engine::{
    AddClientLocation, AddSubjectIdentifier, AssertDemographics, ConfirmDistinctSubjects,
    CreateClient, DeactivateSubjectIdentifier, Engine, GetSubjectInput, ListIdentityHoldsInput,
    RegisterPractitioner, RegisterSubject, SetLaboratoryProfile, SubjectMatchQuery, SubjectSearch,
};
use lis_identity::{DemographicsInput, IdentifierInput, IdentityHoldId, SubjectIdentifierId};

use crate::fixture::{TestEngine, code_of};

/// A synthetic name that must never reach an audit event or a log.
const CANARY: &str = "Syntheticcanary";

fn demographics(birth_date: &str) -> DemographicsInput {
    DemographicsInput {
        family_name: Phi::new(CANARY.to_owned()),
        given_names: Some(Phi::new("Alpha".to_owned())),
        birth_date: Phi::new(birth_date.to_owned()),
        administrative_sex: None,
    }
}

fn mrn(value: &str) -> IdentifierInput {
    IdentifierInput {
        assigning_authority: "SYN-CLINIC".to_owned(),
        identifier_type: "MR".to_owned(),
        value: Phi::new(value.to_owned()),
    }
}

fn registration(identifiers: Vec<IdentifierInput>) -> RegisterSubject {
    RegisterSubject {
        demographics: demographics("1990-01-01"),
        source: "requisition".to_owned(),
        identifiers,
        acknowledged_candidates: vec![],
    }
}

fn search(family_name: &str) -> SubjectSearch {
    SubjectSearch {
        query: None,
        offset: None,
        identifier: None,
        family_name: Some(Phi::new(family_name.to_owned())),
        given_names: None,
        birth_date: None,
        limit: 10,
    }
}

/// Calls the identity command `name` with plausible input; `None` for other areas.
pub(crate) async fn identity_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let subject_id = lis_identity::SubjectId::generate();
    Some(match name {
        "search_subjects" => engine.search_subjects(search("")).await.map(drop),
        "match_subject_candidates" => engine
            .match_subject_candidates(SubjectMatchQuery {
                demographics: Some(demographics("1990-01-01")),
                identifiers: vec![],
            })
            .await
            .map(drop),
        "register_subject" => engine
            .register_subject(registration(vec![]))
            .await
            .map(drop),
        "get_subject" => engine
            .get_subject(GetSubjectInput { subject_id })
            .await
            .map(drop),
        "assert_demographics" => engine
            .assert_demographics(AssertDemographics {
                subject_id,
                expected_revision: 1,
                demographics: demographics("1990-01-02"),
                source: "requisition".to_owned(),
                reason: "Corrected".to_owned(),
                acknowledged_candidates: vec![],
            })
            .await
            .map(drop),
        "add_subject_identifier" => engine
            .add_subject_identifier(AddSubjectIdentifier {
                subject_id,
                expected_revision: 1,
                identifier: mrn("SYN-9"),
            })
            .await
            .map(drop),
        "deactivate_subject_identifier" => engine
            .deactivate_subject_identifier(DeactivateSubjectIdentifier {
                identifier_id: SubjectIdentifierId::generate(),
                expected_subject_revision: 1,
                reason: "Mistyped".to_owned(),
            })
            .await
            .map(drop),
        "list_identity_holds" => engine
            .list_identity_holds(ListIdentityHoldsInput { subject_id: None })
            .await
            .map(drop),
        "resolve_identity_hold" => engine
            .resolve_identity_hold(ConfirmDistinctSubjects {
                hold_id: IdentityHoldId::generate(),
                expected_revision: 1,
                reason: "Different people".to_owned(),
            })
            .await
            .map(drop),
        _ => return organization_call(engine, name).await,
    })
}

async fn organization_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_clients" => engine.list_clients().await.map(drop),
        "create_client" => engine
            .create_client(CreateClient {
                code: "SYN-CL".to_owned(),
                name: "Synthetic clinic".to_owned(),
            })
            .await
            .map(drop),
        "add_client_location" => engine
            .add_client_location(AddClientLocation {
                client_id: lis_identity::ClientId::generate(),
                code: "SYN-LOC".to_owned(),
                name: "Synthetic location".to_owned(),
            })
            .await
            .map(drop),
        "list_practitioners" => engine.list_practitioners().await.map(drop),
        "get_laboratory_profile" => engine.get_laboratory_profile().await.map(drop),
        "set_laboratory_profile" => engine
            .set_laboratory_profile(laboratory_profile(None))
            .await
            .map(drop),
        "register_practitioner" => engine
            .register_practitioner(RegisterPractitioner {
                npi: "1234567893".to_owned(),
                family_name: "Synthetic".to_owned(),
                given_names: None,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// A synthetic laboratory profile against `expected_version`.
fn laboratory_profile(expected_version: Option<i64>) -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version,
        name: "Synthetic Toxicology Laboratory".to_owned(),
        address_lines: vec!["100 Synthetic Way".to_owned()],
        clia_id: "00D0000000".to_owned(),
        director: "Director Synthetic".to_owned(),
        phone: "555-0100".to_owned(),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}

/// An administrator, an accessioner and a quality manager; the accessioner is signed in.
/// Returns the administrator's and the quality manager's TOTP secrets.
async fn staff(t: &TestEngine) -> (String, String) {
    let admin = t.admin_session().await;
    t.create_user("qm1", &[RoleCode::QualityManager]).await;
    t.create_user("acc1", &[RoleCode::Accessioner]).await;
    let (_, quality) = t.first_sign_in("qm1").await;
    t.first_sign_in("acc1").await;
    (admin, quality)
}

/// The `details` and `reason` text of every audit event.
fn audit_text(t: &TestEngine) -> String {
    t.conn()
        .prepare("SELECT coalesce(details, '') || coalesce(reason, '') FROM audit_events")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n")
}

#[tokio::test]
async fn an_accessioner_registers_searches_and_opens_a_subject_with_audited_reads() {
    let t = TestEngine::start().await;
    staff(&t).await;
    let registered = t
        .engine
        .register_subject(registration(vec![mrn("SYN-0001")]))
        .await
        .unwrap();
    assert_eq!(registered.identity_hold_id, None);

    let found = t
        .engine
        .search_subjects(search("syntheticc"))
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].subject_id, registered.subject_id);
    assert_eq!(found[0].family_name.expose(), CANARY);
    let detail = t
        .engine
        .get_subject(GetSubjectInput {
            subject_id: registered.subject_id,
        })
        .await
        .unwrap();
    assert_eq!(detail.identifiers.len(), 1);

    let access: Vec<(String, String)> = t.events("access");
    assert_eq!(
        access[access.len() - 2..],
        [
            (
                "identity.search_subjects".to_owned(),
                "succeeded".to_owned()
            ),
            ("identity.get_subject".to_owned(), "succeeded".to_owned()),
        ]
    );
    let record: String = t
        .conn()
        .query_row(
            "SELECT record_id FROM audit_events WHERE action = 'identity.get_subject'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(record, registered.subject_id.to_string());
    let details: String = t
        .conn()
        .query_row(
            "SELECT details FROM audit_events WHERE action = 'identity.search_subjects'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(details.contains("\"returned\":1"), "{details}");
    assert!(!audit_text(&t).contains(CANARY));
}

#[tokio::test]
async fn a_possible_duplicate_is_acknowledged_held_and_resolved_by_quality() {
    let t = TestEngine::start().await;
    let (_, quality) = staff(&t).await;
    let first = t
        .engine
        .register_subject(registration(vec![]))
        .await
        .unwrap();
    let unacknowledged = t.engine.register_subject(registration(vec![])).await;
    assert_eq!(code_of(unacknowledged), ErrorCode::Conflict);
    let candidates = t
        .engine
        .match_subject_candidates(SubjectMatchQuery {
            demographics: Some(demographics("1990-01-01")),
            identifiers: vec![],
        })
        .await
        .unwrap();
    assert_eq!(candidates.candidates.len(), 1);
    assert_eq!(candidates.candidates[0].subject_id, first.subject_id);
    let mut acknowledged = registration(vec![]);
    acknowledged.acknowledged_candidates = vec![first.subject_id];
    let second = t.engine.register_subject(acknowledged).await.unwrap();
    let hold_id = second.identity_hold_id.unwrap();

    // Hold review needs manage_quality, which an accessioner lacks.
    let holds = t
        .engine
        .list_identity_holds(ListIdentityHoldsInput { subject_id: None })
        .await;
    assert_eq!(code_of(holds), ErrorCode::Forbidden);
    t.sign_in_with("qm1", crate::fixture::NEW_PASSWORD, &quality)
        .await;
    let holds = t
        .engine
        .list_identity_holds(ListIdentityHoldsInput {
            subject_id: Some(first.subject_id),
        })
        .await
        .unwrap();
    assert_eq!(holds.len(), 1);
    assert_eq!(holds[0].id, hold_id);
    t.engine
        .resolve_identity_hold(ConfirmDistinctSubjects {
            hold_id,
            expected_revision: holds[0].revision,
            reason: "Different insurance and address on the requisitions".to_owned(),
        })
        .await
        .unwrap();
    let open = t
        .engine
        .list_identity_holds(ListIdentityHoldsInput { subject_id: None })
        .await
        .unwrap();
    assert!(open.is_empty());
    let hold_reads = t
        .events("access")
        .into_iter()
        .filter(|(action, _)| action == "identity.list_holds")
        .count();
    assert_eq!(hold_reads, 2);
    let actions = t.events("clinical");
    assert_eq!(actions.last().unwrap().0, "identity.confirm_distinct");
}

#[tokio::test]
async fn invalid_identity_input_returns_field_errors_without_the_value() {
    let t = TestEngine::start().await;
    staff(&t).await;
    let mut input = registration(vec![]);
    input.demographics.birth_date = Phi::new("SYN-NOT-A-DATE".to_owned());
    let error = t.engine.register_subject(input).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "birthDate");
    assert_eq!(json["message"], "Check the highlighted fields.");
    assert!(!json.to_string().contains("SYN-NOT-A-DATE"));
    let unknown = t
        .engine
        .get_subject(GetSubjectInput {
            subject_id: lis_identity::SubjectId::generate(),
        })
        .await;
    assert_eq!(code_of(unknown), ErrorCode::NotFound);
}

#[tokio::test]
async fn an_administrator_configures_clients_and_practitioners_for_order_entry() {
    let t = TestEngine::start().await;
    let (admin, _) = staff(&t).await;
    t.sign_in_with(crate::fixture::ADMIN, crate::fixture::PASSWORD, &admin)
        .await;
    let client_id = t
        .engine
        .create_client(CreateClient {
            code: "SYN-CL".to_owned(),
            name: "Synthetic clinic".to_owned(),
        })
        .await
        .unwrap();
    t.engine
        .add_client_location(AddClientLocation {
            client_id,
            code: "SYN-LOC".to_owned(),
            name: "Synthetic location".to_owned(),
        })
        .await
        .unwrap();
    t.engine
        .register_practitioner(RegisterPractitioner {
            npi: "1234567893".to_owned(),
            family_name: "Synthetic".to_owned(),
            given_names: Some("Prescriber".to_owned()),
        })
        .await
        .unwrap();
    let duplicate = t
        .engine
        .create_client(CreateClient {
            code: "SYN-CL".to_owned(),
            name: "Another".to_owned(),
        })
        .await;
    assert_eq!(code_of(duplicate), ErrorCode::Conflict);
    let bad_npi = t
        .engine
        .register_practitioner(RegisterPractitioner {
            npi: "1234567890".to_owned(),
            family_name: "Synthetic".to_owned(),
            given_names: None,
        })
        .await
        .unwrap_err();
    assert_eq!(bad_npi.code(), ErrorCode::Validation);
    // Administration grants no clinical authority.
    assert_eq!(
        code_of(t.engine.search_subjects(search("")).await),
        ErrorCode::Forbidden
    );

    let clients = t.engine.list_clients().await.unwrap();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].locations[0].code, "SYN-LOC");
    let practitioners = t.engine.list_practitioners().await.unwrap();
    assert_eq!(practitioners[0].npi.as_str(), "1234567893");
}

#[tokio::test]
async fn an_administrator_sets_the_laboratory_profile_every_user_reads() {
    let t = TestEngine::start().await;
    let (admin, _) = staff(&t).await;
    // The accessioner is signed in: reading needs only a session.
    assert_eq!(t.engine.get_laboratory_profile().await.unwrap(), None);
    assert_eq!(
        code_of(
            t.engine
                .set_laboratory_profile(laboratory_profile(None))
                .await
        ),
        ErrorCode::Forbidden
    );
    t.sign_in_with(crate::fixture::ADMIN, crate::fixture::PASSWORD, &admin)
        .await;
    let first = t
        .engine
        .set_laboratory_profile(laboratory_profile(None))
        .await
        .unwrap();
    assert_eq!(
        (first.version, first.time_zone.as_str()),
        (1, "America/Chicago")
    );
    let mut change = laboratory_profile(Some(1));
    change.time_zone = "Mars/Olympus".to_owned();
    change.reason = Some("Synthetic move".to_owned());
    let invalid = t
        .engine
        .set_laboratory_profile(change.clone())
        .await
        .unwrap_err();
    assert_eq!(invalid.code(), ErrorCode::Validation);
    let json = serde_json::to_value(&invalid).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "timeZone");
    change.time_zone = "America/Denver".to_owned();
    let second = t.engine.set_laboratory_profile(change).await.unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(
        code_of(
            t.engine
                .set_laboratory_profile(laboratory_profile(None))
                .await
        ),
        ErrorCode::Conflict
    );
    // Reads name who set the profile; the stored record keeps only the user ID.
    let mut shown = t.engine.get_laboratory_profile().await.unwrap().unwrap();
    assert_eq!(
        shown.set_by_display.take().as_deref(),
        Some("Test Administrator")
    );
    assert_eq!(shown, second);
}

#[tokio::test]
async fn patient_directory_pages_and_simple_search_are_audited_without_criteria_values() {
    let t = TestEngine::start().await;
    staff(&t).await;
    let first = t
        .engine
        .register_subject(registration(vec![mrn("SYN-0001")]))
        .await
        .unwrap();
    let mut next = registration(vec![]);
    next.demographics = demographics("1991-01-01");
    let second = t.engine.register_subject(next).await.unwrap();
    let mut input = search("");
    input.limit = 1;
    let page = t.engine.search_subjects(input.clone()).await.unwrap();
    assert_eq!(page[0].subject_id, first.subject_id);
    input.offset = Some(1);
    let page = t.engine.search_subjects(input.clone()).await.unwrap();
    assert_eq!(page[0].subject_id, second.subject_id);
    input.offset = Some(0);
    input.query = Some(Phi::new("SYN-00".to_owned()));
    let found = t.engine.search_subjects(input).await.unwrap();
    assert_eq!(found[0].subject_id, first.subject_id);
    let details: Vec<String> = t.conn().prepare(
        "SELECT details FROM audit_events WHERE action = 'identity.search_subjects' ORDER BY seq"
    ).unwrap().query_map([], |row| row.get(0)).unwrap()
        .collect::<Result<_, _>>().unwrap();
    assert_eq!(details.len(), 3);
    assert!(details[0].contains("\"by_query\":false"));
    assert!(details[2].contains("\"by_query\":true"));
    assert!(
        details
            .iter()
            .all(|detail| detail.contains("\"returned\":1"))
    );
    assert!(!audit_text(&t).contains("SYN-00"));
    assert!(!audit_text(&t).contains(CANARY));
}
