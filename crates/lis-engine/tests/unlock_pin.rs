//! Local PINs unlock an existing session; they never replace full sign-in or
//! sensitive-action authentication (REQ-IDN-004, NFR-19). Synthetic data only.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "the shared scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/fixture.rs"]
mod fixture;
#[path = "fixtures/security_key_client.rs"]
mod security_key_client;

use fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of, secret};
use lis_auth::{
    CreateUserInput, DisableUnlockPinInput, ResetUserCredentialsInput, SetUnlockPinInput,
    UnlockSessionInput, UnlockWithPinInput,
};
use lis_core::{Clock, CommandError, ErrorCode, FixedClock, UtcTimestamp};
use lis_engine::{
    KeyRecoveryAcknowledgement, KeySignInChoicesInput, SecurityKeyAction, SecurityKeyActionInput,
    SecurityKeyRecoveryInput, SecurityKeySignInInput, SecurityKeySignupInput,
};
use security_key_client::TestKey;
use std::cell::Cell;
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

const PIN: &str = "846291";
const OTHER_PIN: &str = "573820";

async fn set_pin(t: &TestEngine, pin: &str, password: &str) -> Result<(), CommandError> {
    t.engine
        .set_unlock_pin(SetUnlockPinInput {
            pin: secret(pin),
            password: secret(password).into(),
        })
        .await
}

async fn unlock(t: &TestEngine, pin: &str) -> Result<lis_engine::SessionInfo, CommandError> {
    t.engine
        .unlock_with_pin(UnlockWithPinInput { pin: secret(pin) })
        .await
}

async fn assert_remaining(t: &TestEngine, expected: u32) {
    let status = t.engine.get_unlock_pin_status().await.unwrap();
    assert!(status.enabled);
    assert_eq!(u32::from(status.attempts_remaining), expected);
}

fn event_count(t: &TestEngine, action: &str, outcome: &str) -> usize {
    t.events("security")
        .iter()
        .filter(|(a, o)| a == action && o == outcome)
        .count()
}

#[tokio::test]
async fn pin_is_disabled_initially_and_cannot_start_or_resume_a_session_after_restart() {
    let t = TestEngine::start().await;
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Unauthenticated);
    let totp = t.admin_session().await;
    let initial = t.engine.get_unlock_pin_status().await.unwrap();
    assert!(!initial.enabled);
    assert_eq!(initial.attempts_remaining, 0);
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    assert!(
        unlock(&t, PIN).await.is_err(),
        "PIN requires a locked session"
    );

    let t = t.crash_and_restart().await;
    assert!(t.engine.current_session().await.unwrap().is_none());
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Unauthenticated);
    t.sign_in_with(ADMIN, PASSWORD, &totp).await;
    assert_remaining(&t, 5).await;
    t.engine.lock_session().await.unwrap();
    assert_eq!(unlock(&t, PIN).await.unwrap().username, ADMIN);
    t.engine.sign_out().await.unwrap();
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Unauthenticated);
}

#[tokio::test]
async fn pin_configuration_requires_fresh_full_authentication_and_exactly_six_ascii_digits() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    for malformed in ["12345", "1234567", "12a456", "１２３４５６", "12345\n"] {
        assert_eq!(
            code_of(set_pin(&t, malformed, PASSWORD).await),
            ErrorCode::Validation
        );
    }
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    assert!(set_pin(&t, PIN, "incorrect full password").await.is_err());
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    assert_eq!(event_count(&t, "auth.unlock_pin_set", "succeeded"), 1);
    assert!(
        t.engine
            .disable_unlock_pin(DisableUnlockPinInput {
                password: secret(PIN).into()
            })
            .await
            .is_err()
    );
    assert_remaining(&t, 5).await;
    t.engine.lock_session().await.unwrap();
    assert_eq!(
        code_of(set_pin(&t, OTHER_PIN, PASSWORD).await),
        ErrorCode::SessionLocked
    );
    assert_eq!(
        code_of(
            t.engine
                .disable_unlock_pin(DisableUnlockPinInput {
                    password: secret(PASSWORD).into(),
                })
                .await
        ),
        ErrorCode::SessionLocked
    );
    unlock(&t, PIN).await.unwrap();
    t.engine
        .disable_unlock_pin(DisableUnlockPinInput {
            password: secret(PASSWORD).into(),
        })
        .await
        .unwrap();
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    assert_eq!(event_count(&t, "auth.unlock_pin_disabled", "succeeded"), 1);
    t.engine.lock_session().await.unwrap();
    assert!(unlock(&t, PIN).await.is_err());
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
}

