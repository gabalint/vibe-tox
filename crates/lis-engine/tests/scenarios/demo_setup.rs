//! First-run demo consent, normal authentication, atomic loading and durable retry.

use lis_auth::{CreateFirstAdministratorInput, RoleCode};
use lis_core::ErrorCode;
use lis_engine::InstallationKind;

use crate::fixture::{ADMIN, PASSWORD, TestEngine, code_of, secret};

fn account(load_demo_data: Option<bool>, one_person: bool) -> CreateFirstAdministratorInput {
    CreateFirstAdministratorInput {
        username: ADMIN.into(),
        display_name: "Synthetic Demo Evaluator".into(),
        password: secret(PASSWORD),
        one_person,
        load_demo_data,
    }
}

async fn requested() -> TestEngine {
    let t = TestEngine::start_as(InstallationKind::Demo).await;
    t.engine
        .create_first_administrator(account(Some(true), true))
        .await
        .unwrap();
    t
}

fn count(t: &TestEngine, table: &str) -> i64 {
    t.conn()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[tokio::test]
async fn demo_setup_requires_normal_mfa_and_survives_restart() {
    let t = requested().await;
    let status = t.engine.setup_status().await.unwrap();
    assert!(!status.needs_administrator && status.demo_setup_pending);
    assert!(t.engine.current_session().await.unwrap().is_none());
    assert_eq!(
        code_of(t.engine.complete_demo_setup().await),
        ErrorCode::Unauthenticated
    );
    let t = t.crash_and_restart().await;
    assert!(t.engine.setup_status().await.unwrap().demo_setup_pending);
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    assert_eq!(
        code_of(t.engine.complete_demo_setup().await),
        ErrorCode::Unauthenticated
    );
    let (session, _) = t.enrol(step).await;
    let (first, second) = tokio::join!(
        t.engine.complete_demo_setup(),
        t.engine.complete_demo_setup()
    );
    first.unwrap();
    second.unwrap();
    assert!(!t.engine.setup_status().await.unwrap().demo_setup_pending);
    assert!(t.engine.demo_status().await.unwrap().demo_data_loaded);
    for (table, expected) in [
        ("cat_specimen_types", 2),
        ("cat_analytes", 111),
        ("cat_methods", 27),
        ("cat_orderables", 27),
        ("cat_standard_loads", 1),
        ("idn_client_organizations", 4),
        ("idn_practitioners", 8),
        ("idn_subjects", 12),
    ] {
        assert_eq!(count(&t, table), expected, "{table}");
    }
    let authored: i64 = t
        .conn()
        .query_row(
            "SELECT count(*) FROM cat_method_versions WHERE status = 'draft' AND created_by = ?1",
            [session.user_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        authored, 27,
        "all catalog methods remain drafts by the signed-in owner"
    );
    for action in ["auth.demo_setup_requested", "auth.demo_setup_completed"] {
        assert_eq!(
            t.events("security")
                .iter()
                .filter(|(name, outcome)| name == action && outcome == "succeeded")
                .count(),
            1
        );
    }
    let before = t.fingerprint();
    t.engine.complete_demo_setup().await.unwrap();
    assert_eq!(t.fingerprint(), before, "completed preparation is a no-op");
}

#[tokio::test]
async fn failed_demo_data_load_rolls_back_catalog_and_preserves_retry() {
    let t = requested().await;
    t.enrol(t.sign_in(ADMIN, PASSWORD).await.unwrap()).await;
    t.conn().execute_batch("CREATE TRIGGER synthetic_failure BEFORE INSERT ON idn_subjects BEGIN SELECT RAISE(ABORT, 'synthetic test failure'); END;").unwrap();
    let before = t.fingerprint();
    assert_eq!(
        code_of(t.engine.complete_demo_setup().await),
        ErrorCode::Internal
    );
    assert_eq!(
        t.fingerprint(),
        before,
        "catalog, parties and audit events roll back together"
    );
    assert!(t.engine.setup_status().await.unwrap().demo_setup_pending);
    t.conn()
        .execute_batch("DROP TRIGGER synthetic_failure;")
        .unwrap();
    t.engine.complete_demo_setup().await.unwrap();
    assert_eq!(count(&t, "cat_standard_loads"), 1);
    assert_eq!(count(&t, "idn_subjects"), 12);
    assert!(!t.engine.setup_status().await.unwrap().demo_setup_pending);
}

#[tokio::test]
async fn laboratory_installation_and_missing_one_person_choice_refuse_before_creation() {
    let t = TestEngine::start().await;
    let before = t.fingerprint();
    assert_eq!(
        code_of(
            t.engine
                .create_first_administrator(account(Some(true), true))
                .await
        ),
        ErrorCode::Conflict
    );
    assert_eq!(t.fingerprint(), before);
    let t = TestEngine::start_as(InstallationKind::Demo).await;
    let before = t.fingerprint();
    assert_eq!(
        code_of(
            t.engine
                .create_first_administrator(account(Some(true), false))
                .await
        ),
        ErrorCode::Validation
    );
    assert_eq!(t.fingerprint(), before);
    assert!(t.engine.setup_status().await.unwrap().needs_administrator);
}

#[tokio::test]
async fn omitted_or_declined_demo_choice_keeps_the_installation_empty() {
    for choice in [None, Some(false)] {
        let t = TestEngine::start_as(InstallationKind::Demo).await;
        t.engine
            .create_first_administrator(account(choice, true))
            .await
            .unwrap();
        t.enrol(t.sign_in(ADMIN, PASSWORD).await.unwrap()).await;
        assert!(!t.engine.setup_status().await.unwrap().demo_setup_pending);
        let before = t.fingerprint();
        t.engine.complete_demo_setup().await.unwrap();
        assert_eq!(t.fingerprint(), before);
        assert_eq!(count(&t, "idn_subjects"), 0);
    }
}

#[tokio::test]
async fn another_account_cannot_consume_the_first_accounts_demo_request() {
    let t = requested().await;
    t.enrol(t.sign_in(ADMIN, PASSWORD).await.unwrap()).await;
    t.create_user("other1", &[RoleCode::Administrator, RoleCode::Accessioner])
        .await;
    t.first_sign_in("other1").await;
    let before = t.fingerprint();
    assert_eq!(
        code_of(t.engine.complete_demo_setup().await),
        ErrorCode::Conflict
    );
    assert_eq!(t.fingerprint(), before);
    assert!(t.engine.setup_status().await.unwrap().demo_setup_pending);
}
