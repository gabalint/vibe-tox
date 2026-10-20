//! Synthetic native callbacks exercise the trusted ceremony and command boundary.
//! The software key is test-only and deliberately simulates authenticator UV.
#![cfg(test)]
#[path = "security_keys/policy.rs"]
mod policy;
#[path = "fixtures/security_key_client.rs"]
mod security_key_client;

use lis_auth::{
    AuthenticationMethod, CreateFirstAdministratorInput, CreateUserInput, DataKey, SecurityKeyId,
    SignInInput, SignInStep, VerifyMfaInput,
};
use lis_core::{CommandError, CorrelationId, ErrorCode, FixedClock, UtcTimestamp};
use lis_engine::{
    Engine, EngineConfig, InstallationKind, KeyProvider, KeyRecoveryAcknowledgement,
    KeySignInChoicesInput, PrinterDestinations, SecurityKeyAction, SecurityKeyActionInput,
    SecurityKeyClient, SecurityKeyEnrollInput, SecurityKeyRecoveryInput, SecurityKeySignInInput,
    SecurityKeySignupInput, SecurityKeyUnlockInput,
};
use secrecy::SecretString;
use security_key_client::TestKey;
use serde_json::Value;
use std::sync::{Arc, Mutex};

const USER: &str = "key_operator";
const PASSWORD: &str = "synthetic passphrase 1";
fn secret(value: &str) -> SecretString {
    SecretString::from(value)
}
fn cancelled() -> CommandError {
    CommandError::new(
        ErrorCode::Conflict,
        "Synthetic cancellation",
        CorrelationId::generate(),
    )
}

struct Cancel;
impl SecurityKeyClient for Cancel {
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        Err(cancelled())
    }
    async fn authenticate(&self, _: String, _: Value) -> Result<Value, CommandError> {
        Err(cancelled())
    }
}
struct NeverCalled;
impl SecurityKeyClient for NeverCalled {
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("unauthorized native registration")
    }
    async fn authenticate(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("unauthorized native assertion")
    }
}

struct Fixture {
    engine: Engine,
    dir: tempfile::TempDir,
    key: TestKey,
    clock: Arc<FixedClock>,
}
impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(FixedClock::new(
            UtcTimestamp::parse("2026-10-15T08:00:00.000000000Z").unwrap(),
        ));
        let engine = start_engine(&dir, &clock).await;
        Self {
            engine,
            dir,
            key: TestKey::new(),
            clock,
        }
    }
    fn signup() -> SecurityKeySignupInput {
        SecurityKeySignupInput {
            username: USER.to_owned(),
            display_name: "Synthetic Operator".to_owned(),
            one_person: true,
            load_demo_data: None,
            key_label: "Everyday key".to_owned(),
        }
    }
    async fn signed_in() -> (Self, String) {
        let f = Self::new().await;
        let recovery = f
            .engine
            .security_key_signup(Self::signup(), &f.key)
            .await
            .unwrap()
            .recovery_code;
        f.engine
            .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
                recovery_code: secret(&recovery),
            })
            .await
            .unwrap();
        (f, recovery)
    }
    fn conn(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.dir.path().join(lis_store::DATABASE_FILE)).unwrap()
    }
    async fn key_id(&self) -> SecurityKeyId {
        self.engine
            .list_key_sign_in_choices(KeySignInChoicesInput {
                username: USER.to_owned(),
            })
            .await
            .unwrap()[0]
            .credential_id
    }
    fn create_action(id: SecurityKeyId) -> SecurityKeyActionInput {
        SecurityKeyActionInput {
            credential_id: id,
            action: SecurityKeyAction::CreateUser(CreateUserInput {
                username: "frozen_account".to_owned(),
                display_name: "Frozen Name".to_owned(),
                roles: vec![],
                temporary_password: secret(PASSWORD),
                password: secret("ignored placeholder").into(),
            }),
        }
    }
    async fn password_session(&self) {
        self.engine
            .create_first_administrator(CreateFirstAdministratorInput {
                username: USER.to_owned(),
                display_name: "Synthetic Operator".to_owned(),
                password: secret(PASSWORD),
                one_person: false,
                load_demo_data: None,
            })
            .await
            .unwrap();
        let step = self
            .engine
            .sign_in(SignInInput {
                username: USER.to_owned(),
                password: secret(PASSWORD),
            })
            .await
            .unwrap();
        let SignInStep::MfaEnrollmentRequired {
            challenge_id,
            enrollment,
        } = step
        else {
            panic!("expected enrollment")
        };
        let totp = totp_rs::Builder::new()
            .with_secret(totp_rs::Secret::try_from_base32(&enrollment.secret_base32).unwrap())
            .build()
            .unwrap();
        let now = UtcTimestamp::parse("2026-10-15T08:00:00.000000000Z")
            .unwrap()
            .unix_seconds();
        self.engine
            .verify_mfa(VerifyMfaInput {
                challenge_id,
                code: secret(&totp.generate(u64::try_from(now).unwrap()).to_string()),
            })
            .await
            .unwrap();
    }
}

