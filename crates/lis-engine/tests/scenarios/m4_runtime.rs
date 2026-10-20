//! Runtime operations (REQ-OPS-006, REQ-OPS-008, REQ-OPS-013, NFR-19, NFR-24; M4 design
//! brief §3.9, §6, §7, §8): installation-key states, the pre-migration snapshot, the close
//! warning and its override record, settings and security-policy versions, the session
//! policy (idle timeout from the policy, absolute lifetime, disabled account), health and
//! update records. Synthetic data only.

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::time::Duration;

use lis_auth::{
    ChangeInitialPasswordInput, CreateFirstAdministratorInput, CreateUserInput,
    DEFAULT_SEPARATION_POLICY, DataKey, RoleCode, SessionUser, SetSecurityPolicyInput,
    SetSeparationPolicyInput, SignInInput, SignInStep, VerifyMfaInput,
};
use lis_core::{Clock, CommandError, ErrorCode, FixedClock, UserId};
use lis_engine::{
    AcceptInstalledVersionInput, AcknowledgeListeningGapInput, AttestHealthCheckInput,
    DisposalMethod, Engine, EngineConfig, HealthState, KeyProvider, KeyState, ListAppRunsInput,
    ListHealthHistoryInput, ListWorkQueueItemsInput, MemoryKeyStore, OperationsSettings,
    RecordMediaDisposalInput, RecordUpdateApprovalInput, RegisterInstrument,
    RestoreInstallationKeyInput, SessionInfo, SetOperationsSettingsInput, VersionStatus,
    WithdrawUpdateApprovalInput,
};
use lis_interfaces::{ConnectionSettings, ListenerState};
use lis_ops::health::CheckCode;
use lis_ops::policy::DEFAULT_SETTINGS;
use lis_store::{DATABASE_FILE, PRE_MIGRATION_DIR};
use rusqlite::Connection;
use secrecy::SecretString;
use tempfile::TempDir;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, challenge_id, code_of, secret, start_time};

/// Calls the runtime command `name` with plausible input; `None` for other areas.
pub(crate) async fn runtime_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    match health_call(engine, name).await {
        Some(result) => Some(result),
        None => records_call(engine, name).await,
    }
}

