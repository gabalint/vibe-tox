//! A laboratory that one person runs (ADR 0015; Q-08, Q-37; REQ-OPS-015): first-run
//! setup for one person gives every role; that person loads the Standard Catalog and
//! approves it alone, but only after confirming the self-approval, which the audit
//! viewer then lists; the policy can require a second person again; and a laboratory
//! set up for several people can become a one-person laboratory only while one account
//! is active. Synthetic data only.

use lis_auth::{
    CreateFirstAdministratorInput, DEFAULT_SEPARATION_POLICY, DesignateOnePersonLaboratoryInput,
    RoleCode, SeparationPolicy, SetSeparationPolicyInput,
};
use lis_catalog::StandardItemState;
use lis_core::{CommandError, ErrorCode};
use lis_engine::{AuditStream, Engine, ListAuditEventsInput};

use crate::fixture::{ADMIN, PASSWORD, TestEngine, code_of, secret};
use crate::starter_catalog::{approval, confirmations, preview, selection};

const ALL_ALLOWED: SeparationPolicy = SeparationPolicy {
    self_approve_configuration: true,
    self_release_reports: true,
    self_verify_destinations: true,
};

/// The field errors of a refusal, as the UI receives them.
fn fields(error: &CommandError) -> String {
    serde_json::to_value(error).unwrap()["fieldErrors"].to_string()
}

/// Actions of the clinical events marked as self-approvals, newest first.
async fn self_approvals(engine: &Engine) -> Vec<String> {
    let input = ListAuditEventsInput {
        stream: AuditStream::Clinical,
        before_seq: None,
        limit: 500,
        action: None,
        record_type: None,
        record_id: None,
        self_approvals_only: Some(true),
    };
    let events = engine.list_audit_events(input).await.unwrap();
    events.into_iter().map(|event| event.action).collect()
}

/// Loads the first test or panel of the shipped file not loaded yet, with what it
/// needs, as drafts.
async fn load_one(engine: &Engine) {
    let tree = engine.standard_catalog_tree().await.unwrap();
    let next = tree
        .orderables
        .iter()
        .find(|o| o.status.state != StandardItemState::AlreadyPresent)
        .unwrap();
    let input = selection(
        &tree.info.catalog_version,
        std::slice::from_ref(&next.code),
        &[],
    );
    engine.load_standard_catalog(input).await.unwrap();
}

#[tokio::test]
async fn one_person_approves_their_own_starter_catalog_only_after_confirming() {
    let t = TestEngine::start().await;
    t.engine
        .create_first_administrator(CreateFirstAdministratorInput {
            username: ADMIN.into(),
            display_name: "Test Director".into(),
            password: secret(PASSWORD),
            one_person: true,
            load_demo_data: None,
        })
        .await
        .unwrap();
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    let (session, _) = t.enrol(step).await;
    assert_eq!(session.roles, RoleCode::ALL);
    for capability in ["approve_configuration", "release_reports", "manage_users"] {
        assert!(
            session.capabilities.iter().any(|c| c == capability),
            "{capability}"
        );
    }

    load_one(&t.engine).await;
    let preview_id = t.engine.generate_catalog_preview().await.unwrap();
    let listed = preview(&t.engine, preview_id).await;
    assert!(listed.drafts > 0);
    let unconfirmed = approval(preview_id, confirmations(&listed), PASSWORD);
    let refused = t
        .engine
        .approve_reviewed_catalog(unconfirmed)
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    assert!(
        fields(&refused).contains("selfApproval"),
        "{}",
        fields(&refused)
    );
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!(status.drafts_remaining, listed.drafts);
    assert!(self_approvals(&t.engine).await.is_empty());

    let mut confirmed = approval(preview_id, confirmations(&listed), PASSWORD);
    confirmed.self_approval = true;
    t.engine.approve_reviewed_catalog(confirmed).await.unwrap();
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!(status.drafts_remaining, 0);
    let reviewed_by = preview(&t.engine, preview_id).await.reviewed_by;
    assert_eq!(reviewed_by, Some(session.user_id));
    let marked = self_approvals(&t.engine).await;
    assert_eq!(marked[0], "catalog.approve_reviewed_catalog");
    assert!(
        marked.iter().any(|a| a == "catalog.approve_method_version"),
        "{marked:?}"
    );

    // The laboratory hires a second person: self-approval needs a second person again.
    let view = t.engine.get_separation_policy().await.unwrap();
    assert_eq!((view.version, view.values), (Some(1), ALL_ALLOWED));
    t.engine
        .set_separation_policy(SetSeparationPolicyInput {
            expected_version: Some(1),
            values: DEFAULT_SEPARATION_POLICY,
            reason: "A second reviewer joined".to_owned(),
            password: secret(PASSWORD).into(),
        })
        .await
        .unwrap();
    t.advance(1);
    load_one(&t.engine).await;
    let preview_id = t.engine.generate_catalog_preview().await.unwrap();
    let listed = preview(&t.engine, preview_id).await;
    assert!(listed.drafts > 0);
    let mut confirmed = approval(preview_id, confirmations(&listed), PASSWORD);
    confirmed.self_approval = true;
    let refused = t.engine.approve_reviewed_catalog(confirmed).await;
    assert_eq!(code_of(refused), ErrorCode::GateBlocked);
}

#[tokio::test]
async fn only_the_one_active_account_becomes_a_one_person_laboratory() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let designate = |password: &str| DesignateOnePersonLaboratoryInput {
        reason: "The director runs the laboratory alone".to_owned(),
        password: secret(password).into(),
    };
    let wrong = t
        .engine
        .designate_one_person_laboratory(designate("not the passphrase"))
        .await
        .unwrap_err();
    assert!(fields(&wrong).contains("password"), "{}", fields(&wrong));
    let before = t.engine.current_session().await.unwrap().unwrap();
    assert_eq!(before.roles, [RoleCode::Administrator]);

    let user = t
        .engine
        .designate_one_person_laboratory(designate(PASSWORD))
        .await
        .unwrap();
    assert_eq!(user.roles, RoleCode::ALL);
    let after = t.engine.current_session().await.unwrap().unwrap();
    assert!(
        after
            .capabilities
            .iter()
            .any(|c| c == "approve_configuration")
    );
    let view = t.engine.get_separation_policy().await.unwrap();
    assert_eq!(view.values, ALL_ALLOWED);

    // A second active account ends the one-person case.
    t.create_user("clerk1", &[]).await;
    let refused = t
        .engine
        .designate_one_person_laboratory(designate(PASSWORD))
        .await;
    assert_eq!(code_of(refused), ErrorCode::Conflict);
}
