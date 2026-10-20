use super::*;
use lis_auth::{DataKey, Rejection, SetUnlockPinInput, UnlockPinStatus};

const PIN: &str = "482905";

pub(super) fn enable(f: &mut Fixture) {
    let password = f.proof(ReauthPurpose::UnlockPinChange);
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::set_unlock_pin(
                ctx,
                &DataKey::from_bytes([7; 32]),
                SetUnlockPinInput {
                    pin: SecretString::from(PIN),
                    password,
                },
            )
        })
        .unwrap(),
    );
}

fn status(f: &Fixture) -> UnlockPinStatus {
    lis_auth::get_unlock_pin_status(&f.conn(), f.user_id, f.session.id).unwrap()
}

pub(super) fn attempt(
    f: &Fixture,
    value: &str,
) -> Result<Attempt<lis_auth::SessionUser>, AuthError> {
    let pin = SecretString::from(value);
    write(&f.db, &f.principal, move |ctx| {
        lis_auth::unlock_with_pin(ctx, &DataKey::from_bytes([7; 32]), &pin)
    })
}

#[test]
fn pin_is_optional_and_unlock_does_not_create_a_session() {
    let mut f = Fixture::new();
    assert!(!status(&f).enabled);
    assert_eq!(status(&f).attempts_remaining, 0);
    assert!(matches!(
        attempt(&f, PIN),
        Err(AuthError::UnlockPinUnavailable)
    ));
    assert!(!lis_auth::has_sealed_secrets(&f.conn()).unwrap());
    enable(&mut f);
    assert!(status(&f).enabled);
    assert_eq!(status(&f).attempts_remaining, 5);
    let user = accepted(attempt(&f, PIN).unwrap());
    assert_eq!(user.user_id, f.user_id);
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM auth_sessions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn pin_verifier_is_encrypted_and_bound_to_the_installation_key() {
    let mut f = Fixture::new();
    enable(&mut f);
    let sealed: Vec<u8> = f
        .conn()
        .query_row("SELECT verifier FROM auth_unlock_pins", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(!sealed.windows(PIN.len()).any(|part| part == PIN.as_bytes()));
    assert!(!sealed.windows(9).any(|part| part == b"$argon2id"));
    assert!(lis_auth::has_sealed_secrets(&f.conn()).unwrap());
    assert!(lis_auth::key_opens_sealed_secrets(&f.conn(), &DataKey::from_bytes([7; 32])).unwrap());
    assert!(!lis_auth::key_opens_sealed_secrets(&f.conn(), &DataKey::from_bytes([8; 32])).unwrap());
    assert!(matches!(
        write(&f.db, &f.principal, |ctx| {
            lis_auth::unlock_with_pin(ctx, &DataKey::from_bytes([8; 32]), &SecretString::from(PIN))
        }),
        Err(AuthError::SecretCipher)
    ));
    assert_eq!(
        status(&f).attempts_remaining,
        5,
        "wrong installation key is not a PIN guess"
    );
}

#[test]
fn malformed_and_wrong_pins_persist_and_fifth_failure_ends_only_the_session() {
    let mut f = Fixture::new();
    enable(&mut f);
    for value in ["wrong", "12345"] {
        assert!(matches!(
            attempt(&f, value).unwrap(),
            Attempt::Rejected(Rejection::WrongUnlockPin)
        ));
    }
    f.db = f.db.reopen().unwrap();
    assert_eq!(status(&f).attempts_remaining, 3);
    for value in ["１２３４５６", "482906"] {
        assert!(matches!(
            attempt(&f, value).unwrap(),
            Attempt::Rejected(Rejection::WrongUnlockPin)
        ));
    }
    assert!(matches!(
        attempt(&f, "482906").unwrap(),
        Attempt::Rejected(Rejection::UnlockPinAttemptsExhausted)
    ));
    assert!(!lis_auth::session_is_open(&f.conn(), f.session.id, f.user_id).unwrap());
    assert!(matches!(
        attempt(&f, PIN),
        Err(AuthError::SessionUnavailable)
    ));
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM auth_unlock_pins", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_exhaustion_audit(&f);
}

fn assert_exhaustion_audit(f: &Fixture) {
    let audit: String = f.conn().query_row("SELECT details FROM audit_events WHERE action = 'auth.unlock_with_pin' ORDER BY seq DESC LIMIT 1", [], |row| row.get(0)).unwrap();
    assert!(audit.contains("attempts_exhausted"));
    assert!(audit.contains("\"attempts_remaining\":0"));
    assert!(!audit.contains(PIN));
    assert_eq!(Rejection::WrongUnlockPin.field_errors()[0].field, "pin");
    assert_eq!(
        Rejection::UnlockPinAttemptsExhausted.code(),
        lis_core::ErrorCode::Unauthenticated
    );
}

#[test]
fn pin_success_resets_its_counter_and_normal_key_unlock_does_not() {
    let mut f = Fixture::new();
    enable(&mut f);
    assert!(matches!(
        attempt(&f, "482906").unwrap(),
        Attempt::Rejected(_)
    ));
    let assertion = f.assertion();
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_key_unlock(ctx, assertion)
        })
        .unwrap(),
    );
    assert_eq!(status(&f).attempts_remaining, 4);
    accepted(attempt(&f, PIN).unwrap());
    assert_eq!(status(&f).attempts_remaining, 5);
}