/// Health, runs, settings, the security policy and the separation-of-duties policy.
async fn health_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "get_host_health" => engine.get_host_health().await.map(drop),
        "refresh_host_health" => engine.refresh_host_health().await.map(drop),
        "list_health_history" => engine
            .list_health_history(ListHealthHistoryInput {
                before_event_id: None,
                check: None,
                limit: 10,
            })
            .await
            .map(drop),
        "attest_health_check" => engine
            .attest_health_check(AttestHealthCheckInput {
                check: CheckCode::Ups,
                note: "Synthetic UPS check".to_owned(),
            })
            .await
            .map(drop),
        "list_app_runs" => engine
            .list_app_runs(ListAppRunsInput {
                run_id: None,
                before_run_id: None,
                limit: 10,
            })
            .await
            .map(drop),
        "acknowledge_listening_gap" => engine
            .acknowledge_listening_gap(AcknowledgeListeningGapInput {
                run_id: lis_ops::runs::AppRunId::generate(),
                note: "Synthetic acknowledgement".to_owned(),
            })
            .await
            .map(drop),
        "get_operations_settings" => engine.get_operations_settings().await.map(drop),
        "set_operations_settings" => engine
            .set_operations_settings(settings_input(None, DEFAULT_SETTINGS, None))
            .await
            .map(drop),
        "get_security_policy" => engine.get_security_policy().await.map(drop),
        "set_security_policy" => engine
            .set_security_policy(policy_input(None, 10, None))
            .await
            .map(drop),
        "get_separation_policy" => engine.get_separation_policy().await.map(drop),
        "set_separation_policy" => engine
            .set_separation_policy(SetSeparationPolicyInput {
                expected_version: None,
                values: DEFAULT_SEPARATION_POLICY,
                reason: "Synthetic policy".to_owned(),
                password: secret(PASSWORD).into(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// Update records, disposals and the installation key.
async fn records_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_installed_versions" => engine.list_installed_versions().await.map(drop),
        "list_update_approvals" => engine.list_update_approvals().await.map(drop),
        "record_update_approval" => engine
            .record_update_approval(approval_input("9.9.9", PASSWORD), None)
            .await
            .map(drop),
        "withdraw_update_approval" => engine
            .withdraw_update_approval(WithdrawUpdateApprovalInput {
                approval_id: lis_ops::updates::UpdateApprovalId::generate(),
                expected_revision: 1,
                reason: "Synthetic withdrawal".to_owned(),
            })
            .await
            .map(drop),
        "accept_installed_version" => engine
            .accept_installed_version(acceptance(env!("CARGO_PKG_VERSION"), PASSWORD))
            .await
            .map(drop),
        "record_media_disposal" => engine.record_media_disposal(disposal()).await.map(drop),
        "list_media_disposals" => engine.list_media_disposals().await.map(drop),
        "get_installation_key_state" => engine.get_installation_key_state().await.map(drop),
        "restore_installation_key" => engine
            .restore_installation_key(RestoreInstallationKeyInput {
                installation_key: SecretString::from("vtk1-0000"),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn settings_input(
    expected_version: Option<i64>,
    values: OperationsSettings,
    reason: Option<&str>,
) -> SetOperationsSettingsInput {
    SetOperationsSettingsInput {
        expected_version,
        values,
        reason: reason.map(str::to_owned),
    }
}

fn policy_input(
    expected_version: Option<i64>,
    idle_timeout_minutes: u32,
    reason: Option<&str>,
) -> SetSecurityPolicyInput {
    SetSecurityPolicyInput {
        expected_version,
        idle_timeout_minutes,
        lockout_failures: 5,
        lockout_minutes: 15,
        allow_security_key_touch_only: false,
        password: None,
        reason: reason.map(str::to_owned),
    }
}

fn approval_input(version: &str, password: &str) -> RecordUpdateApprovalInput {
    RecordUpdateApprovalInput {
        version: version.to_owned(),
        risk_class_text: "Minor (as published)".to_owned(),
        release_notes_reviewed: true,
        pre_update_run_id: lis_ops::backup::RunId::generate(),
        notes: "Synthetic approval notes".to_owned(),
        password: secret(password).into(),
    }
}

fn acceptance(version: &str, password: &str) -> AcceptInstalledVersionInput {
    AcceptInstalledVersionInput {
        version: version.to_owned(),
        note: "Synthetic acceptance checks".to_owned(),
        password: secret(password).into(),
    }
}

fn disposal() -> RecordMediaDisposalInput {
    RecordMediaDisposalInput {
        medium: "Synthetic backup drive A".to_owned(),
        method: DisposalMethod::Wiped,
        identifier: Some("SYN-0001".to_owned()),
        note: "Wiped with the synthetic procedure".to_owned(),
    }
}

/// A data folder and clock that outlive the engines started on them.
pub(crate) struct Host {
    pub(crate) dir: TempDir,
    pub(crate) clock: Arc<FixedClock>,
}

impl Host {
    pub(crate) fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            clock: Arc::new(FixedClock::new(start_time())),
        }
    }

    pub(crate) async fn start(&self, keys: KeyProvider) -> Engine {
        Engine::start(EngineConfig {
            data_dir: self.dir.path().to_owned(),
            clock: self.clock.clone(),
            keys,
            printer_destinations: lis_engine::PrinterDestinations::LabNetwork,
            webview: Some("TestWebView 1.0".to_owned()),
            outbox_handlers: Vec::new(),
            installation_kind: lis_engine::InstallationKind::Laboratory,
        })
        .await
        .unwrap()
    }

    pub(crate) fn advance(&self, seconds: u64) {
        self.clock.set(
            self.clock
                .now()
                .checked_add(Duration::from_secs(seconds))
                .unwrap(),
        );
    }

    pub(crate) fn conn(&self) -> Connection {
        Connection::open(self.dir.path().join(DATABASE_FILE)).unwrap()
    }

    /// `(action, outcome)` of every security event, in order.
    pub(crate) fn security_events(&self) -> Vec<(String, String)> {
        self.conn()
            .prepare(
                "SELECT action, outcome FROM audit_events WHERE stream = 'security' ORDER BY seq",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// The current code of `secret_base32`.
    pub(crate) fn code(&self, secret_base32: &str) -> String {
        let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
        let totp = totp_rs::Builder::new().with_secret(secret).build().unwrap();
        let seconds = u64::try_from(self.clock.now().unix_seconds()).unwrap();
        totp.generate(seconds).to_string()
    }

    /// Creates the first administrator and enrols their authenticator; returns the secret.
    pub(crate) async fn enrol_admin(&self, engine: &Engine) -> String {
        engine
            .create_first_administrator(CreateFirstAdministratorInput {
                username: ADMIN.into(),
                display_name: "Test Administrator".into(),
                password: secret(PASSWORD),
                one_person: false,
                load_demo_data: None,
            })
            .await
            .unwrap();
        let step = sign_in(engine).await.unwrap();
        let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
            panic!("expected enrolment, got {step:?}");
        };
        let totp = enrollment.secret_base32.clone();
        verify(engine, &step, &self.code(&totp)).await.unwrap();
        totp
    }

    /// Creates a user as the signed-in administrator, whose own password is
    /// `actor_password`, and completes the user's first sign-in (new password, enrolment);
    /// the window's session becomes theirs. Returns their TOTP secret.
    pub(crate) async fn add_user(
        &self,
        engine: &Engine,
        username: &str,
        roles: &[RoleCode],
        actor_password: &str,
    ) -> String {
        engine
            .create_user(CreateUserInput {
                username: username.into(),
                display_name: format!("Test {username}"),
                roles: roles.to_vec(),
                temporary_password: secret(PASSWORD),
                password: secret(actor_password).into(),
            })
            .await
            .unwrap();
        let step = engine
            .sign_in(SignInInput {
                username: username.into(),
                password: secret(PASSWORD),
            })
            .await
            .unwrap();
        let step = engine
            .change_initial_password(ChangeInitialPasswordInput {
                challenge_id: challenge_id(&step),
                new_password: secret(NEW_PASSWORD),
            })
            .await
            .unwrap();
        let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
            panic!("expected enrolment, got {step:?}");
        };
        let totp = enrollment.secret_base32.clone();
        verify(engine, &step, &self.code(&totp)).await.unwrap();
        totp
    }

    /// Signs the administrator in with their enrolled authenticator.
    pub(crate) async fn admin_sign_in(
        &self,
        engine: &Engine,
        totp: &str,
    ) -> Result<SessionInfo, CommandError> {
        self.advance(30);
        let step = sign_in(engine).await.unwrap();
        assert!(matches!(step, SignInStep::MfaRequired { .. }), "{step:?}");
        verify(engine, &step, &self.code(totp)).await
    }
}

async fn sign_in(engine: &Engine) -> Result<SignInStep, CommandError> {
    engine
        .sign_in(SignInInput {
            username: ADMIN.into(),
            password: secret(PASSWORD),
        })
        .await
}

async fn verify(
    engine: &Engine,
    step: &SignInStep,
    code: &str,
) -> Result<SessionInfo, CommandError> {
    engine
        .verify_mfa(VerifyMfaInput {
            challenge_id: challenge_id(step),
            code: secret(code),
        })
        .await
}

/// The error as the UI receives it, for its field errors and gate reasons.
pub(crate) fn json(error: &CommandError) -> serde_json::Value {
    serde_json::to_value(error).unwrap()
}

pub(crate) fn key(byte: u8) -> DataKey {
    DataKey::from_bytes([byte; 32])
}

pub(crate) fn key_line(key: &DataKey) -> SecretString {
    SecretString::from(lis_ops::kit::key_text(key).as_str().to_owned())
}

pub(crate) async fn key_state(engine: &Engine) -> KeyState {
    engine.get_installation_key_state().await.unwrap().key_state
}

#[tokio::test]
async fn a_new_installation_creates_its_key_once() {
    let host = Host::new();
    let store = MemoryKeyStore::new(None);
    let engine = host.start(KeyProvider::Memory(store.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Ok);
    let created = store
        .key()
        .expect("a key for a database without sealed secrets");
    let totp = host.enrol_admin(&engine).await;
    engine.shutdown().await;
    drop(engine);

    let engine = host.start(KeyProvider::Memory(store.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Ok);
    assert_eq!(
        store.key().unwrap().expose(),
        created.expose(),
        "never replaced"
    );
    host.admin_sign_in(&engine, &totp).await.unwrap();
}

#[tokio::test]
async fn a_missing_key_is_never_replaced_and_is_restored_from_the_kit() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let totp = host.enrol_admin(&engine).await;
    engine.shutdown().await;
    drop(engine);

    // The OS key store lost its entry, as on a rebuilt host.
    let store = MemoryKeyStore::new(None);
    let engine = host.start(KeyProvider::Memory(store.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Missing);
    assert!(store.key().is_none(), "no new key over sealed secrets");

    // Listeners and work without sign-in continue; TOTP sign-in is refused, typed.
    host.advance(30);
    let step = sign_in(&engine).await.unwrap();
    let refused = verify(&engine, &step, &host.code(&totp)).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Unavailable);
    assert_eq!(
        json(&refused)["gateReasons"][0]["code"],
        "installation_key_missing"
    );

    // A malformed key and another installation's key are refused, and nothing is stored.
    let malformed = engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: SecretString::from("vtk1-not-a-key"),
        })
        .await
        .unwrap_err();
    assert_eq!(malformed.code(), ErrorCode::Validation);
    assert_eq!(json(&malformed)["fieldErrors"][0]["code"], "invalid_format");
    let foreign = engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: key_line(&key(2)),
        })
        .await
        .unwrap_err();
    assert_eq!(json(&foreign)["fieldErrors"][0]["code"], "does_not_open");
    assert!(store.key().is_none());

    let restored = engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: key_line(&key(1)),
        })
        .await
        .unwrap();
    assert_eq!(restored.key_state, KeyState::Ok);
    assert_eq!(store.key().unwrap().expose(), key(1).expose());
    host.admin_sign_in(&engine, &totp).await.unwrap();
    let again = engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: key_line(&key(1)),
        })
        .await;
    assert_eq!(code_of(again), ErrorCode::Conflict);
    let restores = host
        .security_events()
        .into_iter()
        .filter(|(action, _)| action == "auth.restore_installation_key")
        .count();
    assert_eq!(restores, 1);
}

