//! Find in the audit trail (REQ-OPS-001): records by readable values or record ID, patients
//! and patient names only for users who may read patients, and each search recorded as an
//! access event without its text. Synthetic data only.

use lis_auth::RoleCode;
use lis_core::Phi;
use lis_engine::{
    AuditStream, FindAuditRecordsInput, FoundAuditRecord, ListAuditEventsInput, RegisterSubject,
};
use lis_identity::DemographicsInput;

use crate::fixture::TestEngine;

/// A synthetic name that must never reach an audit event.
const CANARY: &str = "Syntheticfindcanary";

fn find(text: &str) -> FindAuditRecordsInput {
    FindAuditRecordsInput {
        text: Phi::new(text.to_owned()),
        record_type: None,
        stream: None,
        day: None,
    }
}

async fn found(t: &TestEngine, input: FindAuditRecordsInput) -> Vec<FoundAuditRecord> {
    t.engine.find_audit_records(input).await.unwrap().records
}

/// The `details` of every audit event.
fn audit_details(t: &TestEngine) -> String {
    t.conn()
        .prepare("SELECT coalesce(details, '') FROM audit_events")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n")
}

/// The record label of the trail's subject rows, as the signed-in user reads them.
async fn subject_labels(t: &TestEngine) -> Vec<Option<String>> {
    t.engine
        .list_audit_events(ListAuditEventsInput {
            limit: 500,
            record_type: Some("subject".to_owned()),
            ..ListAuditEventsInput::default()
        })
        .await
        .unwrap()
        .into_iter()
        .map(|event| event.record_label)
        .collect()
}

#[tokio::test]
async fn find_offers_records_by_readable_values_and_patients_only_to_who_may_read_them() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("auditor1", &[RoleCode::Auditor]).await;
    t.create_user("acc1", &[RoleCode::Accessioner, RoleCode::Auditor])
        .await;
    t.first_sign_in("acc1").await;
    let subject_id = t
        .engine
        .register_subject(RegisterSubject {
            demographics: DemographicsInput {
                family_name: Phi::new(CANARY.to_owned()),
                given_names: Some(Phi::new("Alpha".to_owned())),
                birth_date: Phi::new("1990-01-01".to_owned()),
                administrative_sex: None,
            },
            source: "requisition".to_owned(),
            identifiers: vec![],
            acknowledged_candidates: vec![],
        })
        .await
        .unwrap()
        .subject_id
        .to_string();
    let name = format!("{CANARY}, Alpha");

    // A user who may read patients finds them by name and sees their names.
    let patients = found(&t, find("syntheticfind")).await;
    assert_eq!(patients.len(), 1);
    assert_eq!(patients[0].record_type, "subject");
    assert_eq!(patients[0].record_id, subject_id);
    assert_eq!(patients[0].label.as_deref(), Some(name.as_str()));
    assert!(subject_labels(&t).await.contains(&Some(name)));

    // An auditor finds users by username, with the streams they appear in...
    t.first_sign_in("auditor1").await;
    let users = found(&t, find("auditor")).await;
    let auditor = users
        .iter()
        .find(|record| record.record_type == "user")
        .unwrap();
    assert_eq!(auditor.label.as_deref(), Some("Test auditor1"));
    assert!(auditor.streams.contains(&AuditStream::Security));
    // ...but not patients by name, and sees no patient names in the trail.
    assert!(found(&t, find("syntheticfind")).await.is_empty());
    assert!(subject_labels(&t).await.iter().all(Option::is_none));
    // The record ID still finds the patient's events, without the name.
    let by_id = found(&t, find(&subject_id)).await;
    assert_eq!(by_id.len(), 1);
    assert_eq!(by_id[0].label, None);
    // One letter is not a search.
    let short = t.engine.find_audit_records(find("s")).await.unwrap();
    assert!(short.records.is_empty() && !short.more);

    // Every search is an access event; none records the text.
    let searches = t
        .events("access")
        .into_iter()
        .filter(|(action, _)| action == "audit.find_records")
        .count();
    assert_eq!(searches, 5);
    let details = audit_details(&t);
    assert!(!details.contains(CANARY) && !details.to_lowercase().contains("syntheticfind"));
}
