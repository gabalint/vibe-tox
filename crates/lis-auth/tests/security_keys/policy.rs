//! Laboratory opt-in never weakens historical credentials or sensitive actions.

use super::*;
use lis_auth::{KeyAuthenticationPurpose, SetSecurityPolicyInput};
use serde_json::Value;

fn policy_input(f: &Fixture, enabled: bool) -> SetSecurityPolicyInput {
    let policy =
        f.db.db
            .read_blocking(lis_auth::get_security_policy)
            .unwrap();
    SetSecurityPolicyInput {
        expected_version: policy.version,
        idle_timeout_minutes: policy.values.idle_timeout_minutes,
        lockout_failures: policy.values.lockout_failures,
        lockout_minutes: policy.values.lockout_minutes,
        allow_security_key_touch_only: enabled,
        password: None,
        reason: Some("Synthetic laboratory assessed this sign-in policy".into()),
    }
}

fn set_policy(f: &mut Fixture, enabled: bool) {
    let mut input = policy_input(f, enabled);
    input.password = Some(f.proof(ReauthPurpose::SecurityKeyPolicy));
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_security_policy(ctx, input)
        })
        .unwrap(),
    );
}

fn own_account(f: &mut Fixture) -> KeyRegistrationAccount {
    let proof = f.proof(ReauthPurpose::SecurityKeyEnrollment);
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::prepare_own_key_registration(ctx, &proof)
        })
        .unwrap(),
    )
}

fn touch_key(f: &mut Fixture) -> (TestKey, SecurityKeyId) {
    let account = own_account(f);
    let mut key = WebauthnAuthenticator::new(SoftPasskey::new(false));
    let registration = register(&f.verifier, account, &mut key);
    write(&f.db, &f.principal, move |ctx| {
        lis_auth::complete_own_key_registration(ctx, registration, "Touch only")
    })
    .unwrap();
    let user_id = f.user_id;
    let id =
        f.db.db
            .read_blocking(move |conn| lis_auth::list_security_keys(conn, user_id))
            .unwrap()
            .into_iter()
            .find(|key| key.label == "Touch only")
            .unwrap()
            .credential_id;
    (key, id)
}

fn begin(
    f: &Fixture,
    id: SecurityKeyId,
    purpose: KeyAuthenticationPurpose,
) -> (Value, lis_auth::PendingKeyAuthentication) {
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication_for_purpose(ctx, "key_operator", id, purpose)
        })
        .unwrap(),
    );
    f.verifier.begin_authentication(account).unwrap()
}

fn respond(key: &mut TestKey, options: Value) -> Value {
    serde_json::to_value(
        key.do_authentication(
            Url::parse(ORIGIN).unwrap(),
            serde_json::from_value(options).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn relaxing_first_policy_needs_reason_and_fresh_confirmation() {
    let mut f = Fixture::new();
    let mut input = policy_input(&f, true);
    input.reason = None;
    let missing_reason = write(&f.db, &f.principal, move |ctx| {
        lis_auth::set_security_policy(ctx, input)
    })
    .unwrap_err();
    assert_eq!(missing_reason.field_errors()[0].field, "reason");
    let input = policy_input(&f, true);
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_security_policy(ctx, input)
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
    assert_eq!(
        f.db.db
            .read_blocking(lis_auth::get_security_policy)
            .unwrap()
            .version,
        None
    );
    set_policy(&mut f, true);
    let (reason, enabled): (String, bool) = f.conn().query_row(
        "SELECT reason, allow_security_key_touch_only FROM auth_security_policies WHERE version=1",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert!(!reason.is_empty());
    assert!(enabled);
    let details: String = f.conn().query_row(
        "SELECT details FROM audit_events WHERE action='auth.set_security_policy' ORDER BY seq DESC LIMIT 1",
        [], |row| row.get(0),
    ).unwrap();
    assert!(details.contains("\"allow_security_key_touch_only\":true"));
}

#[test]
fn touch_only_sign_in_cannot_issue_sensitive_proof_and_requests_stronger_uv() {
    let mut f = Fixture::new();
    set_policy(&mut f, true);
    let (mut key, id) = touch_key(&mut f);
    let (options, pending) = begin(&f, id, KeyAuthenticationPurpose::SignInOrUnlock);
    assert_eq!(options["publicKey"]["userVerification"], "preferred");
    let assertion = f
        .verifier
        .finish_authentication(pending, respond(&mut key, options))
        .unwrap();
    let no_proof = write(&f.db, &f.principal, move |ctx| {
        lis_auth::issue_key_action_proof(ctx, assertion, ReauthPurpose::ReportRelease)
    });
    assert!(matches!(no_proof, Err(AuthError::SecurityKeyVerification)));

    let (mut options, pending) = begin(&f, id, KeyAuthenticationPurpose::SensitiveAction);
    assert_eq!(options["publicKey"]["userVerification"], "required");
    // A hostile client ignores Required and returns a real signature without UV.
    options["publicKey"]["userVerification"] = "preferred".into();
    assert!(matches!(
        f.verifier
            .finish_authentication(pending, respond(&mut key, options)),
        Err(AuthError::SecurityKeyVerification)
    ));

    let (options, pending) = begin(&f, id, KeyAuthenticationPurpose::SignInOrUnlock);
    let assertion = f
        .verifier
        .finish_authentication(pending, respond(&mut key, options))
        .unwrap();
    let mut session = f.session.clone();
    session.id = SessionId::generate();
    let user = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &session)
        })
        .unwrap(),
    );
    assert_eq!(user.user_id, f.user_id);
}