#[test]
fn pin_cannot_sign_in_switch_user_or_confirm_sensitive_actions() {
    let mut f = Fixture::new();
    enable(&mut f);
    assert!(matches!(
        write(&f.db, &public(), |ctx| {
            lis_auth::unlock_with_pin(ctx, &DataKey::from_bytes([7; 32]), &SecretString::from(PIN))
        }),
        Err(AuthError::SessionUnavailable)
    ));
    let other_session = Principal::user(
        f.user_id,
        SessionId::generate(),
        f.session.workstation_id,
        [],
    );
    assert!(matches!(
        write(&f.db, &other_session, |ctx| {
            lis_auth::unlock_with_pin(ctx, &DataKey::from_bytes([7; 32]), &SecretString::from(PIN))
        }),
        Err(AuthError::SessionUnavailable)
    ));
    assert!(matches!(
        write(&f.db, &f.principal, |ctx| {
            lis_auth::reauthenticate(
                ctx,
                &SecretString::from(PIN).into(),
                ReauthPurpose::ReportRelease,
            )
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
    assert!(matches!(
        write(&f.db, &f.principal, |ctx| {
            lis_auth::disable_unlock_pin(
                ctx,
                lis_auth::DisableUnlockPinInput {
                    password: SecretString::from(PIN).into(),
                },
            )
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
    assert!(status(&f).enabled);
    let proof = f.proof(ReauthPurpose::UnlockPinChange);
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::disable_unlock_pin(ctx, lis_auth::DisableUnlockPinInput { password: proof })
        })
        .unwrap(),
    );
    assert!(!status(&f).enabled);
    assert!(matches!(
        attempt(&f, PIN),
        Err(AuthError::UnlockPinUnavailable)
    ));
}

#[test]
fn changing_pin_invalidates_earlier_proofs_and_ends_other_sessions() {
    let mut f = Fixture::new();
    let earlier = f.proof(ReauthPurpose::ReportRelease);
    let assertion = f.assertion();
    let other = NewSession {
        id: SessionId::generate(),
        ..f.session.clone()
    };
    let opened = other.clone();
    accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &opened)
        })
        .unwrap(),
    );
    enable(&mut f);
    assert!(!lis_auth::session_is_open(&f.conn(), other.id, f.user_id).unwrap());
    assert!(lis_auth::session_is_open(&f.conn(), f.session.id, f.user_id).unwrap());
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| lis_auth::reauthenticate(
            ctx,
            &earlier,
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
    assert!(matches!(
        attempt(&f, "482906").unwrap(),
        Attempt::Rejected(_)
    ));
    enable(&mut f);
    assert_eq!(status(&f).attempts_remaining, 5);
}

#[test]
fn disabled_account_cannot_use_a_retained_pin() {
    let mut f = Fixture::new();
    enable(&mut f);
    f.conn()
        .execute("UPDATE auth_users SET status = 'disabled'", [])
        .unwrap();
    assert!(matches!(
        attempt(&f, PIN),
        Err(AuthError::SessionUnavailable)
    ));
}