#[tokio::test]
async fn a_mismatched_key_refuses_sign_in_until_the_right_key_is_restored() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let totp = host.enrol_admin(&engine).await;
    engine.shutdown().await;
    drop(engine);

    let store = MemoryKeyStore::new(Some(key(3)));
    let engine = host.start(KeyProvider::Memory(store.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Mismatch);
    assert_eq!(
        store.key().unwrap().expose(),
        key(3).expose(),
        "left as found"
    );
    host.advance(30);
    let step = sign_in(&engine).await.unwrap();
    let refused = verify(&engine, &step, &host.code(&totp)).await.unwrap_err();
    assert_eq!(
        json(&refused)["gateReasons"][0]["code"],
        "installation_key_mismatch"
    );
    engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: key_line(&key(1)),
        })
        .await
        .unwrap();
    host.admin_sign_in(&engine, &totp).await.unwrap();
}

#[tokio::test]
async fn pending_migrations_on_existing_data_keep_a_pre_migration_snapshot() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    engine.shutdown().await;
    drop(engine);
    // Simulate an older database: the anchors module was not migrated yet.
    let conn = host.conn();
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP TABLE ops_anchor_export_ranges;
         DROP TABLE ops_anchor_exports;
         DELETE FROM store_migrations WHERE module = 'ops_anchors';",
    )
    .unwrap();
    drop(conn);

    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let snapshots: Vec<_> = std::fs::read_dir(host.dir.path().join(PRE_MIGRATION_DIR))
        .unwrap()
        .collect();
    assert_eq!(
        snapshots.len(),
        1,
        "one integrity-checked copy before migrating"
    );
    engine.shutdown().await;
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn wait_for_listening(engine: &Engine) {
    for _ in 0..200 {
        let status = engine.list_listener_status().await.unwrap();
        if status
            .first()
            .is_some_and(|s| s.state == ListenerState::Listening)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the listener did not start listening");
}

/// `(stop_kind, stopped)` of every app run, oldest first.
fn stops(host: &Host) -> Vec<(Option<String>, bool)> {
    host.conn()
        .prepare(
            "SELECT stop_kind, stopped_at IS NOT NULL FROM ops_app_runs ORDER BY started_at, id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[tokio::test]
async fn quitting_with_an_active_listener_warns_and_records_the_override() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    assert!(engine.quit_warnings().await.is_empty(), "nothing pending");
    host.enrol_admin(&engine).await;
    engine
        .register_instrument(RegisterInstrument {
            code: "SYN-ANALYZER".to_owned(),
            name: "Synthetic analyzer".to_owned(),
            connection: ConnectionSettings::AstmTcp {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: free_port(),
                allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            },
            enabled: true,
        })
        .await
        .unwrap();
    wait_for_listening(&engine).await;
    let warnings = engine.quit_warnings().await;
    assert_eq!(warnings.0, ["Synthetic analyzer is listening for results."]);

    engine.record_quit_with_warnings().await;
    engine.shutdown().await;
    assert_eq!(
        stops(&host),
        [(Some("quit_with_warnings".to_owned()), true)]
    );
    let stop_events: Vec<(String, String)> = host
        .conn()
        .prepare("SELECT principal_kind, details FROM audit_events WHERE action = 'ops.app_stop'")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(stop_events.len(), 1, "recorded once");
    assert_eq!(stop_events[0].0, "user", "attributed to the signed-in user");
    assert!(stop_events[0].1.contains("quit_with_warnings"));
}

#[tokio::test]
async fn a_clean_shutdown_records_one_stop_by_the_sign_in_screen() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    engine.shutdown().await;
    engine.shutdown().await;
    assert_eq!(stops(&host), [(Some("clean".to_owned()), true)]);
    let principal: String = host
        .conn()
        .query_row(
            "SELECT principal_id FROM audit_events WHERE action = 'ops.app_stop'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(principal, "sign-in-screen");
}

#[tokio::test]
async fn settings_and_policy_are_versions_with_reasons() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    let first = engine
        .set_operations_settings(settings_input(None, DEFAULT_SETTINGS, None))
        .await
        .unwrap();
    assert_eq!(first.version, Some(1));
    let changed = OperationsSettings {
        backup_interval_minutes: 30,
        ..DEFAULT_SETTINGS
    };
    let without_reason = engine
        .set_operations_settings(settings_input(Some(1), changed, None))
        .await
        .unwrap_err();
    assert_eq!(json(&without_reason)["fieldErrors"][0]["field"], "reason");
    let stale = engine
        .set_operations_settings(settings_input(None, changed, Some("Slower disk")))
        .await;
    assert_eq!(code_of(stale), ErrorCode::Conflict);
    let second = engine
        .set_operations_settings(settings_input(Some(1), changed, Some("Slower disk")))
        .await
        .unwrap();
    assert_eq!(
        (second.version, second.values.backup_interval_minutes),
        (Some(2), 30)
    );

    let policy = engine
        .set_security_policy(policy_input(None, 10, None))
        .await
        .unwrap();
    assert_eq!(policy.version, Some(1));
    let refused = engine
        .set_security_policy(policy_input(Some(1), 61, Some("Too long")))
        .await
        .unwrap_err();
    assert_eq!(
        json(&refused)["fieldErrors"][0]["field"],
        "idleTimeoutMinutes"
    );
    let actions: Vec<String> = host
        .security_events()
        .into_iter()
        .map(|(action, _)| action)
        .filter(|action| action == "ops.set_settings" || action == "auth.set_security_policy")
        .collect();
    assert_eq!(
        actions,
        [
            "ops.set_settings",
            "ops.set_settings",
            "auth.set_security_policy"
        ]
    );
}

#[tokio::test]
async fn the_idle_timeout_follows_the_security_policy() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    engine
        .set_security_policy(policy_input(None, 5, None))
        .await
        .unwrap();
    host.advance(5 * 60 - 1);
    engine.touch_session().await.unwrap();
    host.advance(5 * 60);
    assert_eq!(
        code_of(engine.touch_session().await),
        ErrorCode::SessionLocked
    );
}

