//! NFR-19: account changes invalidate even challenges already copied for queued work.

use lis_auth::{ChallengeSlot, DataKey, LiveChallenge, NewSession, SessionUser, SignInStep};
use lis_core::Clock;

use super::*;

const NEW_PASSWORD: &str = "synthetic replacement passphrase";

fn challenge(db: &TestDatabase, username: &str, password: &str) -> (SignInStep, LiveChallenge) {
    let input = SignInInput {
        username: username.to_owned(),
        password: SecretString::from(password),
    };
    let password = accepted(
        write(db, &sign_in_screen(), move |ctx| {
            lis_auth::check_password(ctx, &input)
        })
        .unwrap(),
    );
    issue(db, &password)
}

fn issue(db: &TestDatabase, password: &lis_auth::PasswordAccepted) -> (SignInStep, LiveChallenge) {
    let slot = ChallengeSlot::default();
    let step = slot.issue(password, db.clock.now()).unwrap();
    let id = match &step {
        SignInStep::PasswordChangeRequired { challenge_id }
        | SignInStep::MfaEnrollmentRequired { challenge_id, .. }
        | SignInStep::MfaRequired { challenge_id } => *challenge_id,
    };
    let live = slot.live(id, db.clock.now()).unwrap();
    // Model a command that already copied the slot before the engine clears it.
    slot.clear();
    (step, live)
}

fn enrollment(db: &TestDatabase, username: &str) -> (SignInStep, LiveChallenge) {
    let (_, live) = challenge(db, username, PASSWORD);
    let password = write(db, &sign_in_screen(), move |ctx| {
        lis_auth::set_initial_password(ctx, &live, SecretString::from(NEW_PASSWORD))
    })
    .unwrap();
    issue(db, &password)
}

fn code(db: &TestDatabase, secret: &str) -> SecretString {
    let secret = totp_rs::Secret::try_from_base32(secret).unwrap();
    let totp = totp_rs::Builder::new().with_secret(secret).build().unwrap();
    let seconds = u64::try_from(db.clock.now().unix_seconds()).unwrap();
    SecretString::from(totp.generate(seconds).to_string())
}

fn complete(
    db: &TestDatabase,
    live: LiveChallenge,
    code: SecretString,
) -> Result<Attempt<SessionUser>, AuthError> {
    let workstation_id = write(db, &sign_in_screen(), lis_auth::register_workstation).unwrap();
    write(db, &sign_in_screen(), move |ctx| {
        lis_auth::complete_sign_in(
            ctx,
            &DataKey::from_bytes([42; 32]),
            &live,
            &code,
            &NewSession {
                id: SessionId::generate(),
                workstation_id,
                webview: None,
                replaces: None,
            },
        )
    })
}

fn current_user(db: &TestDatabase, id: UserId) -> UserSummary {
    db.db
        .read_blocking(move |conn| lis_auth::list_users(conn, test_start()))
        .unwrap()
        .into_iter()
        .find(|user| user.user_id == id)
        .unwrap()
}

#[test]
fn a_copied_password_change_challenge_cannot_overwrite_a_reset_password() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let user = create_user(&db, &admin, "billing1", &[RoleCode::Billing]);
    let (_, live) = challenge(&db, "billing1", PASSWORD);
    accepted(reset(&db, &admin, &user, PASSWORD).unwrap());
    let before = audit_actions(&db);

    let result = write(&db, &sign_in_screen(), move |ctx| {
        lis_auth::set_initial_password(ctx, &live, SecretString::from(NEW_PASSWORD))
    });
    assert!(matches!(result, Err(AuthError::ChallengeExpired)));
    assert_eq!(audit_actions(&db), before);
    assert_eq!(
        sign_in(&db, "billing1", NEW_PASSWORD).unwrap(),
        Some(Rejection::InvalidCredentials)
    );
    assert_eq!(
        sign_in(&db, "billing1", "temporary passphrase 9").unwrap(),
        None
    );
}

#[test]
fn a_copied_enrollment_challenge_cannot_sign_in_after_a_reset() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let user = create_user(&db, &admin, "billing1", &[RoleCode::Billing]);
    let (step, live) = enrollment(&db, "billing1");
    let SignInStep::MfaEnrollmentRequired { enrollment, .. } = step else {
        panic!("expected enrollment");
    };
    let user = current_user(&db, user.user_id);
    accepted(reset(&db, &admin, &user, PASSWORD).unwrap());

    let result = complete(&db, live, code(&db, &enrollment.secret_base32));
    assert!(matches!(result, Err(AuthError::ChallengeExpired)));
    let user = current_user(&db, user.user_id);
    assert!(!user.mfa_enrolled);
    let sessions: i64 = connection(&db)
        .query_row("SELECT count(*) FROM auth_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 0);
}

