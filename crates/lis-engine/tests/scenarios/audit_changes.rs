//! The Changes of an audit event (REQ-OPS-001, ticket #4534): patient values for
//! people who may read the patient, field names only for the others, and an
//! access event for every view. Synthetic data only.

use lis_auth::RoleCode;
use lis_core::{ErrorCode, Phi};
use lis_engine::{
    AssertDemographics, AuditChangeStatus, AuditStream, GetAuditEventChangesInput, RegisterSubject,
};
use lis_identity::{DemographicsInput, IdentifierInput, SubjectId};

use crate::fixture::{NEW_PASSWORD, TestEngine, code_of};

fn demographics(family_name: &str) -> DemographicsInput {
    DemographicsInput {
        family_name: Phi::new(family_name.to_owned()),
        given_names: Some(Phi::new("Test".to_owned())),
        birth_date: Phi::new("1980-01-01".to_owned()),
        administrative_sex: None,
    }
}

/// A synthetic patient registered as "Dae, Test" with one MRN.
pub(crate) fn registration() -> RegisterSubject {
    RegisterSubject {
        demographics: demographics("Dae"),
        source: "requisition".to_owned(),
        identifiers: vec![IdentifierInput {
            assigning_authority: "SYN-CLINIC".to_owned(),
            identifier_type: "MR".to_owned(),
            value: Phi::new("MRN-00000".to_owned()),
        }],
        acknowledged_candidates: vec![],
    }
}

/// Corrects the family name of the patient from [`registration`] to "Doe".
pub(crate) fn correction(subject_id: SubjectId) -> AssertDemographics {
    AssertDemographics {
        subject_id,
        expected_revision: 1,
        demographics: demographics("Doe"),
        source: "requisition".to_owned(),
        reason: "Typo in the family name".to_owned(),
        acknowledged_candidates: vec![],
    }
}

/// The sequence number of the latest `action` event in `stream`.
fn seq_of(t: &TestEngine, stream: &str, action: &str) -> i64 {
    t.conn()
        .query_row(
            "SELECT max(seq) FROM audit_events WHERE stream = ?1 AND action = ?2",
            [stream, action],
            |row| row.get(0),
        )
        .unwrap()
}

/// The record, authority and details of the latest `audit.view_changes` event.
fn last_view(t: &TestEngine) -> (Option<String>, Option<String>, String) {
    t.conn()
        .query_row(
            "SELECT record_id, authority, details FROM audit_events \
             WHERE action = 'audit.view_changes' ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

fn input(stream: AuditStream, seq: i64) -> GetAuditEventChangesInput {
    GetAuditEventChangesInput { stream, seq }
}

#[tokio::test]
async fn patient_changes_show_values_only_to_people_who_may_read_the_patient() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("qa1", &[RoleCode::QualityManager, RoleCode::Auditor])
        .await;
    t.create_user("aud1", &[RoleCode::Auditor]).await;
    t.create_user("acc1", &[RoleCode::Accessioner]).await;
    let (_, quality) = t.first_sign_in("qa1").await;
    t.first_sign_in("acc1").await;
    let registered = t.engine.register_subject(registration()).await.unwrap();
    t.sign_in_with("qa1", NEW_PASSWORD, &quality).await;
    t.engine
        .assert_demographics(correction(registered.subject_id))
        .await
        .unwrap();
    let corrected = input(
        AuditStream::Clinical,
        seq_of(&t, "clinical", "identity.assert_demographics"),
    );

    let changes = t.engine.get_audit_event_changes(corrected).await.unwrap();
    assert_eq!(changes.status, AuditChangeStatus::Changed);
    assert_eq!(
        (changes.old_revision, changes.new_revision),
        (Some(1), Some(2))
    );
    assert_eq!(changes.fields.len(), 1);
    assert_eq!(changes.fields[0].field, "Family name");
    assert_eq!(changes.fields[0].old.as_deref(), Some("Dae"));
    assert_eq!(changes.fields[0].new.as_deref(), Some("Doe"));
    let patient = changes.patient.unwrap();
    assert_eq!(patient.name, "Doe, Test");
    assert_eq!(patient.identifiers, ["MR (SYN-CLINIC): MRN-00000"]);
    let (record, authority, details) = last_view(&t);
    assert_eq!(record, Some(registered.subject_id.to_string()));
    assert!(authority.unwrap().contains("manage_quality"));
    assert!(details.contains("\"values_shown\":true"), "{details}");
    // The view names the patient by ID; no value reaches the audit trail.
    let text: String = t
        .conn()
        .query_row(
            "SELECT group_concat(coalesce(details, '') || coalesce(reason, ''), '|') \
             FROM audit_events",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!text.contains("Dae") && !text.contains("MRN-00000"));

    t.first_sign_in("aud1").await;
    let hidden = t.engine.get_audit_event_changes(corrected).await.unwrap();
    assert_eq!(hidden.status, AuditChangeStatus::Hidden);
    assert_eq!(hidden.fields[0].field, "Family name");
    assert_eq!(
        (hidden.fields[0].old.as_ref(), hidden.fields[0].new.as_ref()),
        (None, None)
    );
    assert!(hidden.patient.is_none());
    assert_eq!(hidden.read_capabilities, ["enter_orders", "manage_quality"]);
    let (record, _, details) = last_view(&t);
    assert_eq!(record.as_deref(), Some("clinical"));
    assert!(details.contains("\"values_shown\":false"), "{details}");
}

#[tokio::test]
async fn events_without_record_data_and_unknown_events() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("aud1", &[RoleCode::Auditor]).await;
    t.first_sign_in("aud1").await;
    let sign_in = input(
        AuditStream::Security,
        seq_of(&t, "security", "auth.sign_in"),
    );
    let changes = t.engine.get_audit_event_changes(sign_in).await.unwrap();
    assert_eq!(changes.status, AuditChangeStatus::NoChange);
    assert!(changes.fields.is_empty());
    let missing = t
        .engine
        .get_audit_event_changes(input(AuditStream::Clinical, 1_000_000))
        .await;
    assert_eq!(code_of(missing), ErrorCode::NotFound);
}

#[tokio::test]
async fn reading_changes_needs_view_audit() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let sign_in = input(AuditStream::Security, 1);
    let denied = t.engine.get_audit_event_changes(sign_in).await;
    assert_eq!(code_of(denied), ErrorCode::Forbidden);
    let denials = t
        .events("security")
        .into_iter()
        .filter(|event| event.0 == "audit.view_changes" && event.1 == "denied")
        .count();
    assert_eq!(denials, 1);
}
