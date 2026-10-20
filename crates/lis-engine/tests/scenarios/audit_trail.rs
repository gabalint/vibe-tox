//! The audit viewer: paging, filters, access events and chain verification.

use lis_auth::RoleCode;
use lis_core::ErrorCode;
use lis_engine::{AuditStream, ListAuditEventsInput, VerifyAuditChainInput};

use crate::fixture::{TestEngine, code_of};

fn page(stream: AuditStream, before_seq: Option<i64>, limit: u32) -> ListAuditEventsInput {
    ListAuditEventsInput {
        stream,
        before_seq,
        limit,
        action: None,
        record_type: None,
        record_id: None,
        self_approvals_only: None,
    }
}

async fn auditor_session(t: &TestEngine) {
    t.admin_session().await;
    t.create_user("auditor1", &[RoleCode::Auditor]).await;
    t.first_sign_in("auditor1").await;
}

#[tokio::test]
async fn auditor_pages_newest_first_and_each_read_is_recorded() {
    let t = TestEngine::start().await;
    auditor_session(&t).await;
    let total = t.events("security").len();
    let first = t
        .engine
        .list_audit_events(page(AuditStream::Security, None, 3))
        .await
        .unwrap();
    let seqs: Vec<i64> = first.iter().map(|event| event.seq).collect();
    let newest = i64::try_from(total).unwrap();
    assert_eq!(seqs, [newest, newest - 1, newest - 2]);
    assert_eq!(first[0].action, "auth.sign_in");
    assert_eq!(first[0].principal_kind, "system");
    let next = t
        .engine
        .list_audit_events(page(AuditStream::Security, Some(newest - 2), 500))
        .await
        .unwrap();
    assert_eq!(next.len(), total - 3);
    let created = next
        .iter()
        .find(|event| event.action == "auth.user_create")
        .unwrap();
    assert_eq!(
        created.principal_display.as_deref(),
        Some("Test Administrator")
    );
    assert_eq!(created.action_label.as_deref(), Some("User created"));
    assert_eq!(created.record_type_label.as_deref(), Some("User"));
    assert_eq!(created.record_label.as_deref(), Some("Test auditor1"));
    assert!(
        created
            .available_actions
            .as_ref()
            .unwrap()
            .contains(&("auth.sign_in".to_owned(), "Sign-in".to_owned()))
    );
    assert!(
        created
            .available_record_types
            .as_ref()
            .unwrap()
            .contains(&("user".to_owned(), "User".to_owned()))
    );
    assert_eq!(created.authority.as_deref(), Some("manage_users"));
    let mut filtered = page(AuditStream::Security, None, 50);
    filtered.action = Some("auth.mfa_enroll".into());
    assert_eq!(t.engine.list_audit_events(filtered).await.unwrap().len(), 2);

    let access = t.events("access");
    assert_eq!(access.len(), 3, "one access event per read");
    assert!(
        access
            .iter()
            .all(|(action, outcome)| action == "audit.list_events" && outcome == "succeeded")
    );
    let authorities: Vec<String> = t
        .conn()
        .prepare("SELECT authority FROM audit_events WHERE stream = 'access' ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(authorities, vec!["view_audit"; 3]);
}

#[tokio::test]
async fn page_size_is_validated() {
    let t = TestEngine::start().await;
    auditor_session(&t).await;
    for limit in [0, 501] {
        let error = t
            .engine
            .list_audit_events(page(AuditStream::Access, None, limit))
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Validation);
    }
    assert!(t.events("access").is_empty());
}

#[tokio::test]
async fn chain_verification_reports_intact_and_tampered_streams() {
    let t = TestEngine::start().await;
    auditor_session(&t).await;
    let report = t
        .engine
        .verify_audit_chain(VerifyAuditChainInput {
            stream: AuditStream::Security,
        })
        .await
        .unwrap();
    assert!(report.intact);
    assert_eq!(
        report.events_checked,
        u64::try_from(t.events("security").len()).unwrap()
    );

    // An OS administrator edits a row directly.
    let conn = t.conn();
    conn.execute_batch(
        "DROP TRIGGER audit_events_no_update; \
         UPDATE audit_events SET reason = 'edited' WHERE stream = 'security' AND seq = 2;",
    )
    .unwrap();
    let report = t
        .engine
        .verify_audit_chain(VerifyAuditChainInput {
            stream: AuditStream::Security,
        })
        .await
        .unwrap();
    assert!(!report.intact);
    assert_eq!(report.findings[0].kind, "hash_mismatch");
    assert_eq!(report.findings[0].seq, Some(2));
}

#[tokio::test]
async fn administrator_cannot_read_the_audit_trail() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    assert_eq!(
        code_of(
            t.engine
                .list_audit_events(page(AuditStream::Security, None, 10))
                .await
        ),
        ErrorCode::Forbidden
    );
    assert!(t.events("access").is_empty());
}
