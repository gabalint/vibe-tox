//! The audit viewer: paging, filters, access events and chain verification.

use lis_auth::RoleCode;
use lis_core::{Clock, ErrorCode};
use lis_engine::{
    AuditActorKind, AuditCursor, AuditEventView, AuditOutcome, AuditStream, ListAuditEventsInput,
    VerifyAuditChainInput,
};

use crate::fixture::{TestEngine, code_of};

fn page(stream: AuditStream, limit: u32) -> ListAuditEventsInput {
    ListAuditEventsInput {
        stream: Some(stream),
        limit,
        ..ListAuditEventsInput::default()
    }
}

/// Where the page after `event` starts.
fn after(event: &AuditEventView) -> AuditCursor {
    AuditCursor {
        recorded_at: event.recorded_at,
        stream: event.stream,
        seq: event.seq,
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
        .list_audit_events(page(AuditStream::Security, 3))
        .await
        .unwrap();
    let seqs: Vec<i64> = first.iter().map(|event| event.seq).collect();
    let newest = i64::try_from(total).unwrap();
    assert_eq!(seqs, [newest, newest - 1, newest - 2]);
    assert_eq!(first[0].action, "auth.sign_in");
    assert_eq!(first[0].principal_kind, "system");
    let next = t
        .engine
        .list_audit_events(ListAuditEventsInput {
            before: Some(after(&first[2])),
            ..page(AuditStream::Security, 500)
        })
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
    let mut filtered = page(AuditStream::Security, 50);
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
async fn every_stream_reads_newest_first_with_a_stable_cursor() {
    let t = TestEngine::start().await;
    auditor_session(&t).await;
    let stored = t.events("security").len() + t.events("clinical").len();
    let every = |limit| ListAuditEventsInput {
        limit,
        ..ListAuditEventsInput::default()
    };
    // Each read below is a second later, so its access event is the newest.
    t.advance(1);
    let all = t.engine.list_audit_events(every(500)).await.unwrap();
    assert_eq!(all.len(), stored);
    let code = |stream| match stream {
        AuditStream::Clinical => "clinical",
        AuditStream::Security => "security",
        AuditStream::Access => "access",
    };
    let order: Vec<_> = all
        .iter()
        .map(|event| (event.recorded_at, code(event.stream), event.seq))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] > pair[1]), "{order:?}");
    assert!(
        all.iter()
            .any(|event| event.stream == AuditStream::Security)
    );

    t.advance(1);
    let first = t.engine.list_audit_events(every(2)).await.unwrap();
    assert_eq!(first[0].stream, AuditStream::Access);
    assert_eq!(first[0].action, "audit.list_events");
    assert_eq!(first[0].record_id.as_deref(), Some("all"));
    // The access event of the first page, appended later, does not shift the next page.
    t.advance(1);
    let rest = t
        .engine
        .list_audit_events(ListAuditEventsInput {
            before: Some(after(&first[1])),
            ..every(500)
        })
        .await
        .unwrap();
    assert_eq!(first.len() + rest.len(), stored + 1);
    assert!(rest.iter().all(|event| !first.contains(event)));
    let details: String = t
        .conn()
        .query_row(
            "SELECT details FROM audit_events WHERE stream = 'access' AND seq = 3",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(details.contains("\"before_stream\""), "{details}");
}

#[tokio::test]
async fn filters_by_time_principal_and_outcome_in_the_query() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let auditor = t.create_user("auditor1", &[RoleCode::Auditor]).await;
    t.first_sign_in("auditor1").await;
    t.advance(60);
    // An auditor may not list users: one denied security event, a minute later.
    assert_eq!(code_of(t.engine.list_users().await), ErrorCode::Forbidden);
    let read = |change: fn(&mut ListAuditEventsInput)| {
        let mut input = page(AuditStream::Security, 500);
        change(&mut input);
        t.engine.list_audit_events(input)
    };
    let denied = read(|input| input.outcome = Some(AuditOutcome::Denied))
        .await
        .unwrap();
    assert_eq!(denied.len(), 1);
    assert_eq!(denied[0].outcome, "denied");
    let system = read(|input| input.by_kind = Some(AuditActorKind::System))
        .await
        .unwrap();
    assert!(!system.is_empty());
    assert!(system.iter().all(|event| event.principal_kind != "user"));
    let people = read(|input| input.by_kind = Some(AuditActorKind::People))
        .await
        .unwrap();
    assert!(people.iter().all(|event| event.principal_kind == "user"));
    let every = read(|_| ()).await.unwrap();
    assert_eq!(people.len() + system.len(), every.len());
    let mut by_auditor = page(AuditStream::Security, 500);
    by_auditor.by_user = Some(auditor.user_id);
    let by_auditor = t.engine.list_audit_events(by_auditor).await.unwrap();
    assert!(by_auditor.iter().any(|event| event.outcome == "denied"));
    assert!(
        by_auditor
            .iter()
            .all(|event| event.principal_id == auditor.user_id.to_string())
    );
    assert!(
        by_auditor[0]
            .available_users
            .as_ref()
            .unwrap()
            .contains(&(auditor.user_id.to_string(), "Test auditor1".to_owned()))
    );
    // Recorded from the denial's instant on: the denial, not the sign-ins a minute before.
    let since = t.clock.now();
    let mut recent = page(AuditStream::Security, 500);
    recent.recorded_from = Some(since);
    let recent = t.engine.list_audit_events(recent).await.unwrap();
    assert!(recent.iter().any(|event| event.outcome == "denied"));
    assert!(recent.iter().all(|event| event.recorded_at >= since));
    assert!(recent.len() < every.len());
    let mut earlier = page(AuditStream::Security, 500);
    earlier.recorded_before = Some(since);
    let earlier = t.engine.list_audit_events(earlier).await.unwrap();
    assert!(earlier.iter().all(|event| event.recorded_at < since));
    let mut empty = page(AuditStream::Security, 500);
    empty.recorded_from = Some(since);
    empty.recorded_before = Some(since);
    let error = t.engine.list_audit_events(empty).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
}

#[tokio::test]
async fn page_size_is_validated() {
    let t = TestEngine::start().await;
    auditor_session(&t).await;
    for limit in [0, 501] {
        let error = t
            .engine
            .list_audit_events(page(AuditStream::Access, limit))
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
                .list_audit_events(page(AuditStream::Security, 10))
                .await
        ),
        ErrorCode::Forbidden
    );
    assert!(t.events("access").is_empty());
}
