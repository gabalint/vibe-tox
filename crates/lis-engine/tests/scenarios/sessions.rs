//! Session lifecycle: idle lock, lock and unlock, user switching, sign-out,
//! crash recovery and shutdown.

use lis_auth::{RoleCode, UnlockSessionInput};
use lis_core::ErrorCode;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of, secret};

fn end_reasons(t: &TestEngine) -> Vec<Option<String>> {
    t.conn()
        .prepare("SELECT end_reason FROM auth_sessions ORDER BY started_at")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[tokio::test]
async fn idle_timeout_locks_and_the_next_command_is_refused() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.advance(14 * 60);
    t.engine.touch_session().await.unwrap();
    t.advance(14 * 60);
    assert!(
        t.engine.list_users().await.is_ok(),
        "activity resets the idle timer"
    );
    t.advance(15 * 60);
    assert_eq!(
        code_of(t.engine.list_users().await),
        ErrorCode::SessionLocked
    );
    assert_eq!(
        code_of(t.engine.touch_session().await),
        ErrorCode::SessionLocked
    );
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
    let locks: Vec<String> = t
        .conn()
        .prepare("SELECT json_extract(details, '$.trigger') FROM audit_events WHERE action = 'auth.lock'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(locks, ["idle"], "the idle lock is audited once");
}

#[tokio::test]
async fn unlock_needs_the_same_users_password() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.engine.lock_session().await.unwrap();
    t.engine.lock_session().await.unwrap();
    assert_eq!(
        code_of(t.engine.list_users().await),
        ErrorCode::SessionLocked
    );
    let wrong = t
        .engine
        .unlock_session(UnlockSessionInput {
            password: secret("not the passphrase"),
        })
        .await;
    assert_eq!(code_of(wrong), ErrorCode::Unauthenticated);
    let session = t
        .engine
        .unlock_session(UnlockSessionInput {
            password: secret(PASSWORD),
        })
        .await
        .unwrap();
    assert!(!session.locked);
    assert!(t.engine.list_users().await.is_ok());
    let events = t.events("security");
    let count = |action: &str, outcome: &str| {
        events
            .iter()
            .filter(|(a, o)| a == action && o == outcome)
            .count()
    };
    assert_eq!(count("auth.lock", "succeeded"), 1);
    assert_eq!(count("auth.unlock", "failed"), 1);
    assert_eq!(count("auth.unlock", "succeeded"), 1);
}

#[tokio::test]
async fn sign_in_from_the_lock_screen_switches_users() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("analyst1", &[RoleCode::Analyst]).await;
    t.engine.lock_session().await.unwrap();
    let analyst = t.first_sign_in("analyst1").await.0;
    assert_eq!(analyst.username, "analyst1");
    assert_eq!(code_of(t.engine.list_users().await), ErrorCode::Forbidden);
    assert_eq!(end_reasons(&t), [Some("switched".to_owned()), None]);
    t.engine.sign_out().await.unwrap();
    assert_eq!(t.engine.current_session().await.unwrap(), None);
    assert_eq!(
        code_of(t.engine.touch_session().await),
        ErrorCode::Unauthenticated
    );
    assert_eq!(
        end_reasons(&t),
        [Some("switched".to_owned()), Some("signed_out".to_owned())]
    );
}

#[tokio::test]
async fn crash_leaves_no_open_session_and_shutdown_ends_it() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let t = t.crash_and_restart().await;
    assert_eq!(end_reasons(&t), [Some("abandoned".to_owned())]);
    assert_eq!(t.engine.current_session().await.unwrap(), None);
    // The enrolled authenticator survives the restart.
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    assert!(
        matches!(step, lis_auth::SignInStep::MfaRequired { .. }),
        "{step:?}"
    );
    t.engine.shutdown().await;
    t.engine.shutdown().await;
}

#[tokio::test]
async fn shutdown_ends_the_open_session_as_app_exit() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.engine.shutdown().await;
    assert_eq!(end_reasons(&t), [Some("app_exit".to_owned())]);
}

#[tokio::test]
async fn change_password_checks_the_current_one() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    let wrong = t
        .engine
        .change_password(lis_auth::ChangePasswordInput {
            current_password: secret("not the passphrase"),
            new_password: secret(NEW_PASSWORD),
        })
        .await
        .unwrap_err();
    assert_eq!(wrong.code(), ErrorCode::Validation);
    t.engine
        .change_password(lis_auth::ChangePasswordInput {
            current_password: secret(PASSWORD),
            new_password: secret(NEW_PASSWORD),
        })
        .await
        .unwrap();
    t.engine.sign_out().await.unwrap();
    t.sign_in_with(ADMIN, NEW_PASSWORD, &totp).await;
}
