//! Sign-in: bootstrap, enrolment, identical refusals, lockout, replay, temporary passwords.

use lis_auth::{ChangeInitialPasswordInput, RoleCode, SignInStep};
use lis_core::{CommandError, ErrorCode};

use crate::fixture::{ADMIN, PASSWORD, TestEngine, WEBVIEW, challenge_id, code_of, secret};

/// The error as the UI sees it, without the per-command correlation ID.
fn answer(error: CommandError) -> serde_json::Value {
    let mut json = serde_json::to_value(error).unwrap();
    json.as_object_mut().unwrap().remove("correlationId");
    json
}

#[tokio::test]
async fn bootstrap_then_first_sign_in_forces_mfa_enrolment() {
    let t = TestEngine::start().await;
    assert!(t.engine.setup_status().await.unwrap().needs_administrator);
    t.bootstrap().await;
    assert!(!t.engine.setup_status().await.unwrap().needs_administrator);
    assert_eq!(
        code_of(t.engine.list_users().await),
        ErrorCode::Unauthenticated
    );

    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
        panic!("first sign-in must enrol MFA: {step:?}");
    };
    assert!(enrollment.otpauth_uri.starts_with("otpauth://totp/"));
    assert!(enrollment.qr_svg.contains("<svg"));
    let (session, _) = t.enrol(step).await;
    assert_eq!(session.username, ADMIN);
    assert_eq!(session.roles, [RoleCode::Administrator]);
    assert_eq!(
        session.capabilities,
        [
            "export",
            "backup_restore",
            "manage_users",
            "manage_configuration",
            "approve_configuration"
        ]
    );
    assert_eq!(session.idle_timeout_seconds, 900);
    assert!(!session.locked);
    assert_eq!(t.engine.current_session().await.unwrap(), Some(session));
    assert_eq!(t.engine.list_users().await.unwrap().len(), 1);

    let actions: Vec<String> = t
        .events("security")
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    assert_eq!(
        actions,
        [
            "ops.installation_create",
            "auth.workstation_register",
            "ops.app_start",
            "auth.first_administrator_create",
            "auth.mfa_enroll",
            "auth.sign_in"
        ]
    );
    let webview: String = t
        .conn()
        .query_row("SELECT webview FROM auth_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(webview, WEBVIEW);
}

#[tokio::test]
async fn second_administrator_bootstrap_is_refused() {
    let t = TestEngine::start().await;
    t.bootstrap().await;
    let again = t
        .engine
        .create_first_administrator(lis_auth::CreateFirstAdministratorInput {
            username: "admin2".into(),
            display_name: "Second".into(),
            password: secret(PASSWORD),
            one_person: false,
            load_demo_data: None,
        })
        .await;
    assert_eq!(code_of(again), ErrorCode::Conflict);
}

#[tokio::test]
async fn wrong_password_and_unknown_user_get_the_same_answer() {
    let t = TestEngine::start().await;
    t.bootstrap().await;
    let wrong_password = t.sign_in(ADMIN, "not the passphrase").await.unwrap_err();
    let unknown_user = t.sign_in("nobody", PASSWORD).await.unwrap_err();
    assert_eq!(wrong_password.code(), ErrorCode::Unauthenticated);
    assert_eq!(answer(wrong_password), answer(unknown_user));
}

#[tokio::test]
async fn five_failures_lock_the_username_until_the_lock_expires() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    t.engine.sign_out().await.unwrap();
    for _ in 0..5 {
        assert_eq!(
            code_of(t.sign_in(ADMIN, "not the passphrase").await),
            ErrorCode::Unauthenticated
        );
    }
    let locked = t.sign_in(ADMIN, PASSWORD).await.unwrap_err();
    let locked_unknown = {
        for _ in 0..5 {
            let _ = t.sign_in("ghost", "not the passphrase").await;
        }
        t.sign_in("ghost", PASSWORD).await.unwrap_err()
    };
    assert_eq!(answer(locked), answer(locked_unknown));
    assert_eq!(
        t.events("security")
            .iter()
            .filter(|(a, _)| a == "auth.lockout")
            .count(),
        2
    );

    t.advance(15 * 60);
    t.sign_in_with(ADMIN, PASSWORD, &totp).await;
}

#[tokio::test]
async fn mfa_failures_count_towards_the_lockout() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    t.engine.sign_out().await.unwrap();
    t.advance(30);
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    for _ in 0..4 {
        assert_eq!(
            code_of(t.verify(&step, "000000x").await),
            ErrorCode::Unauthenticated
        );
    }
    assert_eq!(
        code_of(t.verify(&step, "000000x").await),
        ErrorCode::Unauthenticated
    );
    // The lockout ended the sign-in, and even the right code is refused now.
    assert_eq!(
        code_of(t.verify(&step, &t.code(&totp)).await),
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(t.sign_in(ADMIN, PASSWORD).await),
        ErrorCode::Unauthenticated
    );
}

#[tokio::test]
async fn a_used_totp_code_is_refused_until_the_next_time_step() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    t.engine.sign_out().await.unwrap();
    // Same instant: the enrolment already used this step's code.
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    let replay = t.verify(&step, &t.code(&totp)).await;
    assert_eq!(code_of(replay), ErrorCode::Unauthenticated);
    let reasons: Vec<String> = t
        .conn()
        .prepare("SELECT json_extract(details, '$.reason_code') FROM audit_events WHERE action = 'auth.mfa_verify'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(reasons, ["replayed_code"]);
    t.advance(30);
    assert!(
        t.verify(&step, &t.code(&totp)).await.is_ok(),
        "a wrong code leaves the challenge live"
    );
}

#[tokio::test]
async fn temporary_password_must_be_replaced_before_mfa() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("analyst1", &[RoleCode::Analyst]).await;
    let step = t.sign_in("analyst1", PASSWORD).await.unwrap();
    assert!(
        matches!(step, SignInStep::PasswordChangeRequired { .. }),
        "{step:?}"
    );
    assert_eq!(
        code_of(t.verify(&step, "123456").await),
        ErrorCode::NotFound
    );
    let same = t
        .engine
        .change_initial_password(ChangeInitialPasswordInput {
            challenge_id: challenge_id(&step),
            new_password: secret(PASSWORD),
        })
        .await;
    assert_eq!(code_of(same), ErrorCode::Validation);
    let (session, _) = {
        t.engine.sign_out().await.unwrap();
        t.first_sign_in("analyst1").await
    };
    assert_eq!(session.capabilities, ["perform_analysis"]);
}

#[tokio::test]
async fn disabled_user_cannot_sign_in() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let user = t.create_user("analyst1", &[RoleCode::Analyst]).await;
    t.engine
        .disable_user(lis_auth::ChangeUserStatusInput {
            user_id: user.user_id,
            expected_revision: user.revision,
            reason: "Left the lab".into(),
        })
        .await
        .unwrap();
    let refused = t.sign_in("analyst1", PASSWORD).await.unwrap_err();
    let wrong = t
        .sign_in("analyst1", "not the passphrase")
        .await
        .unwrap_err();
    assert_eq!(answer(refused), answer(wrong));
}