async fn start_engine(dir: &tempfile::TempDir, clock: &Arc<FixedClock>) -> Engine {
    Engine::start(EngineConfig {
        data_dir: dir.path().to_owned(),
        clock: clock.clone(),
        keys: KeyProvider::Fixed(DataKey::from_bytes([7; 32])),
        printer_destinations: PrinterDestinations::LabNetworkAndLoopback,
        webview: None,
        outbox_handlers: Vec::new(),
        installation_kind: InstallationKind::Laboratory,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn signup_requires_recovery_acknowledgement_then_activates_without_second_assertion() {
    let f = Fixture::new().await;
    let result = f
        .engine
        .security_key_signup(Fixture::signup(), &f.key)
        .await
        .unwrap();
    assert!(f.engine.current_session().await.unwrap().is_none());
    let session = f
        .engine
        .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
            recovery_code: secret(&result.recovery_code),
        })
        .await
        .unwrap();
    assert_eq!(session.username, USER);
    assert!(!session.locked);
    assert!(
        f.engine
            .get_authentication_status()
            .await
            .unwrap()
            .recovery_available
    );
    assert!(
        f.engine
            .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
                recovery_code: secret(&result.recovery_code)
            })
            .await
            .is_err()
    );
    let assertions: i64 = f
        .conn()
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action = 'auth.key_assertion'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(assertions, 0);
}

#[tokio::test]
async fn sign_in_lock_and_unlock_use_the_same_account_key() {
    let (f, _) = Fixture::signed_in().await;
    let id = f.key_id().await;
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
    f.engine.lock_session().await.unwrap();
    assert_eq!(
        f.engine.list_users().await.unwrap_err().code(),
        ErrorCode::SessionLocked
    );
    let unlocked = f
        .engine
        .security_key_unlock(SecurityKeyUnlockInput { credential_id: id }, &f.key)
        .await
        .unwrap();
    assert!(!unlocked.locked);
    assert_eq!(unlocked.username, USER);
    assert_eq!(f.engine.list_users().await.unwrap().len(), 1);
}

