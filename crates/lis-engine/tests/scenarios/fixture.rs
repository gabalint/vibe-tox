//! A started engine on a temporary data directory with a fixed clock and a
//! fixed data key, plus sign-in helpers. Synthetic accounts only.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use lis_auth::{
    ChangeInitialPasswordInput, CreateFirstAdministratorInput, CreateUserInput, DataKey, RoleCode,
    SignInInput, SignInStep, UserSummary, VerifyMfaInput,
};
use lis_core::{Clock, CommandError, ErrorCode, FixedClock, UserId, UtcTimestamp};
use lis_engine::{
    Engine, EngineConfig, InstallationKind, KeyProvider, PrinterDestinations, SessionInfo,
};
use lis_store::DATABASE_FILE;
use rusqlite::Connection;
use rusqlite::types::Value;
use secrecy::SecretString;
use tempfile::TempDir;

pub(crate) const ADMIN: &str = "admin1";
pub(crate) const PASSWORD: &str = "synthetic passphrase 1";
pub(crate) const NEW_PASSWORD: &str = "synthetic passphrase 2";
pub(crate) const WEBVIEW: &str = "TestWebView 1.0";

pub(crate) fn secret(text: &str) -> SecretString {
    SecretString::from(text)
}

pub(crate) fn start_time() -> UtcTimestamp {
    UtcTimestamp::parse("2026-10-12T08:00:00.000000000Z").unwrap()
}

pub(crate) struct TestEngine {
    pub(crate) engine: Engine,
    pub(crate) clock: Arc<FixedClock>,
    dir: TempDir,
    kind: InstallationKind,
}

impl TestEngine {
    pub(crate) async fn start() -> Self {
        Self::start_as(InstallationKind::Laboratory).await
    }

    /// A laboratory or demo installation (proposed REQ-OPS-016).
    pub(crate) async fn start_as(kind: InstallationKind) -> Self {
        let clock = Arc::new(FixedClock::new(start_time()));
        Self::start_with_clock(kind, clock.clone(), clock).await
    }

    /// Injects an observing clock while retaining the fixed clock used by helpers.
    pub(crate) async fn start_with_clock(
        kind: InstallationKind,
        clock: Arc<FixedClock>,
        engine_clock: Arc<dyn Clock>,
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Self::try_engine(&dir, engine_clock, kind).await.unwrap();
        Self {
            engine,
            clock,
            dir,
            kind,
        }
    }

    /// Starts another engine on this data folder as `kind`, after this one is dropped.
    pub(crate) async fn reopen_as(
        self,
        kind: InstallationKind,
    ) -> Result<Engine, lis_engine::EngineError> {
        let Self { engine, dir, .. } = self;
        drop(engine);
        let clock = Arc::new(FixedClock::new(start_time()));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            match Self::try_engine(&dir, clock.clone(), kind).await {
                Err(error) if error.code() == "already_open" => {
                    assert!(tokio::time::Instant::now() < deadline, "lock not released");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                started => return started,
            }
        }
    }

    async fn try_engine(
        dir: &TempDir,
        clock: Arc<dyn Clock>,
        kind: InstallationKind,
    ) -> Result<Engine, lis_engine::EngineError> {
        Engine::start(EngineConfig {
            data_dir: dir.path().to_owned(),
            clock,
            keys: KeyProvider::Fixed(DataKey::from_bytes([7; 32])),
            printer_destinations: PrinterDestinations::LabNetworkAndLoopback,
            webview: Some(WEBVIEW.to_owned()),
            outbox_handlers: Vec::new(),
            installation_kind: kind,
        })
        .await
    }

    /// Stops without shutdown, as a crash would, and starts again.
    pub(crate) async fn crash_and_restart(self) -> Self {
        let Self {
            engine,
            clock,
            dir,
            kind,
        } = self;
        drop(engine);
        // Dropping the engine stops its dispatcher task, which releases the
        // database lock once it runs; a crashed process releases it at once.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(engine) = Self::try_engine(&dir, clock.clone(), kind).await {
                return Self {
                    engine,
                    clock,
                    dir,
                    kind,
                };
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the database lock was not released within 10 seconds"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub(crate) fn advance(&self, seconds: u64) {
        self.clock.set(
            self.clock
                .now()
                .checked_add(Duration::from_secs(seconds))
                .unwrap(),
        );
    }

    /// The data folder.
    pub(crate) fn data_dir(&self) -> &Path {
        self.dir.path()
    }

    pub(crate) fn conn(&self) -> Connection {
        Connection::open(self.dir.path().join(DATABASE_FILE)).unwrap()
    }