#[tokio::test]
async fn correct_pin_unlocks_the_same_session_resets_attempts_and_does_not_confirm_sensitive_actions()
 {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let before: String = t
        .conn()
        .query_row(
            "SELECT id FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    t.engine.lock_session().await.unwrap();
    assert_eq!(code_of(unlock(&t, OTHER_PIN).await), ErrorCode::Validation);
    assert_remaining(&t, 4).await;
    let session = unlock(&t, PIN).await.unwrap();
    assert!(!session.locked);
    assert_eq!(session.username, ADMIN);
    assert_remaining(&t, 5).await;
    let after: String = t
        .conn()
        .query_row(
            "SELECT id FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "succeeded"), 1);
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "failed"), 1);
    let account = |confirmation: &str| CreateUserInput {
        username: "pin_confirmed_user".into(),
        display_name: "Synthetic staff".into(),
        roles: Vec::new(),
        temporary_password: secret(PASSWORD),
        password: secret(confirmation).into(),
    };
    assert!(t.engine.create_user(account(PIN)).await.is_err());
    assert_eq!(t.engine.list_users().await.unwrap().len(), 1);
    t.engine.create_user(account(PASSWORD)).await.unwrap();
}

#[tokio::test]
async fn malformed_attempts_count_and_password_unlock_or_repeated_lock_cannot_reset_the_limit() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    t.engine.lock_session().await.unwrap();
    for (attempt, remaining) in [(OTHER_PIN, 4), ("", 3), ("not six digits", 2), ("12345", 1)] {
        assert_eq!(code_of(unlock(&t, attempt).await), ErrorCode::Validation);
        assert_remaining(&t, remaining).await;
        t.engine.lock_session().await.unwrap();
        assert_remaining(&t, remaining).await;
    }
    t.engine
        .unlock_session(UnlockSessionInput {
            password: secret(PASSWORD),
        })
        .await
        .unwrap();
    assert_remaining(&t, 1).await;
    t.engine.lock_session().await.unwrap();
    assert_eq!(
        code_of(unlock(&t, "１２３４５６").await),
        ErrorCode::Unauthenticated
    );
    assert!(t.engine.current_session().await.unwrap().is_none());
    let open: i64 = t
        .conn()
        .query_row(
            "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        open, 0,
        "the fifth failed attempt must commit session termination"
    );
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "failed"), 5);
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Unauthenticated);
    t.sign_in_with(ADMIN, PASSWORD, &totp).await;
    assert_remaining(&t, 5).await;
    t.engine.lock_session().await.unwrap();
    unlock(&t, PIN).await.unwrap();
}

#[tokio::test]
async fn pin_cannot_unlock_another_persons_session_or_an_expired_session() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    t.create_user("other_person", &[]).await;
    t.engine.lock_session().await.unwrap();
    t.first_sign_in("other_person").await;
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    set_pin(&t, OTHER_PIN, NEW_PASSWORD).await.unwrap();
    t.engine.lock_session().await.unwrap();
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Validation);
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
    assert_eq!(
        unlock(&t, OTHER_PIN).await.unwrap().username,
        "other_person"
    );
    t.engine.lock_session().await.unwrap();
    t.advance(12 * 60 * 60);
    assert_eq!(
        code_of(unlock(&t, OTHER_PIN).await),
        ErrorCode::Unauthenticated
    );
    assert!(t.engine.current_session().await.unwrap().is_none());
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "succeeded"), 1);
}

#[tokio::test]
async fn administrator_credential_reset_removes_the_target_users_pin() {
    let t = TestEngine::start().await;
    let totp = t.admin_session().await;
    t.create_user("reset_target", &[]).await;
    t.first_sign_in("reset_target").await;
    set_pin(&t, PIN, NEW_PASSWORD).await.unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &totp).await;
    let target = t
        .engine
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|user| user.username == "reset_target")
        .unwrap();
    t.engine
        .reset_user_credentials(ResetUserCredentialsInput {
            user_id: target.user_id,
            temporary_password: secret(PASSWORD),
            reset_mfa: true,
            expected_revision: target.revision,
            reason: "Synthetic credential loss".into(),
            password: secret(PASSWORD).into(),
        })
        .await
        .unwrap();
    t.first_sign_in("reset_target").await;
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    t.engine.lock_session().await.unwrap();
    assert!(unlock(&t, PIN).await.is_err());
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
}