#[test]
fn tightening_rejects_pending_assertion_and_pending_optional_registration() {
    let mut f = Fixture::new();
    set_policy(&mut f, true);
    let (mut key, id) = touch_key(&mut f);
    let (options, pending) = begin(&f, id, KeyAuthenticationPurpose::SignInOrUnlock);
    let assertion = f
        .verifier
        .finish_authentication(pending, respond(&mut key, options))
        .unwrap();
    let account = own_account(&mut f);
    let mut spare = WebauthnAuthenticator::new(SoftPasskey::new(false));
    let registration = register(&f.verifier, account, &mut spare);
    set_policy(&mut f, false);
    let session = f.session.clone();
    assert!(matches!(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &session)
        }),
        Err(AuthError::ChallengeExpired)
    ));
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_own_key_registration(ctx, registration, "Late key")
        }),
        Err(AuthError::ChallengeExpired)
    ));
    let (mut options, pending) = begin(&f, id, KeyAuthenticationPurpose::SignInOrUnlock);
    assert_eq!(options["publicKey"]["userVerification"], "required");
    options["publicKey"]["userVerification"] = "preferred".into();
    assert!(matches!(
        f.verifier
            .finish_authentication(pending, respond(&mut key, options)),
        Err(AuthError::SecurityKeyVerification)
    ));
}

#[test]
fn enrollment_options_and_storage_preserve_strong_credentials_and_recovery() {
    let mut f = Fixture::new();
    let legacy = |f: &Fixture| -> Value {
        let text: String = f
            .conn()
            .query_row(
                "SELECT credential_json FROM auth_security_keys WHERE id=?1",
                [f.key_id],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&text).unwrap()
    };
    let original = legacy(&f);
    assert_eq!(original["cred"]["registration_policy"], "required");
    set_policy(&mut f, true);
    let preserved = legacy(&f);
    assert_eq!(
        preserved["cred"]["registration_policy"],
        original["cred"]["registration_policy"]
    );
    assert_eq!(
        preserved["cred"]["user_verified"],
        original["cred"]["user_verified"]
    );
    assert!(preserved.get("securityKey").is_none());
    let (options, _) = begin(&f, f.key_id, KeyAuthenticationPurpose::SignInOrUnlock);
    assert_eq!(options["publicKey"]["userVerification"], "required");
    let account = own_account(&mut f);
    let (options, _) = f.verifier.begin_registration(account).unwrap();
    assert_eq!(
        options["publicKey"]["authenticatorSelection"]["userVerification"],
        "preferred"
    );
    assert!(
        options["publicKey"]["extensions"]
            .get("credentialProtectionPolicy")
            .is_none()
    );
    let recovery = f.recovery_code.clone();
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_recovery(ctx, "key_operator", &recovery)
        })
        .unwrap(),
    );
    let (options, _) = f.verifier.begin_registration(account).unwrap();
    assert_eq!(
        options["publicKey"]["authenticatorSelection"]["userVerification"],
        "required"
    );
    let (_, id) = touch_key(&mut f);
    let json: String = f
        .conn()
        .query_row(
            "SELECT credential_json FROM auth_security_keys WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["securityKey"]["cred"]["registration_policy"],
        "preferred"
    );
    assert_eq!(value["securityKey"]["cred"]["user_verified"], false);
}