    /// `(action, outcome)` of every event of `stream`, in order.
    pub(crate) fn events(&self, stream: &str) -> Vec<(String, String)> {
        self.conn()
            .prepare("SELECT action, outcome FROM audit_events WHERE stream = ?1 ORDER BY seq")
            .unwrap()
            .query_map([stream], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Every row of every table, as text, for "nothing else changed" checks.
    pub(crate) fn fingerprint(&self) -> Vec<String> {
        let conn = self.conn();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut rows = Vec::new();
        for table in tables {
            let mut statement = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
            let columns = statement.column_count();
            let mut query = statement.query([]).unwrap();
            while let Some(row) = query.next().unwrap() {
                let values: Vec<Value> = (0..columns).map(|i| row.get(i).unwrap()).collect();
                rows.push(format!("{table}: {values:?}"));
            }
        }
        rows
    }

    pub(crate) async fn bootstrap(&self) {
        self.engine
            .create_first_administrator(CreateFirstAdministratorInput {
                username: ADMIN.into(),
                display_name: "Test Administrator".into(),
                password: secret(PASSWORD),
                one_person: false,
                load_demo_data: None,
            })
            .await
            .unwrap();
    }

    pub(crate) async fn sign_in(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SignInStep, CommandError> {
        self.engine
            .sign_in(SignInInput {
                username: username.into(),
                password: secret(password),
            })
            .await
    }

    pub(crate) async fn verify(
        &self,
        step: &SignInStep,
        code: &str,
    ) -> Result<SessionInfo, CommandError> {
        self.engine
            .verify_mfa(VerifyMfaInput {
                challenge_id: challenge_id(step),
                code: secret(code),
            })
            .await
    }

    /// The current code of `secret_base32`.
    pub(crate) fn code(&self, secret_base32: &str) -> String {
        let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
        let totp = totp_rs::Builder::new().with_secret(secret).build().unwrap();
        let seconds = u64::try_from(self.clock.now().unix_seconds()).unwrap();
        totp.generate(seconds).to_string()
    }

    /// Completes a sign-in whose next step is enrolment; returns the session and the secret.
    pub(crate) async fn enrol(&self, step: SignInStep) -> (SessionInfo, String) {
        let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
            panic!("expected enrolment, got {step:?}");
        };
        let secret = enrollment.secret_base32.clone();
        let session = self.verify(&step, &self.code(&secret)).await.unwrap();
        (session, secret)
    }

    /// Signs in with an enrolled authenticator, half a minute after the last code.
    pub(crate) async fn sign_in_with(
        &self,
        username: &str,
        password: &str,
        secret: &str,
    ) -> SessionInfo {
        self.advance(30);
        let step = self.sign_in(username, password).await.unwrap();
        assert!(matches!(step, SignInStep::MfaRequired { .. }), "{step:?}");
        self.verify(&step, &self.code(secret)).await.unwrap()
    }

    /// Bootstraps and signs the first administrator in; returns their TOTP secret.
    pub(crate) async fn admin_session(&self) -> String {
        self.bootstrap().await;
        let step = self.sign_in(ADMIN, PASSWORD).await.unwrap();
        self.enrol(step).await.1
    }

    /// Creates an account as the signed-in administrator. Granting a clinical
    /// role needs a quality manager (REQ-IDN-004), whom most scenarios do not
    /// sign in; as test setup only, the roles are granted in the database.
    pub(crate) async fn create_user(&self, username: &str, roles: &[RoleCode]) -> UserSummary {
        let user = self
            .engine
            .create_user(CreateUserInput {
                username: username.into(),
                display_name: format!("Test {username}"),
                roles: Vec::new(),
                temporary_password: secret(PASSWORD),
                password: secret(PASSWORD).into(),
            })
            .await
            .unwrap();
        let conn = self.conn();
        for role in roles {
            conn.execute(
                "INSERT INTO auth_role_grants (id, user_id, role, granted_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                // The grant ID is any UUIDv7 text.
                rusqlite::params![UserId::generate(), user.user_id, role, self.clock.now()],
            )
            .unwrap();
        }
        let users = self.engine.list_users().await.unwrap();
        users
            .into_iter()
            .find(|u| u.user_id == user.user_id)
            .unwrap()
    }

    /// First sign-in of a created user: replace the temporary password, enrol.
    pub(crate) async fn first_sign_in(&self, username: &str) -> (SessionInfo, String) {
        let step = self.sign_in(username, PASSWORD).await.unwrap();
        let next = self
            .engine
            .change_initial_password(ChangeInitialPasswordInput {
                challenge_id: challenge_id(&step),
                new_password: secret(NEW_PASSWORD),
            })
            .await
            .unwrap();
        self.enrol(next).await
    }
}

pub(crate) fn challenge_id(step: &SignInStep) -> lis_auth::ChallengeId {
    match step {
        SignInStep::PasswordChangeRequired { challenge_id }
        | SignInStep::MfaEnrollmentRequired { challenge_id, .. }
        | SignInStep::MfaRequired { challenge_id } => *challenge_id,
    }
}

pub(crate) fn code_of<T: std::fmt::Debug>(result: Result<T, CommandError>) -> ErrorCode {
    result.unwrap_err().code()
}