async fn key_session(t: &TestEngine, key: &TestKey) -> String {
    let recovery = t
        .engine
        .security_key_signup(
            SecurityKeySignupInput {
                username: ADMIN.into(),
                display_name: "Synthetic key user".into(),
                one_person: false,
                load_demo_data: None,
                key_label: "Synthetic everyday key".into(),
            },
            key,
        )
        .await
        .unwrap();
    t.engine
        .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
            recovery_code: secret(&recovery.recovery_code),
        })
        .await
        .unwrap();
    recovery.recovery_code
}

async fn key_configure(t: &TestEngine, key: &TestKey, action: SecurityKeyAction) {
    let credential_id = t
        .engine
        .get_authentication_status()
        .await
        .unwrap()
        .credentials[0]
        .credential_id;
    t.engine
        .security_key_action(
            SecurityKeyActionInput {
                credential_id,
                action,
            },
            None,
            key,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn key_accounts_configure_pin_through_fresh_key_confirmation_without_password_fallback() {
    let t = TestEngine::start().await;
    let key = TestKey::new();
    key_session(&t, &key).await;
    assert!(set_pin(&t, PIN, PASSWORD).await.is_err());
    key_configure(
        &t,
        &key,
        SecurityKeyAction::SetUnlockPin(SetUnlockPinInput {
            pin: secret(PIN),
            password: secret("unused native confirmation placeholder").into(),
        }),
    )
    .await;
    t.engine.lock_session().await.unwrap();
    assert_eq!(unlock(&t, PIN).await.unwrap().username, ADMIN);
    assert!(
        t.engine
            .disable_unlock_pin(DisableUnlockPinInput {
                password: secret(PIN).into(),
            })
            .await
            .is_err()
    );
    assert_remaining(&t, 5).await;
    key_configure(
        &t,
        &key,
        SecurityKeyAction::DisableUnlockPin(DisableUnlockPinInput {
            password: secret("unused native confirmation placeholder").into(),
        }),
    )
    .await;
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
}

#[tokio::test]
async fn offline_key_recovery_ends_the_session_and_removes_its_users_pin() {
    let t = TestEngine::start().await;
    let key = TestKey::new();
    let recovery_code = key_session(&t, &key).await;
    key_configure(
        &t,
        &key,
        SecurityKeyAction::SetUnlockPin(SetUnlockPinInput {
            pin: secret(PIN),
            password: secret("unused native confirmation placeholder").into(),
        }),
    )
    .await;
    let replacement = TestKey::new();
    t.engine
        .security_key_recover(
            SecurityKeyRecoveryInput {
                username: ADMIN.into(),
                recovery_code: secret(&recovery_code),
                key_label: "Synthetic replacement key".into(),
            },
            &replacement,
        )
        .await
        .unwrap();
    assert_eq!(code_of(unlock(&t, PIN).await), ErrorCode::Unauthenticated);
    let credential_id = t
        .engine
        .list_key_sign_in_choices(KeySignInChoicesInput {
            username: ADMIN.into(),
        })
        .await
        .unwrap()[0]
        .credential_id;
    t.engine
        .security_key_sign_in(
            SecurityKeySignInInput {
                username: ADMIN.into(),
                credential_id,
            },
            &replacement,
        )
        .await
        .unwrap();
    assert!(!t.engine.get_unlock_pin_status().await.unwrap().enabled);
    t.engine.lock_session().await.unwrap();
    assert!(unlock(&t, PIN).await.is_err());
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
}

#[tokio::test]
async fn exhausted_attempt_and_session_end_roll_back_if_the_security_audit_cannot_commit() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    t.engine.lock_session().await.unwrap();
    for _ in 0..4 {
        assert_eq!(code_of(unlock(&t, OTHER_PIN).await), ErrorCode::Validation);
    }
    t.conn()
        .execute_batch(
            "CREATE TRIGGER fail_pin_audit BEFORE INSERT ON audit_events
         WHEN NEW.action = 'auth.unlock_with_pin'
          AND json_extract(NEW.details, '$.reason') = 'attempts_exhausted'
         BEGIN SELECT RAISE(ABORT, 'synthetic audit failure'); END;",
        )
        .unwrap();
    assert_eq!(code_of(unlock(&t, OTHER_PIN).await), ErrorCode::Internal);
    assert_remaining(&t, 1).await;
    assert!(t.engine.current_session().await.unwrap().unwrap().locked);
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "failed"), 4);
    t.conn()
        .execute_batch("DROP TRIGGER fail_pin_audit")
        .unwrap();
    assert_eq!(
        code_of(unlock(&t, OTHER_PIN).await),
        ErrorCode::Unauthenticated
    );
    assert!(t.engine.current_session().await.unwrap().is_none());
    assert_eq!(event_count(&t, "auth.unlock_with_pin", "failed"), 5);
}

