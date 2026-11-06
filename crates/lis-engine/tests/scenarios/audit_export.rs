//! Exporting the audit trail (REQ-OPS-001, ticket #4535): every event the
//! filters match, as CSV or PDF, with patient values only where the exporter may
//! read the patient, and the export itself recorded with its filters.
//! Synthetic data only.

use lis_auth::RoleCode;
use lis_core::ErrorCode;
use lis_engine::{AuditExportFormat, AuditStream, ExportAuditEventsInput, ListAuditEventsInput};

use crate::audit_changes::{correction, registration};
use crate::fixture::{NEW_PASSWORD, TestEngine, code_of};

fn clinical(format: AuditExportFormat) -> ExportAuditEventsInput {
    ExportAuditEventsInput {
        format,
        filter: ListAuditEventsInput {
            stream: Some(AuditStream::Clinical),
            limit: 100,
            ..ListAuditEventsInput::default()
        },
    }
}

/// The latest `audit.export_events` event: outcome, record ID and details.
fn last_export(t: &TestEngine) -> (String, Option<String>, String) {
    t.conn()
        .query_row(
            "SELECT outcome, record_id, details FROM audit_events \
             WHERE action = 'audit.export_events' ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

/// A quality manager who also audits and exports corrects a patient's family name.
async fn corrected_patient(t: &TestEngine) {
    t.admin_session().await;
    let roles = [
        RoleCode::QualityManager,
        RoleCode::Auditor,
        RoleCode::Administrator,
    ];
    t.create_user("qa1", &roles).await;
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
}

#[tokio::test]
async fn csv_export_holds_every_matching_event_with_its_changes_and_is_recorded() {
    let t = TestEngine::start().await;
    corrected_patient(&t).await;
    let downloads = tempfile::tempdir().unwrap();
    let exported = t
        .engine
        .export_audit_events(
            clinical(AuditExportFormat::Csv),
            Some(downloads.path().to_owned()),
        )
        .await
        .unwrap();
    assert!(
        exported.file_name.starts_with("audit-trail-"),
        "{}",
        exported.file_name
    );
    assert!(exported.file_name.ends_with(".csv"));
    let clinical_events = t.events("clinical").len();
    assert_eq!(exported.events, clinical_events);
    let text = std::fs::read_to_string(downloads.path().join(&exported.file_name)).unwrap();
    let mut lines = text.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("stream,seq,occurred,occurred_utc,")
    );
    assert_eq!(lines.count(), clinical_events);
    assert!(text.contains("\"Family name: Dae → Doe\""), "{text}");
    assert!(text.contains("\"Doe, Test, born 1980-01-01; MR (SYN-CLINIC): MRN-00000\""));
    assert!(text.contains("\"Typo in the family name\""));

    let (outcome, record, details) = last_export(&t);
    assert_eq!(outcome, "succeeded");
    assert_eq!(record, Some(exported.file_name));
    for expected in [
        "\"format\":\"csv\"",
        &format!("\"rows\":{clinical_events}"),
        "\"patient_records\":1",
        "\"audit_stream\":\"clinical\"",
        "\"recorded_before\":",
    ] {
        assert!(details.contains(expected), "{expected} in {details}");
    }
    // The export names no value: patients are counted, never copied into the trail.
    assert!(!details.contains("Doe") && !details.contains("MRN-00000"));
}

#[tokio::test]
async fn pdf_export_is_rendered_and_recorded() {
    let t = TestEngine::start().await;
    corrected_patient(&t).await;
    let downloads = tempfile::tempdir().unwrap();
    let exported = t
        .engine
        .export_audit_events(
            clinical(AuditExportFormat::Pdf),
            Some(downloads.path().to_owned()),
        )
        .await
        .unwrap();
    assert!(exported.file_name.ends_with(".pdf"));
    let bytes = std::fs::read(downloads.path().join(&exported.file_name)).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let (_, _, details) = last_export(&t);
    assert!(details.contains("\"format\":\"pdf\""), "{details}");
}

#[tokio::test]
async fn export_needs_view_audit_and_export() {
    let t = TestEngine::start().await;
    corrected_patient(&t).await;
    t.first_sign_in("aud1").await;
    let downloads = tempfile::tempdir().unwrap();
    let denied = t
        .engine
        .export_audit_events(
            clinical(AuditExportFormat::Csv),
            Some(downloads.path().to_owned()),
        )
        .await;
    assert_eq!(code_of(denied), ErrorCode::Forbidden);
    let (outcome, _, details) = last_export(&t);
    assert_eq!(outcome, "denied");
    assert!(details.contains("\"capability\":\"export\""), "{details}");
    assert_eq!(std::fs::read_dir(downloads.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn an_export_without_a_downloads_folder_is_refused() {
    let t = TestEngine::start().await;
    corrected_patient(&t).await;
    let refused = t
        .engine
        .export_audit_events(clinical(AuditExportFormat::Csv), None)
        .await;
    assert_eq!(code_of(refused), ErrorCode::Unavailable);
}
