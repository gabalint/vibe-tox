use super::*;
use lis_auth::{Reauthentication, SetSecurityPolicyInput};

fn policy_input(
    version: Option<i64>,
    allow_touch_only: bool,
    password: Option<Reauthentication>,
) -> SetSecurityPolicyInput {
    SetSecurityPolicyInput {
        expected_version: version,
        idle_timeout_minutes: 15,
        lockout_failures: 5,
        lockout_minutes: 15,
        allow_security_key_touch_only: allow_touch_only,
        password,
        reason: Some("Synthetic laboratory authentication decision".to_owned()),
    }
}

async fn enroll_touch_key(
    f: &Fixture,
    key: &TestKey,
) -> Result<lis_engine::KeyRecoveryResult, CommandError> {
    f.engine
        .security_key_enroll(
            SecurityKeyEnrollInput {
                key_label: "Synthetic touch key".to_owned(),
                password: secret(PASSWORD).into(),
                credential_id: None,
            },
            key,
        )
        .await
}

#[tokio::test]
async fn touch_only_enrollment_requires_explicit_laboratory_policy() {
    let f = Fixture::new().await;
    f.password_session().await;
    let key = TestKey::with_user_verification(false);
    assert!(enroll_touch_key(&f, &key).await.is_err());
    assert_eq!(
        f.engine.get_authentication_status().await.unwrap().method,
        AuthenticationMethod::PasswordTotp
    );
    assert!(
        !f.engine
            .get_security_policy()
            .await
            .unwrap()
            .values
            .allow_security_key_touch_only
    );
    let input = policy_input(None, true, None);
    assert!(f.engine.set_security_policy(input).await.is_err());
    assert!(
        f.engine
            .get_security_policy()
            .await
            .unwrap()
            .version
            .is_none()
    );
    f.engine
        .set_security_policy(policy_input(None, true, Some(secret(PASSWORD).into())))
        .await
        .unwrap();
    enroll_touch_key(&f, &key).await.unwrap();
    let id = f.key_id().await;
    f.engine.sign_out().await.unwrap();
    f.engine
        .security_key_sign_in(
            SecurityKeySignInInput {
                username: USER.to_owned(),
                credential_id: id,
            },
            &key,
        )
        .await
        .unwrap();
    f.engine.lock_session().await.unwrap();
    assert!(
        !f.engine
            .security_key_unlock(SecurityKeyUnlockInput { credential_id: id }, &key)
            .await
            .unwrap()
            .locked
    );
    let error = f
        .engine
        .security_key_action(Fixture::create_action(id), None, &key)
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Unauthenticated);
    assert_eq!(f.engine.list_users().await.unwrap().len(), 1);
    let proof_count: i64 = f
        .conn()
        .query_row("SELECT count(*) FROM auth_key_action_proofs", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(proof_count, 0);
}

#[tokio::test]
async fn key_confirmed_policy_change_preserves_strong_credentials() {
    let (f, _) = Fixture::signed_in().await;
    let id = f.key_id().await;
    let before: String = f
        .conn()
        .query_row(
            "SELECT credential_json FROM auth_security_keys",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let input = policy_input(None, true, Some(secret("ignored key placeholder").into()));
    f.engine
        .security_key_action(
            SecurityKeyActionInput {
                credential_id: id,
                action: SecurityKeyAction::SetSecurityPolicy(input),
            },
            None,
            &f.key,
        )
        .await
        .unwrap();
    assert!(
        f.engine
            .get_security_policy()
            .await
            .unwrap()
            .values
            .allow_security_key_touch_only
    );
    let after: String = f
        .conn()
        .query_row(
            "SELECT credential_json FROM auth_security_keys",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let before: Value = serde_json::from_str(&before).unwrap();
    let after: Value = serde_json::from_str(&after).unwrap();
    assert_eq!(
        before["cred"]["registration_policy"],
        after["cred"]["registration_policy"]
    );
    assert_eq!(
        before["cred"]["user_verified"],
        after["cred"]["user_verified"]
    );
    assert_eq!(after["cred"]["registration_policy"], "required");
    f.engine.sign_out().await.unwrap();
    f.engine
        .security_key_sign_in(
            SecurityKeySignInInput {
                username: USER.to_owned(),
                credential_id: id,
            },
            &f.key,
        )
        .await
        .unwrap();
}

struct TightenDuringRegistration<'a> {
    engine: &'a Engine,
    key: TestKey,
}

impl SecurityKeyClient for TightenDuringRegistration<'_> {
    async fn register(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        let response = self.key.register(origin, options).await?;
        self.engine
            .set_security_policy(policy_input(Some(1), false, Some(secret(PASSWORD).into())))
            .await?;
        Ok(response)
    }

    async fn authenticate(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("password-account enrollment must not request an existing-key assertion")
    }
}

#[tokio::test]
async fn tightening_policy_during_enrollment_cancels_the_old_ceremony() {
    let f = Fixture::new().await;
    f.password_session().await;
    f.engine
        .set_security_policy(policy_input(None, true, Some(secret(PASSWORD).into())))
        .await
        .unwrap();
    let client = TightenDuringRegistration {
        engine: &f.engine,
        key: TestKey::with_user_verification(false),
    };
    let result = f
        .engine
        .security_key_enroll(
            SecurityKeyEnrollInput {
                key_label: "Stale enrollment".to_owned(),
                password: secret(PASSWORD).into(),
                credential_id: None,
            },
            &client,
        )
        .await;
    assert!(result.is_err());
    assert_eq!(
        f.engine.get_authentication_status().await.unwrap().method,
        AuthenticationMethod::PasswordTotp
    );
    assert!(
        !f.engine
            .get_security_policy()
            .await
            .unwrap()
            .values
            .allow_security_key_touch_only
    );
    assert!(
        f.engine
            .list_key_sign_in_choices(KeySignInChoicesInput {
                username: USER.to_owned()
            })
            .await
            .unwrap()
            .is_empty()
    );
}