/// With the database writer held externally, exactly one of these commands owns
/// the engine authentication gate. The other's Conflict proves the surviving
/// command captured its generation before we change session state.
async fn pending_pin(
    t: &TestEngine,
) -> impl Future<Output = Result<lis_engine::SessionInfo, CommandError>> + '_ {
    let mut first = Box::pin(unlock(t, PIN));
    let mut second = Box::pin(unlock(t, PIN));
    tokio::select! {
        result = first.as_mut() => {
            assert_eq!(code_of(result), ErrorCode::Conflict);
            second
        }
        result = second.as_mut() => {
            assert_eq!(code_of(result), ErrorCode::Conflict);
            first
        }
        () = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("one competing unlock must be refused before writer release");
        }
    }
}

#[tokio::test]
async fn queued_pin_cannot_overwrite_a_repeated_lock_or_cross_absolute_session_expiry() {
    for expire in [false, true] {
        let t = TestEngine::start().await;
        t.admin_session().await;
        set_pin(&t, PIN, PASSWORD).await.unwrap();
        t.engine.lock_session().await.unwrap();
        let held_writer = t.conn();
        held_writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        let pending = pending_pin(&t).await;
        if expire {
            t.advance(12 * 60 * 60);
        } else {
            t.engine.lock_session().await.unwrap();
        }
        held_writer.execute_batch("ROLLBACK").unwrap();
        assert!(pending.await.is_err());
        let session = t.engine.current_session().await.unwrap();
        if expire {
            assert!(session.is_none());
        } else {
            assert!(session.unwrap().locked);
        }
    }
}

#[tokio::test]
async fn queued_pin_cannot_restore_the_session_after_a_sign_out_request() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    t.engine.lock_session().await.unwrap();
    let held_writer = t.conn();
    held_writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let pending = pending_pin(&t).await;
    let sign_out = t.engine.sign_out();
    tokio::pin!(sign_out);
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = &mut sign_out => {
                result.unwrap();
                panic!("sign-out audit must wait for writer release");
            }
            () = async {
                while t.engine.current_session().await.unwrap().is_some() {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
    })
    .await
    .expect("sign-out must clear the window before its write commits");
    held_writer.execute_batch("ROLLBACK").unwrap();
    let (unlocked, signed_out) = tokio::join!(pending, sign_out);
    assert!(unlocked.is_err());
    signed_out.unwrap();
    assert!(t.engine.current_session().await.unwrap().is_none());
}

#[tokio::test]
async fn queued_old_session_pin_cannot_replace_a_new_users_full_sign_in() {
    let t = TestEngine::start().await;
    let admin_totp = t.admin_session().await;
    t.create_user("switch_target", &[]).await;
    let (_, other_totp) = t.first_sign_in("switch_target").await;
    t.sign_in_with(ADMIN, PASSWORD, &admin_totp).await;
    set_pin(&t, PIN, PASSWORD).await.unwrap();
    let challenge = t.sign_in("switch_target", NEW_PASSWORD).await.unwrap();
    let code = t.code(&other_totp);
    t.engine.lock_session().await.unwrap();
    let held_writer = t.conn();
    held_writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let sign_in = t.verify(&challenge, &code);
    tokio::pin!(sign_in);
    // The public MFA command submits its transaction before its first pending
    // await; poll it once so replacement precedes the queued old-session unlock.
    tokio::select! {
        biased;
        result = &mut sign_in => {
            result.unwrap();
            panic!("MFA commit must wait for writer release");
        }
        () = std::future::ready(()) => {}
    }
    let pending = pending_pin(&t).await;
    held_writer.execute_batch("ROLLBACK").unwrap();
    let (signed_in, old_pin) = tokio::join!(sign_in, pending);
    assert_eq!(signed_in.unwrap().username, "switch_target");
    assert!(old_pin.is_err());
    assert_eq!(
        t.engine.current_session().await.unwrap().unwrap().username,
        "switch_target"
    );
}

thread_local! {
    static POLLING_CONFIGURATION: Cell<bool> = const { Cell::new(false) };
}

struct ConfigurationClock {
    fixed: Arc<FixedClock>,
    calls: AtomicUsize,
    captured: tokio::sync::Notify,
}