#[tokio::test]
async fn cancelled_signup_and_migration_leave_credentials_unchanged() {
    let f = Fixture::new().await;
    assert!(
        f.engine
            .security_key_signup(Fixture::signup(), &Cancel)
            .await
            .is_err()
    );
    assert!(f.engine.setup_status().await.unwrap().needs_administrator);
    f.password_session().await;
    let password: String = f
        .conn()
        .query_row("SELECT password_hash FROM auth_users", [], |row| row.get(0))
        .unwrap();
    assert!(
        f.engine
            .security_key_enroll(
                SecurityKeyEnrollInput {
                    key_label: "New key".to_owned(),
                    password: secret(PASSWORD).into(),
                    credential_id: None
                },
                &Cancel
            )
            .await
            .is_err()
    );
    assert_eq!(
        f.engine.get_authentication_status().await.unwrap().method,
        AuthenticationMethod::PasswordTotp
    );
    assert_eq!(
        f.conn()
            .query_row("SELECT password_hash FROM auth_users", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        password
    );
    f.engine
        .security_key_enroll(
            SecurityKeyEnrollInput {
                key_label: "New key".to_owned(),
                password: secret(PASSWORD).into(),
                credential_id: None,
            },
            &f.key,
        )
        .await
        .unwrap();
    assert_eq!(
        f.engine.get_authentication_status().await.unwrap().method,
        AuthenticationMethod::SecurityKey
    );
    assert!(
        f.engine
            .sign_in(SignInInput {
                username: USER.to_owned(),
                password: secret(PASSWORD)
            })
            .await
            .is_err()
    );
    assert_eq!(
        f.conn()
            .query_row(
                "SELECT count(*) FROM auth_users WHERE totp_secret IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn recovery_replaces_key_ends_existing_session_and_requires_key_sign_in() {
    let (f, code) = Fixture::signed_in().await;
    let replacement = TestKey::new();
    let recovered = f
        .engine
        .security_key_recover(
            SecurityKeyRecoveryInput {
                username: USER.to_owned(),
                recovery_code: secret(&code),
                key_label: "Replacement".to_owned(),
            },
            &replacement,
        )
        .await
        .unwrap();
    assert_ne!(recovered.recovery_code, code);
    assert!(f.engine.current_session().await.unwrap().is_none());
    assert!(
        f.engine
            .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
                recovery_code: secret(&recovered.recovery_code)
            })
            .await
            .is_err()
    );
    f.engine
        .security_key_sign_in(
            SecurityKeySignInInput {
                username: USER.to_owned(),
                credential_id: f.key_id().await,
            },
            &replacement,
        )
        .await
        .unwrap();
    assert_eq!(f.engine.list_users().await.unwrap().len(), 1);
}

#[tokio::test]
async fn frozen_action_executes_exact_account_payload_and_never_exposes_proof() {
    let (f, _) = Fixture::signed_in().await;
    let value = f
        .engine
        .security_key_action(Fixture::create_action(f.key_id().await), None, &f.key)
        .await
        .unwrap();
    assert_eq!(value["username"], "frozen_account");
    assert_eq!(value["displayName"], "Frozen Name");
    assert_eq!(value["roles"], serde_json::json!([]));
    assert!(value.get("keyProof").is_none());
    assert_eq!(f.engine.list_users().await.unwrap().len(), 2);
}

#[tokio::test]
async fn unknown_key_and_missing_capability_are_rejected_before_native_callback() {
    let (f, _) = Fixture::signed_in().await;
    assert!(
        f.engine
            .security_key_action(
                Fixture::create_action(SecurityKeyId::generate()),
                None,
                &NeverCalled
            )
            .await
            .is_err()
    );
    f.conn().execute("UPDATE auth_role_grants SET revoked_at = '2026-10-15T08:00:00.000000000Z', revoked_by = user_id WHERE revoked_at IS NULL", []).unwrap();
    assert_eq!(
        f.engine
            .security_key_action(Fixture::create_action(f.key_id().await), None, &NeverCalled)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM auth_users", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[derive(Clone, Copy)]
enum Interruption {
    Lock,
    SignOut,
    RevokeRole,
}
struct Interrupted<'a> {
    fixture: &'a Fixture,
    interruption: Interruption,
}
impl SecurityKeyClient for Interrupted<'_> {
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("not registration")
    }
    async fn authenticate(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        let response = self.fixture.key.authenticate(origin, options).await?;
        match self.interruption {
            Interruption::Lock => self.fixture.engine.lock_session().await.unwrap(),
            Interruption::SignOut => self.fixture.engine.sign_out().await.unwrap(),
            Interruption::RevokeRole => {
                self.fixture.conn().execute("UPDATE auth_role_grants SET revoked_at = '2026-10-15T08:00:00.000000000Z', revoked_by = user_id WHERE revoked_at IS NULL", []).unwrap();
            }
        }
        Ok(response)
    }
}