#[test]
fn a_copied_mfa_challenge_is_revoked_even_when_the_authenticator_is_kept() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let user = create_user(&db, &admin, "billing1", &[RoleCode::Billing]);
    let (step, live) = enrollment(&db, "billing1");
    let SignInStep::MfaEnrollmentRequired { enrollment, .. } = step else {
        panic!("expected enrollment");
    };
    accepted(complete(&db, live, code(&db, &enrollment.secret_base32)).unwrap());
    db.clock
        .set(test_start().checked_add(Duration::from_secs(30)).unwrap());
    let (_, live) = challenge(&db, "billing1", NEW_PASSWORD);
    let user = current_user(&db, user.user_id);
    accepted(
        write(&db, &admin, move |ctx| {
            lis_auth::reset_user_credentials(
                ctx,
                lis_auth::ResetUserCredentialsInput {
                    user_id: user.user_id,
                    temporary_password: SecretString::from("temporary passphrase 9"),
                    reset_mfa: false,
                    expected_revision: user.revision,
                    reason: "Reset password only".to_owned(),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
        })
        .unwrap(),
    );

    let result = complete(&db, live, code(&db, &enrollment.secret_base32));
    assert!(matches!(result, Err(AuthError::ChallengeExpired)));
    let open_sessions: i64 = connection(&db)
        .query_row(
            "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(open_sessions, 0);
}

fn signed_in_administrator() -> (TestDatabase, UserId, SessionId, Principal) {
    let db = open();
    let (user_id, _) = bootstrap(&db);
    let (step, live) = challenge(&db, "admin1", PASSWORD);
    let SignInStep::MfaEnrollmentRequired { enrollment, .. } = step else {
        panic!("expected enrollment");
    };
    let signed_in = accepted(complete(&db, live, code(&db, &enrollment.secret_base32)).unwrap());
    let (session_id, workstation_id): (SessionId, WorkstationId) = connection(&db)
        .query_row(
            "SELECT id, workstation_id FROM auth_sessions WHERE user_id = ?1",
            [user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let actor = Principal::user(
        user_id,
        session_id,
        workstation_id,
        signed_in.capabilities(),
    );
    (db, user_id, session_id, actor)
}

#[test]
fn security_key_policy_change_accepts_fresh_password_and_preserves_the_session() {
    let (db, user_id, session_id, actor) = signed_in_administrator();
    let mut input = policy_input(
        None,
        (15, 5, 15),
        Some("Synthetic lab approved optional key verification"),
    );
    input.allow_security_key_touch_only = true;
    input.password = Some(SecretString::from("wrong password").into());
    assert!(matches!(
        write(&db, &actor, move |ctx| lis_auth::set_security_policy(
            ctx, input
        ))
        .unwrap(),
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    assert_eq!(
        db.db
            .read_blocking(lis_auth::get_security_policy)
            .unwrap()
            .version,
        None
    );
    let mut input = policy_input(
        None,
        (15, 5, 15),
        Some("Synthetic lab approved optional key verification"),
    );
    input.allow_security_key_touch_only = true;
    input.password = Some(SecretString::from(PASSWORD).into());
    assert!(matches!(
        write(&db, &actor, move |ctx| lis_auth::set_security_policy(
            ctx, input
        ))
        .unwrap(),
        Attempt::Accepted(_)
    ));
    assert!(lis_auth::session_is_open(&connection(&db), session_id, user_id).unwrap());
    let mut input = policy_input(
        Some(1),
        (15, 5, 15),
        Some("Return to mandatory key verification"),
    );
    input.password = None;
    assert!(matches!(
        write(&db, &actor, move |ctx| lis_auth::set_security_policy(
            ctx, input
        ))
        .unwrap(),
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    assert!(
        db.db
            .read_blocking(lis_auth::current_security_policy)
            .unwrap()
            .allow_security_key_touch_only
    );
}

#[test]
fn personal_pin_keeps_password_confirmation_and_is_removed_by_administrative_reset() {
    let (db, user_id, session_id, actor) = signed_in_administrator();
    accepted(
        write(&db, &actor, |ctx| {
            lis_auth::set_unlock_pin(
                ctx,
                &DataKey::from_bytes([42; 32]),
                lis_auth::SetUnlockPinInput {
                    pin: SecretString::from("482905"),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
        })
        .unwrap(),
    );
    assert_eq!(
        sign_in(&db, "admin1", "482905").unwrap(),
        Some(Rejection::InvalidCredentials)
    );
    assert!(matches!(
        write(&db, &actor, |ctx| lis_auth::reauthenticate(
            ctx,
            &SecretString::from("482905").into(),
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    assert!(matches!(
        write(&db, &actor, |ctx| lis_auth::unlock_with_pin(
            ctx,
            &DataKey::from_bytes([42; 32]),
            &SecretString::from("482906")
        ))
        .unwrap(),
        Attempt::Rejected(Rejection::WrongUnlockPin)
    ));
    accepted(
        write(&db, &actor, |ctx| {
            lis_auth::unlock(ctx, &SecretString::from(PASSWORD))
        })
        .unwrap(),
    );
    assert_eq!(
        lis_auth::get_unlock_pin_status(&connection(&db), user_id, session_id)
            .unwrap()
            .attempts_remaining,
        4
    );
    let administrator = create_user(&db, &actor, "otheradmin", &[RoleCode::Administrator]);
    let other = user_principal(
        administrator.user_id,
        RoleCode::Administrator.capabilities(),
    );
    accepted(reset(&db, &other, &current_user(&db, user_id), PASSWORD).unwrap());
    assert_eq!(
        connection(&db)
            .query_row("SELECT count(*) FROM auth_unlock_pins", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(matches!(
        write(&db, &actor, |ctx| lis_auth::unlock_with_pin(
            ctx,
            &DataKey::from_bytes([42; 32]),
            &SecretString::from("482905")
        )),
        Err(AuthError::SessionUnavailable)
    ));
    assert!(audit_actions(&db).contains(&(
        "auth.unlock_pin_disabled".to_owned(),
        "succeeded".to_owned()
    )));
}
