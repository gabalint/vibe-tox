//! User administration through the engine: guards, audited reasons, fresh
//! authentication and the separation of account administration from clinical
//! authority (REQ-IDN-004; security review 2026-10-13).

use lis_auth::{
    ChangeUserStatusInput, CreateUserInput, ResetUserCredentialsInput, RoleCode, SetUserRolesInput,
    SignInStep, UserSummary,
};
use lis_core::ErrorCode;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of, secret};

fn roles_input(user: &UserSummary, roles: &[RoleCode], password: &str) -> SetUserRolesInput {
    SetUserRolesInput {
        user_id: user.user_id,
        roles: roles.to_vec(),
        expected_revision: user.revision,
        reason: "Completed training".into(),
        password: secret(password).into(),
    }
}

fn reset_input(user: &UserSummary, password: &str) -> ResetUserCredentialsInput {
    ResetUserCredentialsInput {
        user_id: user.user_id,
        temporary_password: secret("temporary passphrase 9"),
        reset_mfa: true,
        expected_revision: user.revision,
        reason: "Lost phone".into(),
        password: secret(password).into(),
    }
}

/// Creates an account without roles as the signed-in administrator.
async fn create_plain_user(t: &TestEngine, username: &str) -> UserSummary {
    t.engine
        .create_user(CreateUserInput {
            username: username.into(),
            display_name: format!("Test {username}"),
            roles: vec![],
            temporary_password: secret(PASSWORD),
            password: secret(PASSWORD).into(),
        })
        .await
        .unwrap()
}

async fn user_named(t: &TestEngine, username: &str) -> UserSummary {
    t.engine
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|user| user.username == username)
        .unwrap()
}

/// `(action, outcome, capability)` of security events with these actions: the
/// granted authority, or for a capability denial the capability missing.
fn security_events(t: &TestEngine, actions: &[&str]) -> Vec<(String, String, Option<String>)> {
    let conn = t.conn();
    let mut statement = conn
        .prepare(
            "SELECT action, outcome, coalesce(authority, json_extract(details, '$.capability')) \
             FROM audit_events WHERE stream = 'security' ORDER BY seq",
        )
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<(String, String, Option<String>)>, _>>()
        .unwrap()
        .into_iter()
        .filter(|(action, _, _)| actions.contains(&action.as_str()))
        .collect()
}

#[tokio::test]
async fn own_account_and_last_administrator_are_protected() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let me = t.engine.list_users().await.unwrap().remove(0);
    let demote = t
        .engine
        .set_user_roles(roles_input(&me, &[RoleCode::Auditor], PASSWORD))
        .await;
    assert_eq!(code_of(demote), ErrorCode::Forbidden);
    let own_reset = t
        .engine
        .reset_user_credentials(reset_input(&me, PASSWORD))
        .await;
    assert_eq!(code_of(own_reset), ErrorCode::Forbidden);
    let disable = t
        .engine
        .disable_user(ChangeUserStatusInput {
            user_id: me.user_id,
            expected_revision: me.revision,
            reason: "Leaving".into(),
        })
        .await;
    assert_eq!(code_of(disable), ErrorCode::Conflict);
    let denied = security_events(&t, &["auth.user_roles_set", "auth.credentials_reset"]);
    let manage_users = Some("manage_users".to_owned());
    assert_eq!(
        denied,
        [
            (
                "auth.user_roles_set".into(),
                "denied".into(),
                manage_users.clone()
            ),
            (
                "auth.credentials_reset".into(),
                "denied".into(),
                manage_users
            ),
        ]
    );
}

/// Applies one change of each kind to `user`, with distinct reasons.
async fn change_everything(t: &TestEngine, user: UserSummary) {
    let mut change = roles_input(&user, &[RoleCode::Billing, RoleCode::Auditor], PASSWORD);
    change.reason = "Joined billing".into();
    let user = t.engine.set_user_roles(change).await.unwrap();
    assert_eq!(user.roles, [RoleCode::Billing, RoleCode::Auditor]);
    let status = |user: &UserSummary, reason: &str| ChangeUserStatusInput {
        user_id: user.user_id,
        expected_revision: user.revision,
        reason: reason.into(),
    };
    let user = t
        .engine
        .disable_user(status(&user, "Extended leave"))
        .await
        .unwrap();
    let user = t
        .engine
        .enable_user(status(&user, "Returned"))
        .await
        .unwrap();
    t.engine
        .reset_user_credentials(reset_input(&user, PASSWORD))
        .await
        .unwrap();
}

#[tokio::test]
async fn every_administrative_change_is_audited_with_its_reason() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let user = create_plain_user(&t, "clerk1").await;
    let stale = t
        .engine
        .disable_user(ChangeUserStatusInput {
            user_id: user.user_id,
            expected_revision: user.revision + 1,
            reason: "Stale".into(),
        })
        .await;
    assert_eq!(code_of(stale), ErrorCode::Conflict);
    change_everything(&t, user).await;
    let audited: Vec<(String, Option<String>, Option<String>)> = t
        .conn()
        .prepare(
            "SELECT action, reason, authority FROM audit_events WHERE stream = 'security' \
             AND action IN ('auth.user_create', 'auth.user_roles_set', 'auth.user_disable', \
             'auth.user_enable', 'auth.credentials_reset') ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let authority = Some("manage_users".to_owned());
    let reasons: Vec<Option<&str>> = audited
        .iter()
        .map(|(_, reason, _)| reason.as_deref())
        .collect();
    assert_eq!(
        reasons,
        [
            None,
            Some("Joined billing"),
            Some("Extended leave"),
            Some("Returned"),
            Some("Lost phone")
        ]
    );
    assert!(audited.iter().all(|(_, _, granted)| *granted == authority));
}