#[tokio::test]
async fn in_flight_lock_sign_out_or_capability_revocation_refuses_frozen_action() {
    for interruption in [
        Interruption::Lock,
        Interruption::SignOut,
        Interruption::RevokeRole,
    ] {
        let (f, _) = Fixture::signed_in().await;
        let client = Interrupted {
            fixture: &f,
            interruption,
        };
        assert!(
            f.engine
                .security_key_action(Fixture::create_action(f.key_id().await), None, &client)
                .await
                .is_err()
        );
        assert_eq!(
            f.conn()
                .query_row("SELECT count(*) FROM auth_users", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

/// Holds the database writer precisely between the final native boundary and
/// the commit, so a lock can invalidate the already-checked ceremony guard.
struct QueuedCommit<'a> {
    fixture: &'a Fixture,
    held_writer: Mutex<Option<rusqlite::Connection>>,
    ready: tokio::sync::Notify,
}
impl SecurityKeyClient for QueuedCommit<'_> {
    fn finish(&self) -> Result<(), CommandError> {
        let conn = self.fixture.conn();
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        *self.held_writer.lock().unwrap() = Some(conn);
        self.ready.notify_one();
        Ok(())
    }
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("not registration")
    }
    async fn authenticate(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        self.fixture.key.authenticate(origin, options).await
    }
}

async fn queued_authentication(
    client: &QueuedCommit<'_>,
    id: SecurityKeyId,
    unlock: bool,
) -> Result<lis_engine::SessionInfo, CommandError> {
    let engine = &client.fixture.engine;
    if unlock {
        engine
            .security_key_unlock(SecurityKeyUnlockInput { credential_id: id }, client)
            .await
    } else {
        engine
            .security_key_sign_in(
                SecurityKeySignInInput {
                    username: USER.to_owned(),
                    credential_id: id,
                },
                client,
            )
            .await
    }
}

async fn wait_until_locked(engine: &Engine) {
    loop {
        if engine
            .current_session()
            .await
            .unwrap()
            .is_some_and(|session| session.locked)
        {
            break;
        }
        tokio::task::yield_now().await;
    }
}

async fn lock_during_queued_commit(client: &QueuedCommit<'_>) {
    client.ready.notified().await;
    let lock = client.fixture.engine.lock_session();
    tokio::pin!(lock);
    tokio::select! {
        result = &mut lock => { result.unwrap(); panic!("lock audit unexpectedly committed through held writer"); }
        () = wait_until_locked(&client.fixture.engine) => {}
    }
    client
        .held_writer
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .execute_batch("ROLLBACK")
        .unwrap();
    lock.await.unwrap();
}

#[tokio::test]
async fn lock_while_verified_commit_is_queued_cannot_be_overwritten_by_activation() {
    for unlock in [false, true] {
        let (f, _) = Fixture::signed_in().await;
        let id = f.key_id().await;
        let client = QueuedCommit {
            fixture: &f,
            held_writer: Mutex::new(None),
            ready: tokio::sync::Notify::new(),
        };
        let authentication = queued_authentication(&client, id, unlock);
        let lock_during_commit = lock_during_queued_commit(&client);
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(authentication, lock_during_commit)
        })
        .await
        .expect("queued commit race completed");
        assert!(result.is_err());
        assert!(
            f.engine
                .current_session()
                .await
                .unwrap()
                .is_none_or(|session| session.locked)
        );
    }
}

struct AddSpare<'a> {
    current: &'a TestKey,
    spare: &'a TestKey,
}
impl SecurityKeyClient for AddSpare<'_> {
    async fn register(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        self.spare.register(origin, options).await
    }
    async fn authenticate(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        self.current.authenticate(origin, options).await
    }
}