#[test]
fn policy_change_ends_other_sessions_and_clears_receipts_but_keeps_caller() {
    let mut f = Fixture::new();
    let assertion = f.assertion();
    let mut other = f.session.clone();
    other.id = SessionId::generate();
    let other_id = other.id;
    accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &other)
        })
        .unwrap(),
    );
    let _pending_proof = f.proof(ReauthPurpose::ReportRelease);
    set_policy(&mut f, true);
    assert!(lis_auth::session_is_open(&f.conn(), f.session.id, f.user_id).unwrap());
    assert!(!lis_auth::session_is_open(&f.conn(), other_id, f.user_id).unwrap());
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM auth_key_action_proofs", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn optional_policy_does_not_downgrade_a_key_enrolled_with_user_verification() {
    let mut f = Fixture::new();
    set_policy(&mut f, true);
    let account = own_account(&mut f);
    let (mut options, pending) = f.verifier.begin_registration(account).unwrap();
    // Simulate a key that uses its configured PIN under Preferred.
    options["publicKey"]["authenticatorSelection"]["userVerification"] = "required".into();
    let mut key = key();
    let response = key
        .do_registration(
            Url::parse(ORIGIN).unwrap(),
            serde_json::from_value(options).unwrap(),
        )
        .unwrap();
    let verified = f
        .verifier
        .finish_registration(pending, serde_json::to_value(response).unwrap())
        .unwrap();
    write(&f.db, &f.principal, move |ctx| {
        lis_auth::complete_own_key_registration(ctx, verified, "Preferred with PIN")
    })
    .unwrap();
    let id: SecurityKeyId = f
        .conn()
        .query_row(
            "SELECT id FROM auth_security_keys WHERE label='Preferred with PIN'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let (mut options, pending) = begin(&f, id, KeyAuthenticationPurpose::SignInOrUnlock);
    assert_eq!(options["publicKey"]["userVerification"], "required");
    options["publicKey"]["userVerification"] = "discouraged".into();
    assert!(matches!(
        f.verifier
            .finish_authentication(pending, respond(&mut key, options)),
        Err(AuthError::SecurityKeyVerification)
    ));
}

#[test]
fn numeric_policy_change_consumes_a_supplied_key_proof_and_rejects_wrong_purpose() {
    let mut f = Fixture::new();
    let mut input = policy_input(&f, false);
    input.idle_timeout_minutes = 10;
    input.password = Some(f.proof(ReauthPurpose::ReportRelease));
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_security_policy(ctx, input)
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
    assert_eq!(
        f.db.db
            .read_blocking(lis_auth::get_security_policy)
            .unwrap()
            .version,
        None
    );

    let assertion = f.assertion();
    let mut other = f.session.clone();
    other.id = SessionId::generate();
    let other_id = other.id;
    accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &other)
        })
        .unwrap(),
    );
    let proof = f.proof(ReauthPurpose::SecurityKeyPolicy);
    let replay = duplicate(&proof);
    let mut input = policy_input(&f, false);
    input.password = Some(proof);
    input.idle_timeout_minutes = 10;
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_security_policy(ctx, input)
        })
        .unwrap(),
    );
    // Numeric-only changes keep other sessions; the supplied proof is still one-use.
    assert!(lis_auth::session_is_open(&f.conn(), other_id, f.user_id).unwrap());
    let mut input = policy_input(&f, false);
    input.password = Some(replay);
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_security_policy(ctx, input)
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
}