#[tokio::test]
async fn a_clinical_role_needs_an_administrator_and_a_quality_manager() {
    let t = TestEngine::start().await;
    let admin_totp = t.admin_session().await;
    // Nobody holds the quality manager role yet, so administration appoints one.
    let qm = create_plain_user(&t, "qm1").await;
    t.engine
        .set_user_roles(roles_input(&qm, &[RoleCode::QualityManager], PASSWORD))
        .await
        .unwrap();
    let person = create_plain_user(&t, "person1").await;
    let refused = t
        .engine
        .set_user_roles(roles_input(&person, &[RoleCode::Releaser], PASSWORD))
        .await;
    assert_eq!(code_of(refused), ErrorCode::Forbidden);
    let offered: Vec<RoleCode> = t
        .engine
        .list_roles()
        .await
        .unwrap()
        .into_iter()
        .filter(|role| role.assignable)
        .map(|role| role.code)
        .collect();
    assert_eq!(
        offered,
        [
            RoleCode::Billing,
            RoleCode::Auditor,
            RoleCode::Administrator
        ]
    );
    t.engine.sign_out().await.unwrap();
    t.first_sign_in("qm1").await;
    let person = user_named(&t, "person1").await;
    let granted = t
        .engine
        .set_user_roles(roles_input(&person, &[RoleCode::Releaser], NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(granted.roles, [RoleCode::Releaser]);
    let new_account = t
        .engine
        .create_user(CreateUserInput {
            username: "person2".into(),
            display_name: "Test person2".into(),
            roles: vec![],
            temporary_password: secret(PASSWORD),
            password: secret(NEW_PASSWORD).into(),
        })
        .await;
    assert_eq!(code_of(new_account), ErrorCode::Forbidden);
    t.engine.sign_out().await.unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &admin_totp).await;
    let changes = security_events(&t, &["auth.user_roles_set"]);
    let outcome = |outcome: &str, authority: &str| {
        (
            "auth.user_roles_set".to_owned(),
            outcome.to_owned(),
            Some(authority.to_owned()),
        )
    };
    assert_eq!(
        changes,
        [
            outcome("succeeded", "manage_users"),
            outcome("denied", "manage_quality"),
            outcome("succeeded", "manage_quality"),
        ]
    );
}

#[tokio::test]
async fn resetting_a_clinical_users_credentials_needs_both_duties() {
    let t = TestEngine::start().await;
    let admin_totp = t.admin_session().await;
    let qm = create_plain_user(&t, "qm-reset").await;
    t.engine
        .set_user_roles(roles_input(&qm, &[RoleCode::QualityManager], PASSWORD))
        .await
        .unwrap();
    create_plain_user(&t, "releaser-reset").await;

    t.engine.sign_out().await.unwrap();
    let (_, qm_totp) = t.first_sign_in("qm-reset").await;
    let releaser = user_named(&t, "releaser-reset").await;
    let releaser = t
        .engine
        .set_user_roles(roles_input(&releaser, &[RoleCode::Releaser], NEW_PASSWORD))
        .await
        .unwrap();

    t.engine.sign_out().await.unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &admin_totp).await;
    let refused = t
        .engine
        .reset_user_credentials(reset_input(&releaser, PASSWORD))
        .await;
    assert_eq!(code_of(refused), ErrorCode::Forbidden);
    assert!(security_events(&t, &["auth.credentials_reset"]).iter().any(
        |(_, outcome, authority)| {
            outcome == "denied" && authority.as_deref() == Some("manage_quality")
        }
    ));

    let qm = user_named(&t, "qm-reset").await;
    t.engine
        .set_user_roles(roles_input(
            &qm,
            &[RoleCode::QualityManager, RoleCode::Administrator],
            PASSWORD,
        ))
        .await
        .unwrap();
    t.engine.sign_out().await.unwrap();
    t.sign_in_with("qm-reset", NEW_PASSWORD, &qm_totp).await;
    t.engine
        .reset_user_credentials(reset_input(&releaser, NEW_PASSWORD))
        .await
        .unwrap();
}

#[tokio::test]
async fn credentials_reset_needs_the_password_and_forces_a_new_sign_in() {
    let t = TestEngine::start().await;
    let admin_totp = t.admin_session().await;
    create_plain_user(&t, "clerk1").await;
    let (_, clerk_totp) = t.first_sign_in("clerk1").await;
    t.engine.sign_out().await.unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &admin_totp).await;
    let clerk = user_named(&t, "clerk1").await;
    assert!(clerk.mfa_enrolled);
    let wrong = t
        .engine
        .reset_user_credentials(reset_input(&clerk, "wrong passphrase"))
        .await;
    assert_eq!(code_of(wrong), ErrorCode::Validation);
    // A sign-in begun with the old password cannot finish after the reset.
    let pending = t.sign_in("clerk1", NEW_PASSWORD).await.unwrap();
    let reset = t
        .engine
        .reset_user_credentials(reset_input(&clerk, PASSWORD))
        .await
        .unwrap();
    assert!(!reset.mfa_enrolled);
    t.advance(30);
    let finished = t.verify(&pending, &t.code(&clerk_totp)).await;
    assert_eq!(code_of(finished), ErrorCode::NotFound);
    t.engine.sign_out().await.unwrap();
    let step = t.sign_in("clerk1", "temporary passphrase 9").await.unwrap();
    assert!(
        matches!(step, SignInStep::PasswordChangeRequired { .. }),
        "{step:?}"
    );
}