impl Clock for ConfigurationClock {
    fn now(&self) -> UtcTimestamp {
        // Only this command's poll counts: background tasks and database threads
        // cannot signal the boundary. Its first read is session lookup, its
        // second creates the configuration guard immediately before writer submit.
        if POLLING_CONFIGURATION.get() && self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            self.captured.notify_one();
        }
        self.fixed.now()
    }
}

async fn pending_configuration<'a>(
    clock: &'a ConfigurationClock,
    configuration: impl Future<Output = Result<(), CommandError>> + 'a,
) -> impl Future<Output = Result<(), CommandError>> + 'a {
    let mut configuration = Box::pin(configuration);
    let mut observed = Box::pin(std::future::poll_fn(move |cx| {
        POLLING_CONFIGURATION.set(true);
        let result = configuration.as_mut().poll(cx);
        POLLING_CONFIGURATION.set(false);
        result
    }));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = observed.as_mut() => {
                result.unwrap();
                panic!("configuration must wait for the held writer");
            }
            () = clock.captured.notified() => {}
        }
    })
    .await
    .expect("configuration guard must be captured before interruption");
    observed
}

#[derive(Clone, Copy)]
enum ConfigurationInterruption {
    Lock,
    SignOut,
    Expire,
}

async fn interrupt_configuration(
    t: &TestEngine,
    held_writer: &rusqlite::Connection,
    pending: impl Future<Output = Result<(), CommandError>>,
    interruption: ConfigurationInterruption,
) {
    if matches!(interruption, ConfigurationInterruption::Expire) {
        t.advance(60);
        held_writer.execute_batch("ROLLBACK").unwrap();
        assert!(pending.await.is_err());
        return;
    }
    let transition = async {
        match interruption {
            ConfigurationInterruption::SignOut => t.engine.sign_out().await,
            _ => t.engine.lock_session().await,
        }
    };
    tokio::pin!(transition);
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = &mut transition => {
                result.unwrap();
                panic!("transition audit must wait for writer release");
            }
            () = async {
                while t.engine.current_session().await.unwrap().is_some_and(|s| !s.locked) {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
    })
    .await
    .expect("window state must change before the queued configuration commits");
    held_writer.execute_batch("ROLLBACK").unwrap();
    let (configured, transitioned) = tokio::join!(pending, transition);
    assert!(configured.is_err());
    transitioned.unwrap();
}

async fn configuration_case(disable: bool, interruption: ConfigurationInterruption) {
    let fixed = Arc::new(FixedClock::new(fixture::start_time()));
    let clock = Arc::new(ConfigurationClock {
        fixed: fixed.clone(),
        calls: AtomicUsize::new(0),
        captured: tokio::sync::Notify::new(),
    });
    let t = TestEngine::start_with_clock(
        lis_engine::InstallationKind::Laboratory,
        fixed,
        clock.clone(),
    )
    .await;
    t.admin_session().await;
    if disable {
        set_pin(&t, PIN, PASSWORD).await.unwrap();
    }
    if matches!(interruption, ConfigurationInterruption::Expire) {
        t.advance(12 * 60 * 60 - 60);
        t.engine
            .unlock_session(UnlockSessionInput {
                password: secret(PASSWORD),
            })
            .await
            .unwrap();
    }
    let audit_before = event_count(&t, "auth.unlock_pin_set", "succeeded")
        + event_count(&t, "auth.unlock_pin_disabled", "succeeded");
    let held_writer = t.conn();
    held_writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    let pending = pending_configuration(&clock, async {
        if disable {
            t.engine
                .disable_unlock_pin(DisableUnlockPinInput {
                    password: secret(PASSWORD).into(),
                })
                .await
        } else {
            set_pin(&t, PIN, PASSWORD).await
        }
    })
    .await;
    interrupt_configuration(&t, &held_writer, pending, interruption).await;
    let rows: i64 = t
        .conn()
        .query_row("SELECT count(*) FROM auth_unlock_pins", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, i64::from(disable));
    let audit_after = event_count(&t, "auth.unlock_pin_set", "succeeded")
        + event_count(&t, "auth.unlock_pin_disabled", "succeeded");
    assert_eq!(
        audit_before, audit_after,
        "refused settings must leave no success audit"
    );
}

#[tokio::test]
async fn queued_pin_settings_roll_back_after_lock_sign_out_or_absolute_expiry_without_observation()
{
    for disable in [false, true] {
        for interruption in [
            ConfigurationInterruption::Lock,
            ConfigurationInterruption::SignOut,
            ConfigurationInterruption::Expire,
        ] {
            configuration_case(disable, interruption).await;
        }
    }
}