#[tokio::test]
async fn spare_enrollment_rotates_recovery_and_fresh_revocation_ends_the_session() {
    let (f, original_code) = Fixture::signed_in().await;
    let id = f.key_id().await;
    let spare = TestKey::new();
    let enrolled = f
        .engine
        .security_key_enroll(
            SecurityKeyEnrollInput {
                key_label: "Spare key".to_owned(),
                password: secret("unused").into(),
                credential_id: Some(id),
            },
            &AddSpare {
                current: &f.key,
                spare: &spare,
            },
        )
        .await
        .unwrap();
    assert_ne!(enrolled.recovery_code, original_code);
    let status = f.engine.get_authentication_status().await.unwrap();
    assert_eq!(status.credentials.len(), 2);
    let spare = status
        .credentials
        .iter()
        .find(|key| key.label == "Spare key")
        .unwrap();
    f.engine
        .security_key_action(
            SecurityKeyActionInput {
                credential_id: id,
                action: SecurityKeyAction::RevokeSecurityKey(lis_engine::RevokeSecurityKeyInput {
                    credential_id: spare.credential_id,
                    expected_revision: spare.revision,
                    password: secret("unused").into(),
                    reason: "Synthetic spare loss".to_owned(),
                }),
            },
            None,
            &f.key,
        )
        .await
        .unwrap();
    assert!(f.engine.current_session().await.unwrap().is_none());
    let choices = f
        .engine
        .list_key_sign_in_choices(KeySignInChoicesInput {
            username: USER.to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].credential_id, id);
}

/// A SQLite snapshot copied to another directory is intentionally narrower than
/// the encrypted backup/restore workflow or physical-key qualification.
#[tokio::test]
async fn copied_database_preserves_key_rp_but_does_not_resume_the_old_session() {
    let (f, _) = Fixture::signed_in().await;
    let id = f.key_id().await;
    let original_user = f.engine.current_session().await.unwrap().unwrap().user_id;
    f.engine.shutdown().await;
    let copied_dir = tempfile::tempdir().unwrap();
    let copied_path = copied_dir.path().join(lis_store::DATABASE_FILE);
    f.conn()
        .execute("VACUUM INTO ?1", [copied_path.to_str().unwrap()])
        .unwrap();
    let restarted = start_engine(&copied_dir, &f.clock).await;
    assert!(restarted.current_session().await.unwrap().is_none());
    let copied = rusqlite::Connection::open(&copied_path).unwrap();
    assert_eq!(
        copied
            .query_row(
                "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let choices = restarted
        .list_key_sign_in_choices(KeySignInChoicesInput {
            username: USER.to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].credential_id, id);
    let session = restarted
        .security_key_sign_in(
            SecurityKeySignInInput {
                username: USER.to_owned(),
                credential_id: id,
            },
            &f.key,
        )
        .await
        .unwrap();
    assert_eq!(session.user_id, original_user);
    assert!(!session.locked);
    restarted.shutdown().await;
}

struct MovedClock<'a> {
    fixture: &'a Fixture,
    finish_at: &'a str,
}
impl SecurityKeyClient for MovedClock<'_> {
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("not registration")
    }
    async fn authenticate(&self, origin: String, options: Value) -> Result<Value, CommandError> {
        let response = self.fixture.key.authenticate(origin, options).await?;
        self.fixture
            .clock
            .set(UtcTimestamp::parse(self.finish_at).unwrap());
        Ok(response)
    }
}

#[tokio::test]
async fn expired_or_backwards_clock_ceremony_cannot_open_a_session() {
    for finish_at in [
        "2026-10-15T08:02:01.000000000Z",
        "2026-10-15T07:59:59.000000000Z",
    ] {
        let (f, _) = Fixture::signed_in().await;
        let id = f.key_id().await;
        f.engine.sign_out().await.unwrap();
        let result = f
            .engine
            .security_key_sign_in(
                SecurityKeySignInInput {
                    username: USER.to_owned(),
                    credential_id: id,
                },
                &MovedClock {
                    fixture: &f,
                    finish_at,
                },
            )
            .await;
        assert!(result.is_err(), "invalid ceremony time: {finish_at}");
        assert!(f.engine.current_session().await.unwrap().is_none());
        assert_eq!(
            f.conn()
                .query_row(
                    "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