#[tokio::test]
async fn a_session_ends_after_its_absolute_lifetime_whatever_the_activity() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    let steps = lis_engine::ABSOLUTE_SESSION_LIFETIME.as_secs() / 600;
    for _ in 1..steps {
        host.advance(600);
        engine.touch_session().await.unwrap();
    }
    host.advance(600);
    assert_eq!(
        code_of(engine.touch_session().await),
        ErrorCode::Unauthenticated
    );
    assert_eq!(engine.current_session().await.unwrap(), None);
    assert_ended(&host, "expired");
}

#[tokio::test]
async fn a_disabled_users_open_session_ends_at_the_next_command() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    // Simulates a change made outside this session, as no window can disable the account
    // that holds it.
    let user: SessionUser = lis_auth::session_user(
        &host.conn(),
        engine.current_session().await.unwrap().unwrap().user_id,
    )
    .unwrap();
    host.conn()
        .execute(
            "UPDATE auth_users SET status = 'disabled' WHERE id = ?1",
            [user.user_id],
        )
        .unwrap();
    assert_eq!(
        code_of(engine.get_host_health().await),
        ErrorCode::Unauthenticated
    );
    assert_eq!(engine.current_session().await.unwrap(), None);
    assert_ended(&host, "account_disabled");
}

/// A session ended with `reason`, stored and in its `auth.session_end` event, and none
/// is left open.
fn assert_ended(host: &Host, reason: &str) {
    let conn = host.conn();
    let details: String = conn
        .query_row(
            "SELECT e.details FROM audit_events e JOIN auth_sessions s ON s.id = e.record_id \
             WHERE e.action = 'auth.session_end' AND s.end_reason = ?1",
            [reason],
            |row| row.get(0),
        )
        .unwrap();
    assert!(details.contains(reason), "{details}");
    let open: i64 = conn
        .query_row(
            "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(open, 0);
}

#[tokio::test]
async fn health_reports_every_check_rate_limits_refresh_and_feeds_the_queue() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.enrol_admin(&engine).await;
    let health = engine.get_host_health().await.unwrap();
    assert_eq!(health.checks.len(), lis_ops::health::ALL_CHECKS.len());
    let check = |code: CheckCode| {
        health
            .checks
            .iter()
            .find(|check| check.check == code)
            .unwrap()
            .clone()
    };
    assert_eq!(check(CheckCode::BackupConfigured).state, HealthState::Alert);
    assert_eq!(check(CheckCode::CloudServices).state, HealthState::Ok);
    assert_eq!(health.overall, HealthState::Alert);
    assert_eq!(health.keep_awake.state.as_str(), "not_needed");
    assert!(
        engine
            .list_health_history(ListHealthHistoryInput {
                before_event_id: None,
                check: None,
                limit: 50
            })
            .await
            .unwrap()
            .is_empty(),
        "reads record nothing"
    );

    engine.refresh_host_health().await.unwrap();
    let too_soon = engine.refresh_host_health().await.unwrap_err();
    assert_eq!(too_soon.code(), ErrorCode::Conflict);
    assert_eq!(
        json(&too_soon)["gateReasons"][0]["code"],
        "refresh_rate_limited"
    );
    let history = engine
        .list_health_history(ListHealthHistoryInput {
            before_event_id: None,
            check: Some(CheckCode::BackupConfigured),
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    let alerts = engine
        .list_work_queue_items(ListWorkQueueItemsInput {
            after: None,
            queue: "ops_alerts".to_owned(),
            limit: 100,
        })
        .await
        .unwrap()
        .items;
    assert!(
        alerts
            .iter()
            .any(|item| item.item_id == "backup_configured")
    );

    let attested = engine
        .attest_health_check(AttestHealthCheckInput {
            check: CheckCode::Ups,
            note: "Synthetic UPS on mains".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(attested.state, HealthState::Attested);
    let not_attestable = engine
        .attest_health_check(AttestHealthCheckInput {
            check: CheckCode::BackupAge,
            note: "Not allowed".to_owned(),
        })
        .await;
    assert_eq!(code_of(not_attestable), ErrorCode::Validation);
}

#[tokio::test]
async fn update_records_need_their_capability_a_backup_and_fresh_authentication() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let admin_totp = host.enrol_admin(&engine).await;
    let versions = engine.list_installed_versions().await.unwrap();
    assert_eq!(versions.len(), 1);
    assert!(versions[0].running);
    // Administrator approval still requires fresh authentication.
    let unauthenticated = engine
        .accept_installed_version(acceptance(&versions[0].version, "wrong passphrase"))
        .await;
    assert_eq!(code_of(unauthenticated), ErrorCode::Validation);
    let dialog_cancelled = engine
        .record_update_approval(approval_input("9.9.9", PASSWORD), None)
        .await
        .unwrap();
    assert_eq!(dialog_cancelled, None);
    let package = host.dir.path().join("synthetic-update-9.9.9.pkg");
    std::fs::write(&package, b"synthetic package bytes").unwrap();
    let without_backup = engine
        .record_update_approval(approval_input("9.9.9", PASSWORD), Some(package.clone()))
        .await;
    assert_eq!(code_of(without_backup), ErrorCode::Conflict);
    let wrong_password = engine
        .record_update_approval(approval_input("9.9.9", "wrong passphrase"), Some(package))
        .await;
    assert_eq!(code_of(wrong_password), ErrorCode::Validation);
    let disposal = engine.record_media_disposal(disposal()).await.unwrap();
    assert_eq!(disposal.method, DisposalMethod::Wiped);
    assert_eq!(engine.list_media_disposals().await.unwrap().len(), 1);

    host.add_user(&engine, "quality1", &[RoleCode::QualityManager], PASSWORD)
        .await;
    let version = versions[0].version.clone();
    let forbidden = engine
        .accept_installed_version(acceptance(&version, NEW_PASSWORD))
        .await;
    assert_eq!(code_of(forbidden), ErrorCode::Forbidden);
    host.admin_sign_in(&engine, &admin_totp).await.unwrap();
    host.add_user(&engine, "config2", &[RoleCode::Administrator], PASSWORD)
        .await;
    let wrong = engine
        .accept_installed_version(acceptance(&version, PASSWORD))
        .await;
    assert_eq!(
        code_of(wrong),
        ErrorCode::Validation,
        "fresh authentication"
    );
    let accepted = engine
        .accept_installed_version(acceptance(&version, NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(accepted.status, VersionStatus::Accepted);
    let again = engine
        .accept_installed_version(acceptance(&version, NEW_PASSWORD))
        .await;
    assert_eq!(code_of(again), ErrorCode::Conflict);
}

/// The shell's setup hook starts the background loops from the main thread, outside any
/// runtime context (smoke test of 3d7b54c: "there is no reactor running"). The engine
/// spawns them on the runtime it started on.
#[test]
fn the_loops_start_from_a_thread_outside_the_runtime() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let t = runtime.block_on(crate::fixture::TestEngine::start());
    t.engine.start_operations(lis_engine::Inhibitor::new(|| {
        Ok(Box::new(()) as lis_engine::Guard)
    }));
    runtime.block_on(t.engine.shutdown());
}
