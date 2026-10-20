//! Module integration tests on a real SQLite file: setup, administration
//! guards, authorization refusals, throttling and append-only grant history.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

#[path = "auth/challenges.rs"]
mod challenges;

use std::time::Duration;

use lis_auth::{
    Attempt, AuthError, ChangeUserStatusInput, CreateFirstAdministratorInput, CreateUserInput,
    DEFAULT_SECURITY_POLICY, DEFAULT_SEPARATION_POLICY, DesignateOnePersonLaboratoryInput,
    EndReason, ReauthPurpose, Rejection, RoleCode, SecurityPolicyView, SeparationPolicy,
    SeparationPolicyView, SetSecurityPolicyInput, SetSeparationPolicyInput, SetUserRolesInput,
    SignInInput, UserStatus, UserSummary,
};
use lis_core::{Capability, CorrelationId, Principal, SessionId, UserId, WorkstationId};
use lis_store::testing::{TestDatabase, test_start};
use lis_store::{CommandContext, DATABASE_FILE, StoreError, lint_schema};
use rusqlite::{Connection, OptionalExtension};
use secrecy::SecretString;

const PASSWORD: &str = "synthetic passphrase 1";

fn open() -> TestDatabase {
    TestDatabase::open(&[lis_auth::SCHEMA]).unwrap()
}

fn write<T, F>(db: &TestDatabase, principal: &Principal, command: F) -> Result<T, AuthError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, AuthError> + Send + 'static,
{
    db.db
        .write_blocking(principal.clone(), CorrelationId::generate(), command)
}

fn sign_in_screen() -> Principal {
    Principal::system("sign-in-screen", [])
}

fn user_principal(id: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        id,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

fn bootstrap(db: &TestDatabase) -> (UserId, Principal) {
    let input = CreateFirstAdministratorInput {
        username: "Admin1".into(),
        display_name: "Test Administrator".into(),
        password: SecretString::from(PASSWORD),
        one_person: false,
        load_demo_data: None,
    };
    let id = write(db, &sign_in_screen(), move |ctx| {
        lis_auth::create_first_administrator(ctx, input)
    })
    .unwrap();
    (
        id,
        user_principal(id, RoleCode::Administrator.capabilities()),
    )
}

/// The first administrator also acting as quality manager: the small
/// laboratory's residual case, which may grant clinical roles.
fn dual_role(admin_id: UserId) -> Principal {
    user_principal(
        admin_id,
        &[Capability::ManageUsers, Capability::ManageQuality],
    )
}

fn accepted<T: std::fmt::Debug>(attempt: Attempt<T>) -> T {
    match attempt {
        Attempt::Accepted(value) => value,
        Attempt::Rejected(rejection) => panic!("refused: {rejection:?}"),
    }
}

fn create_user(
    db: &TestDatabase,
    actor: &Principal,
    username: &str,
    roles: &[RoleCode],
) -> UserSummary {
    let input = CreateUserInput {
        username: username.into(),
        display_name: format!("Test {username}"),
        roles: roles.to_vec(),
        temporary_password: SecretString::from(PASSWORD),
        password: SecretString::from(PASSWORD).into(),
    };
    accepted(write(db, actor, move |ctx| lis_auth::create_user(ctx, input)).unwrap())
}

fn status_input(user: &UserSummary, reason: &str) -> ChangeUserStatusInput {
    ChangeUserStatusInput {
        user_id: user.user_id,
        expected_revision: user.revision,
        reason: reason.into(),
    }
}

fn sign_in(
    db: &TestDatabase,
    username: &str,
    password: &str,
) -> Result<Option<Rejection>, AuthError> {
    let input = SignInInput {
        username: username.into(),
        password: SecretString::from(password),
    };
    let attempt = write(db, &sign_in_screen(), move |ctx| {
        lis_auth::check_password(ctx, &input)
    })?;
    Ok(match attempt {
        Attempt::Accepted(_) => None,
        Attempt::Rejected(rejection) => Some(rejection),
    })
}

fn connection(db: &TestDatabase) -> Connection {
    Connection::open(db.data_dir().join(DATABASE_FILE)).unwrap()
}

fn audit_actions(db: &TestDatabase) -> Vec<(String, String)> {
    connection(db)
        .prepare("SELECT action, outcome FROM audit_events WHERE stream = 'security' ORDER BY seq")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn is_denied(error: &AuthError) -> bool {
    matches!(error, AuthError::Store(StoreError::Denied(_)))
}

#[test]
fn schema_passes_the_lint() {
    assert_eq!(lint_schema(&[lis_auth::SCHEMA]).unwrap(), []);
}

#[test]
fn first_administrator_is_created_only_once() {
    let db = open();
    let status = |db: &TestDatabase| {
        db.db
            .read_blocking(lis_auth::setup_status)
            .unwrap()
            .needs_administrator
    };
    assert!(status(&db));
    let (id, _) = bootstrap(&db);
    assert!(!status(&db));
    let again = CreateFirstAdministratorInput {
        username: "admin2".into(),
        display_name: "Second".into(),
        password: SecretString::from(PASSWORD),
        one_person: false,
        load_demo_data: None,
    };
    let refused = write(&db, &sign_in_screen(), move |ctx| {
        lis_auth::create_first_administrator(ctx, again)
    });
    assert!(matches!(refused, Err(AuthError::AlreadySetUp)));
    let user = db
        .db
        .read_blocking(move |conn| lis_auth::session_user(conn, id))
        .unwrap();
    assert_eq!(user.username, "admin1");
    assert_eq!(user.roles, [RoleCode::Administrator]);
    assert_eq!(
        audit_actions(&db),
        [("auth.first_administrator_create".into(), "succeeded".into())]
    );
}

#[test]
fn administration_commands_refuse_a_principal_without_manage_users() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let target = create_user(&db, &dual_role(admin_id), "analyst1", &[RoleCode::Analyst]);
    let before = audit_actions(&db).len();
    let auditor = user_principal(UserId::generate(), &[Capability::ViewAudit]);
    let id = target.user_id;
    let revision = target.revision;
    let results = [
        write(&db, &auditor, |ctx| {
            lis_auth::create_user(
                ctx,
                CreateUserInput {
                    username: "x-user".into(),
                    display_name: "X".into(),
                    roles: vec![],
                    temporary_password: SecretString::from(PASSWORD),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
            .map(drop)
        }),
        write(&db, &auditor, move |ctx| {
            lis_auth::set_user_roles(
                ctx,
                SetUserRolesInput {
                    user_id: id,
                    roles: vec![RoleCode::Administrator],
                    expected_revision: revision,
                    reason: "test".into(),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
            .map(drop)
        }),
        write(&db, &auditor, move |ctx| {
            lis_auth::disable_user(
                ctx,
                ChangeUserStatusInput {
                    user_id: id,
                    expected_revision: revision,
                    reason: "t".into(),
                },
            )
            .map(drop)
        }),
        write(&db, &auditor, move |ctx| {
            lis_auth::enable_user(
                ctx,
                ChangeUserStatusInput {
                    user_id: id,
                    expected_revision: revision,
                    reason: "t".into(),
                },
            )
            .map(drop)
        }),
        write(&db, &auditor, move |ctx| {
            lis_auth::reset_user_credentials(
                ctx,
                lis_auth::ResetUserCredentialsInput {
                    user_id: id,
                    temporary_password: SecretString::from(PASSWORD),
                    reset_mfa: true,
                    expected_revision: revision,
                    reason: "t".into(),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
            .map(drop)
        }),
    ];
    for result in results {
        assert!(result.as_ref().is_err_and(is_denied), "{result:?}");
    }
    assert_eq!(
        audit_actions(&db).len(),
        before,
        "refused commands write nothing"
    );
}

#[test]
fn last_active_administrator_cannot_be_disabled_or_demoted() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let me = db
        .db
        .read_blocking(move |conn| lis_auth::list_users(conn, test_start()))
        .unwrap();
    let me = me
        .into_iter()
        .find(|user| user.user_id == admin_id)
        .unwrap();
    let disable = status_input(&me, "leaving");
    let refused = write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, disable));
    assert!(matches!(refused, Err(AuthError::LastAdministrator)));
    // Only another administrator could demote this one, so the guard is
    // reached through a principal holding `manage_users` without the role.
    let other = user_principal(UserId::generate(), &[Capability::ManageUsers]);
    let demote = SetUserRolesInput {
        user_id: admin_id,
        roles: vec![RoleCode::Auditor],
        expected_revision: me.revision,
        reason: "rotation".into(),
        password: SecretString::from(PASSWORD).into(),
    };
    let refused = write(&db, &other, move |ctx| {
        lis_auth::set_user_roles(ctx, demote)
    });
    assert!(matches!(refused, Err(AuthError::LastAdministrator)));

    create_user(&db, &admin, "admin2", &[RoleCode::Administrator]);
    let disable = status_input(&me, "leaving");
    let disabled = write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, disable)).unwrap();
    assert_eq!(disabled.status, UserStatus::Disabled);
    assert_eq!(disabled.revision, me.revision + 1);
}

#[test]
fn changes_need_the_current_revision_and_a_reason() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let admin = dual_role(admin_id);
    let user = create_user(&db, &admin, "analyst1", &[RoleCode::Analyst]);
    let mut stale = status_input(&user, "left");
    stale.expected_revision += 1;
    let refused = write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, stale));
    assert!(matches!(refused, Err(AuthError::StaleRevision)));
    let unexplained = status_input(&user, "  ");
    let refused = write(&db, &admin, move |ctx| {
        lis_auth::disable_user(ctx, unexplained)
    })
    .unwrap_err();
    assert_eq!(refused.field_errors()[0].field, "reason");
    let again = status_input(&user, "already active");
    let refused = write(&db, &admin, move |ctx| lis_auth::enable_user(ctx, again));
    assert!(matches!(refused, Err(AuthError::StatusUnchanged)));
}

#[test]
fn create_user_validates_every_field_and_refuses_duplicates() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let invalid = CreateUserInput {
        username: "1x".into(),
        display_name: " ".into(),
        roles: vec![],
        temporary_password: SecretString::from("short"),
        password: SecretString::from(PASSWORD).into(),
    };
    let error = write(&db, &admin, move |ctx| lis_auth::create_user(ctx, invalid)).unwrap_err();
    let fields: Vec<&str> = error
        .field_errors()
        .iter()
        .map(|e| e.field.as_str())
        .collect();
    assert_eq!(fields, ["username", "displayName", "temporaryPassword"]);
    create_user(&db, &admin, "analyst1", &[]);
    let duplicate = CreateUserInput {
        username: "ANALYST1".into(),
        display_name: "Other".into(),
        roles: vec![],
        temporary_password: SecretString::from(PASSWORD),
        password: SecretString::from(PASSWORD).into(),
    };
    let refused = write(&db, &admin, move |ctx| {
        lis_auth::create_user(ctx, duplicate)
    });
    assert!(matches!(refused, Err(AuthError::UsernameTaken)));
}

#[test]
fn unknown_user_wrong_password_and_disabled_account_are_answered_identically() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let admin = dual_role(admin_id);
    let user = create_user(&db, &admin, "analyst1", &[RoleCode::Analyst]);
    create_user(&db, &admin, "admin2", &[RoleCode::Administrator]);
    let disable = status_input(&user, "left the lab");
    write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, disable)).unwrap();
    let invalid = Some(Rejection::InvalidCredentials);
    assert_eq!(sign_in(&db, "nobody", PASSWORD).unwrap(), invalid);
    assert_eq!(sign_in(&db, "admin1", "wrong passphrase").unwrap(), invalid);
    assert_eq!(sign_in(&db, "analyst1", PASSWORD).unwrap(), invalid);
    assert_eq!(sign_in(&db, "x", PASSWORD).unwrap(), invalid);
    assert_eq!(sign_in(&db, " ADMIN1 ", PASSWORD).unwrap(), None);
}

#[test]
fn five_failures_lock_a_username_for_fifteen_minutes_known_or_not() {
    let db = open();
    bootstrap(&db);
    for username in ["admin1", "ghost"] {
        for _ in 0..4 {
            assert_eq!(
                sign_in(&db, username, "wrong passphrase").unwrap(),
                Some(Rejection::InvalidCredentials)
            );
        }
        assert_eq!(
            sign_in(&db, username, "wrong passphrase").unwrap(),
            Some(Rejection::LockedOut)
        );
    }
    assert_eq!(
        sign_in(&db, "admin1", PASSWORD).unwrap(),
        Some(Rejection::LockedOut)
    );
    let locked = db
        .db
        .read_blocking(|conn| lis_auth::list_users(conn, test_start()))
        .unwrap();
    assert!(locked[0].locked_out);
    let lockouts = audit_actions(&db)
        .iter()
        .filter(|(action, _)| action == "auth.lockout")
        .count();
    assert_eq!(lockouts, 2);

    db.clock.set(
        test_start()
            .checked_add(Duration::from_secs(15 * 60))
            .unwrap(),
    );
    assert_eq!(sign_in(&db, "admin1", PASSWORD).unwrap(), None);
    assert_eq!(
        sign_in(&db, "ghost", "wrong passphrase").unwrap(),
        Some(Rejection::InvalidCredentials)
    );
}

#[test]
fn role_grants_are_revoked_never_rewritten_or_deleted() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let admin = dual_role(admin_id);
    let user = create_user(&db, &admin, "analyst1", &[RoleCode::Analyst]);
    let change = SetUserRolesInput {
        user_id: user.user_id,
        roles: vec![RoleCode::TechnicalReviewer],
        expected_revision: user.revision,
        reason: "trained".into(),
        password: SecretString::from(PASSWORD).into(),
    };
    let changed = accepted(
        write(&db, &admin, move |ctx| {
            lis_auth::set_user_roles(ctx, change)
        })
        .unwrap(),
    );
    assert_eq!(changed.roles, [RoleCode::TechnicalReviewer]);
    let conn = connection(&db);
    let history: Vec<(String, bool)> = conn
        .prepare("SELECT role, revoked_at IS NOT NULL FROM auth_role_grants WHERE user_id = ?1 ORDER BY granted_at, role")
        .unwrap()
        .query_map([user.user_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        history,
        [
            ("analyst".into(), true),
            ("technical_reviewer".into(), false)
        ]
    );
    assert!(
        conn.execute("UPDATE auth_role_grants SET role = 'administrator'", [])
            .is_err()
    );
    assert!(conn.execute("DELETE FROM auth_role_grants", []).is_err());
}

fn reauthenticate(
    db: &TestDatabase,
    principal: &Principal,
    password: &str,
) -> Result<Option<Rejection>, AuthError> {
    let password = SecretString::from(password);
    let attempt = write(db, principal, move |ctx| {
        lis_auth::reauthenticate(ctx, &password.into(), ReauthPurpose::ReportRelease)
    })?;
    Ok(match attempt {
        Attempt::Accepted(()) => None,
        Attempt::Rejected(rejection) => Some(rejection),
    })
}

/// `(outcome, purpose detail)` of every `auth.reauthenticate` event.
fn reauth_events(db: &TestDatabase) -> Vec<(String, String)> {
    connection(db)
        .prepare(
            "SELECT outcome, json_extract(details, '$.purpose') FROM audit_events \
             WHERE stream = 'security' AND action = 'auth.reauthenticate' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn reauthentication_checks_the_signed_in_users_password_every_time() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    assert_eq!(reauthenticate(&db, &admin, PASSWORD).unwrap(), None);
    assert_eq!(reauthenticate(&db, &admin, PASSWORD).unwrap(), None);
    let wrong = reauthenticate(&db, &admin, "wrong passphrase").unwrap();
    assert_eq!(wrong, Some(Rejection::WrongPassword));
    let rejection = wrong.unwrap();
    assert_eq!(rejection.code(), lis_core::ErrorCode::Validation);
    assert_eq!(rejection.field_errors()[0].field, "password");
    // The refusal committed its audit event; every event names the purpose.
    let purpose = "report_release".to_owned();
    assert_eq!(
        reauth_events(&db),
        [
            ("succeeded".to_owned(), purpose.clone()),
            ("succeeded".to_owned(), purpose.clone()),
            ("failed".to_owned(), purpose),
        ]
    );
}

#[test]
fn reauthentication_failures_share_the_sign_in_lockout() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    for _ in 0..4 {
        assert_eq!(
            reauthenticate(&db, &admin, "wrong passphrase").unwrap(),
            Some(Rejection::WrongPassword)
        );
    }
    assert_eq!(
        reauthenticate(&db, &admin, "wrong passphrase").unwrap(),
        Some(Rejection::LockedOut)
    );
    // Locked: even the right password is refused, here and at sign-in.
    assert_eq!(
        reauthenticate(&db, &admin, PASSWORD).unwrap(),
        Some(Rejection::LockedOut)
    );
    assert_eq!(
        sign_in(&db, "admin1", PASSWORD).unwrap(),
        Some(Rejection::LockedOut)
    );
    db.clock.set(
        test_start()
            .checked_add(Duration::from_secs(15 * 60))
            .unwrap(),
    );
    assert_eq!(reauthenticate(&db, &admin, PASSWORD).unwrap(), None);
}

#[test]
fn reauthentication_needs_an_active_signed_in_user() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let system = reauthenticate(&db, &Principal::system("worker", []), PASSWORD);
    assert!(matches!(system, Err(AuthError::UserNotFound)));
    let user = create_user(
        &db,
        &dual_role(admin_id),
        "releaser1",
        &[RoleCode::Releaser],
    );
    let releaser = user_principal(user.user_id, RoleCode::Releaser.capabilities());
    let disable = status_input(&user, "left the lab");
    write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, disable)).unwrap();
    assert_eq!(
        reauthenticate(&db, &releaser, PASSWORD).unwrap(),
        Some(Rejection::InvalidCredentials)
    );
}

fn policy_input(
    expected_version: Option<i64>,
    values: (u32, u32, u32),
    reason: Option<&str>,
) -> SetSecurityPolicyInput {
    SetSecurityPolicyInput {
        expected_version,
        idle_timeout_minutes: values.0,
        lockout_failures: values.1,
        lockout_minutes: values.2,
        reason: reason.map(str::to_owned),
        allow_security_key_touch_only: false,
        password: None,
    }
}

fn set_policy(
    db: &TestDatabase,
    principal: &Principal,
    input: SetSecurityPolicyInput,
) -> Result<SecurityPolicyView, AuthError> {
    write(db, principal, move |ctx| {
        lis_auth::set_security_policy(ctx, input).map(|attempt| match attempt {
            Attempt::Accepted(value) => value,
            Attempt::Rejected(reason) => panic!("unexpected policy rejection: {reason}"),
        })
    })
}

#[test]
fn the_security_policy_defaults_apply_until_the_laboratory_sets_versions() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let view = db.db.read_blocking(lis_auth::get_security_policy).unwrap();
    assert_eq!(view.version, None);
    assert_eq!(view.values, DEFAULT_SECURITY_POLICY);
    assert_eq!(
        (view.values.idle_timeout(), view.set_by),
        (Duration::from_secs(15 * 60), None)
    );

    let first = set_policy(&db, &admin, policy_input(None, (10, 4, 20), None)).unwrap();
    assert_eq!(first.version, Some(1));
    assert_eq!(first.defaults, DEFAULT_SECURITY_POLICY);
    assert_eq!(
        (first.set_by, first.set_at),
        (Some(admin_id.to_string()), Some(test_start()))
    );
    let second = set_policy(
        &db,
        &admin,
        policy_input(Some(1), (30, 3, 5), Some("Lab policy QP-12")),
    )
    .unwrap();
    assert_eq!(second.version, Some(2));
    let current = db
        .db
        .read_blocking(lis_auth::current_security_policy)
        .unwrap();
    assert_eq!(
        (
            current.idle_timeout_minutes,
            current.lockout_failures,
            current.lockout_minutes
        ),
        (30, 3, 5)
    );
    let (reason, details): (Option<String>, String) = connection(&db)
        .query_row(
            "SELECT reason, details FROM audit_events WHERE action = 'auth.set_security_policy' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(reason.as_deref(), Some("Lab policy QP-12"));
    assert!(details.contains("\"lockout_failures\":3"), "{details}");
}

#[test]
fn setting_the_security_policy_checks_capability_version_ranges_and_reason() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let configurator = user_principal(UserId::generate(), &[Capability::ManageConfiguration]);
    let denied = set_policy(&db, &configurator, policy_input(None, (15, 5, 15), None));
    assert!(is_denied(&denied.unwrap_err()));

    let invalid = set_policy(&db, &admin, policy_input(None, (1, 11, 61), None)).unwrap_err();
    let fields: Vec<&str> = invalid
        .field_errors()
        .iter()
        .map(|error| error.field.as_str())
        .collect();
    assert_eq!(
        fields,
        ["idleTimeoutMinutes", "lockoutFailures", "lockoutMinutes"]
    );
    let stale = set_policy(&db, &admin, policy_input(Some(1), (15, 5, 15), None));
    assert!(matches!(stale, Err(AuthError::PolicyVersionConflict)));

    set_policy(&db, &admin, policy_input(None, (15, 5, 15), None)).unwrap();
    let replayed = set_policy(&db, &admin, policy_input(None, (20, 5, 15), None));
    assert!(matches!(replayed, Err(AuthError::Invalid(_))));
    for reason in [None, Some("   ")] {
        let unexplained =
            set_policy(&db, &admin, policy_input(Some(1), (20, 5, 15), reason)).unwrap_err();
        assert_eq!(unexplained.field_errors()[0].field, "reason");
    }
    let stale = set_policy(&db, &admin, policy_input(None, (20, 5, 15), Some("why")));
    assert!(matches!(stale, Err(AuthError::PolicyVersionConflict)));
    let versions: i64 = connection(&db)
        .query_row("SELECT count(*) FROM auth_security_policies", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(versions, 1);
}

#[test]
fn security_policy_versions_are_append_only_and_gapless() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    set_policy(&db, &admin, policy_input(None, (15, 5, 15), None)).unwrap();
    let conn = connection(&db);
    for sql in [
        "UPDATE auth_security_policies SET lockout_failures = 10",
        "DELETE FROM auth_security_policies",
        "INSERT INTO auth_security_policies (version, idle_timeout_minutes, lockout_failures, \
         lockout_minutes, reason, set_by, set_at) VALUES (3, 15, 5, 15, 'gap', 'x', 'y')",
        "INSERT INTO auth_security_policies (version, idle_timeout_minutes, lockout_failures, \
         lockout_minutes, reason, set_by, set_at) VALUES (2, 15, 5, 15, NULL, 'x', 'y')",
        "INSERT INTO auth_security_policies (version, idle_timeout_minutes, lockout_failures, \
         lockout_minutes, reason, set_by, set_at) VALUES (2, 61, 5, 15, 'too long', 'x', 'y')",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn the_throttle_applies_the_laboratory_lockout_policy() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    set_policy(&db, &admin, policy_input(None, (15, 3, 5), None)).unwrap();
    for _ in 0..2 {
        assert_eq!(
            sign_in(&db, "ghost", "wrong passphrase").unwrap(),
            Some(Rejection::InvalidCredentials)
        );
    }
    assert_eq!(
        sign_in(&db, "ghost", "wrong passphrase").unwrap(),
        Some(Rejection::LockedOut)
    );
    let just_before = test_start()
        .checked_add(Duration::from_secs(5 * 60 - 1))
        .unwrap();
    db.clock.set(just_before);
    assert_eq!(
        sign_in(&db, "ghost", "wrong passphrase").unwrap(),
        Some(Rejection::LockedOut)
    );
    db.clock.set(
        test_start()
            .checked_add(Duration::from_secs(5 * 60))
            .unwrap(),
    );
    assert_eq!(
        sign_in(&db, "ghost", "wrong passphrase").unwrap(),
        Some(Rejection::InvalidCredentials)
    );
}

fn quality_manager(db: &TestDatabase, admin_id: UserId) -> (UserId, Principal) {
    let qm = create_user(db, &dual_role(admin_id), "qm1", &[RoleCode::QualityManager]);
    (
        qm.user_id,
        user_principal(qm.user_id, RoleCode::QualityManager.capabilities()),
    )
}

fn set_roles(
    db: &TestDatabase,
    actor: &Principal,
    user: &UserSummary,
    roles: &[RoleCode],
    password: &str,
) -> Result<Attempt<UserSummary>, AuthError> {
    let input = SetUserRolesInput {
        user_id: user.user_id,
        roles: roles.to_vec(),
        expected_revision: user.revision,
        reason: "Staffing change".into(),
        password: SecretString::from(password).into(),
    };
    write(db, actor, move |ctx| lis_auth::set_user_roles(ctx, input))
}

fn reset(
    db: &TestDatabase,
    actor: &Principal,
    user: &UserSummary,
    password: &str,
) -> Result<Attempt<UserSummary>, AuthError> {
    let input = lis_auth::ResetUserCredentialsInput {
        user_id: user.user_id,
        temporary_password: SecretString::from("temporary passphrase 9"),
        reset_mfa: true,
        expected_revision: user.revision,
        reason: "Lost phone".into(),
        password: SecretString::from(password).into(),
    };
    write(db, actor, move |ctx| {
        lis_auth::reset_user_credentials(ctx, input)
    })
}

fn summary_of(db: &TestDatabase, id: UserId) -> UserSummary {
    db.db
        .read_blocking(|conn| lis_auth::list_users(conn, test_start()))
        .unwrap()
        .into_iter()
        .find(|user| user.user_id == id)
        .unwrap()
}

fn denied_capability(result: Result<Attempt<UserSummary>, AuthError>) -> Capability {
    match result {
        Err(AuthError::Store(StoreError::Denied(denied))) => denied.capability,
        other => panic!("not denied: {other:?}"),
    }
}

#[test]
fn administration_alone_cannot_grant_or_create_clinical_authority() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    quality_manager(&db, admin_id);
    let clerk = create_user(&db, &admin, "billing1", &[RoleCode::Billing]);
    let refused = set_roles(&db, &admin, &clerk, &[RoleCode::Releaser], PASSWORD);
    assert_eq!(denied_capability(refused), Capability::ManageQuality);
    let input = CreateUserInput {
        username: "releaser9".into(),
        display_name: "Test releaser9".into(),
        roles: vec![RoleCode::Releaser],
        temporary_password: SecretString::from(PASSWORD),
        password: SecretString::from(PASSWORD).into(),
    };
    let refused = write(&db, &admin, move |ctx| lis_auth::create_user(ctx, input));
    assert_eq!(denied_capability(refused), Capability::ManageQuality);
    // With an active quality manager, the bootstrap grant is closed too.
    let refused = set_roles(&db, &admin, &clerk, &[RoleCode::QualityManager], PASSWORD);
    assert_eq!(denied_capability(refused), Capability::ManageQuality);
    assert_eq!(summary_of(&db, clerk.user_id).roles, [RoleCode::Billing]);
}

#[test]
fn a_quality_manager_grants_clinical_roles_but_not_accounts_or_other_roles() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let (_, qm) = quality_manager(&db, admin_id);
    let person = create_user(&db, &admin, "person1", &[]);
    let granted = accepted(set_roles(&db, &qm, &person, &[RoleCode::Releaser], PASSWORD).unwrap());
    assert_eq!(granted.roles, [RoleCode::Releaser]);
    let refused = set_roles(
        &db,
        &qm,
        &granted,
        &[RoleCode::Releaser, RoleCode::Auditor],
        PASSWORD,
    );
    assert_eq!(denied_capability(refused), Capability::ManageUsers);
    let input = CreateUserInput {
        username: "person2".into(),
        display_name: "Test person2".into(),
        roles: vec![],
        temporary_password: SecretString::from(PASSWORD),
        password: SecretString::from(PASSWORD).into(),
    };
    let refused = write(&db, &qm, move |ctx| lis_auth::create_user(ctx, input));
    assert_eq!(denied_capability(refused), Capability::ManageUsers);
}

#[test]
fn administration_appoints_only_the_first_quality_manager() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let person = create_user(&db, &admin, "person1", &[]);
    let refused = set_roles(
        &db,
        &admin,
        &person,
        &[RoleCode::QualityManager, RoleCode::Analyst],
        PASSWORD,
    );
    assert_eq!(denied_capability(refused), Capability::ManageQuality);
    let appointed =
        accepted(set_roles(&db, &admin, &person, &[RoleCode::QualityManager], PASSWORD).unwrap());
    assert_eq!(appointed.roles, [RoleCode::QualityManager]);
    // The last active quality manager stays, or the bootstrap would reopen.
    let disable = status_input(&appointed, "leaving");
    let refused = write(&db, &admin, move |ctx| lis_auth::disable_user(ctx, disable));
    assert!(matches!(refused, Err(AuthError::LastQualityManager)));
}

#[test]
fn nobody_changes_their_own_roles_or_resets_their_own_sign_in() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let both = dual_role(admin_id);
    let me = summary_of(&db, admin_id);
    let before = audit_actions(&db).len();
    let own_roles = set_roles(
        &db,
        &both,
        &me,
        &[RoleCode::Administrator, RoleCode::Releaser],
        PASSWORD,
    );
    assert!(matches!(
        own_roles,
        Ok(Attempt::Rejected(Rejection::OwnAccount))
    ));
    let own_reset = reset(&db, &both, &me, PASSWORD);
    assert!(matches!(
        own_reset,
        Ok(Attempt::Rejected(Rejection::OwnAccount))
    ));
    assert_eq!(Rejection::OwnAccount.code(), lis_core::ErrorCode::Forbidden);
    assert_eq!(summary_of(&db, admin_id), me, "nothing changed");
    // Each refusal committed a denied event.
    assert_eq!(
        audit_actions(&db)[before..],
        [
            ("auth.user_roles_set".to_owned(), "denied".to_owned()),
            ("auth.credentials_reset".to_owned(), "denied".to_owned()),
        ]
    );
}

#[test]
fn administration_needs_the_actors_password_again() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let (_, qm) = quality_manager(&db, admin_id);
    let user = create_user(&db, &admin, "person1", &[]);
    let wrong = "wrong passphrase";
    let refused = set_roles(&db, &qm, &user, &[RoleCode::Releaser], wrong).unwrap();
    assert!(matches!(
        refused,
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    let refused = reset(&db, &admin, &user, wrong).unwrap();
    assert!(matches!(
        refused,
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    let input = CreateUserInput {
        username: "person2".into(),
        display_name: "Test person2".into(),
        roles: vec![],
        temporary_password: SecretString::from(PASSWORD),
        password: SecretString::from(wrong).into(),
    };
    let refused = write(&db, &admin, move |ctx| lis_auth::create_user(ctx, input)).unwrap();
    assert!(matches!(
        refused,
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    assert_eq!(summary_of(&db, user.user_id), user, "nothing changed");
    let users = db
        .db
        .read_blocking(|conn| lis_auth::list_users(conn, test_start()))
        .unwrap();
    assert_eq!(users.len(), 3, "no account was created");
    let purposes: Vec<String> = reauth_events(&db)
        .into_iter()
        .filter(|(outcome, _)| outcome == "failed")
        .map(|(_, purpose)| purpose)
        .collect();
    assert_eq!(
        purposes,
        ["role_change", "credential_reset", "account_creation"]
    );
}

#[test]
fn a_credential_reset_ends_the_users_open_sessions() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    // Resetting a clinical-role holder needs both duties (Q-37).
    let both = dual_role(admin_id);
    let user = create_user(&db, &both, "analyst1", &[RoleCode::Analyst]);
    let (session, workstation) = (SessionId::generate(), WorkstationId::generate());
    let conn = connection(&db);
    conn.execute(
        "INSERT INTO auth_workstations (id, registered_at) VALUES (?1, ?2)",
        rusqlite::params![workstation, test_start()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO auth_sessions (id, user_id, workstation_id, started_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session, user.user_id, workstation, test_start()],
    )
    .unwrap();
    accepted(reset(&db, &both, &user, PASSWORD).unwrap());
    let ended: (bool, Option<String>) = conn
        .query_row(
            "SELECT ended_at IS NOT NULL, end_reason FROM auth_sessions WHERE id = ?1",
            [session],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(ended, (true, Some("credentials_reset".to_owned())));
}

// REQ-IDN-004, NFR-19: an end by policy keeps its own reason, not `abandoned`.
#[test]
fn policy_ends_record_expired_and_account_disabled() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let conn = connection(&db);
    let workstation = WorkstationId::generate();
    conn.execute(
        "INSERT INTO auth_workstations (id, registered_at) VALUES (?1, ?2)",
        rusqlite::params![workstation, test_start()],
    )
    .unwrap();
    for (reason, code) in [
        (EndReason::Expired, "expired"),
        (EndReason::AccountDisabled, "account_disabled"),
    ] {
        let session = SessionId::generate();
        conn.execute(
            "INSERT INTO auth_sessions (id, user_id, workstation_id, started_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![session, admin_id, workstation, test_start()],
        )
        .unwrap();
        write(&db, &admin, move |ctx| {
            lis_auth::end_session(ctx, session, reason)
        })
        .unwrap();
        let stored: Option<String> = conn
            .query_row(
                "SELECT end_reason FROM auth_sessions WHERE id = ?1",
                [session],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored.as_deref(), Some(code));
    }
}

#[test]
fn the_role_catalog_offers_what_the_caller_may_assign() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let assignable = |caller: Principal| -> Vec<RoleCode> {
        db.db
            .read_blocking(move |conn| lis_auth::list_roles(conn, &caller))
            .unwrap()
            .into_iter()
            .filter(|role| role.assignable)
            .map(|role| role.code)
            .collect()
    };
    let plain = [
        RoleCode::Billing,
        RoleCode::Auditor,
        RoleCode::Administrator,
    ];
    let mut first = vec![RoleCode::QualityManager];
    first.extend(plain);
    assert_eq!(assignable(admin.clone()), first);
    let (_, qm) = quality_manager(&db, admin_id);
    assert_eq!(assignable(admin), plain);
    assert_eq!(
        assignable(qm),
        [
            RoleCode::Accessioner,
            RoleCode::Analyst,
            RoleCode::TechnicalReviewer,
            RoleCode::Releaser,
            RoleCode::QualityManager,
        ]
    );
}

const ALL_ALLOWED: SeparationPolicy = SeparationPolicy {
    self_approve_configuration: true,
    self_release_reports: true,
    self_verify_destinations: true,
};

fn separation_input(
    expected_version: Option<i64>,
    values: SeparationPolicy,
    reason: &str,
    password: &str,
) -> SetSeparationPolicyInput {
    SetSeparationPolicyInput {
        expected_version,
        values,
        reason: reason.into(),
        password: SecretString::from(password).into(),
    }
}

fn set_separation(
    db: &TestDatabase,
    principal: &Principal,
    input: SetSeparationPolicyInput,
) -> Result<Attempt<SeparationPolicyView>, AuthError> {
    write(db, principal, move |ctx| {
        lis_auth::set_separation_policy(ctx, input)
    })
}

fn designate(
    db: &TestDatabase,
    principal: &Principal,
    reason: &str,
) -> Result<Attempt<UserSummary>, AuthError> {
    let input = DesignateOnePersonLaboratoryInput {
        reason: reason.into(),
        password: SecretString::from(PASSWORD).into(),
    };
    write(db, principal, move |ctx| {
        lis_auth::designate_one_person_laboratory(ctx, input)
    })
}

fn current_separation(db: &TestDatabase) -> SeparationPolicy {
    db.db
        .read_blocking(|conn| Ok::<_, AuthError>(lis_auth::current_separation_policy(conn)?))
        .unwrap()
}

/// The values the schema guards of other modules read.
fn separation_view(db: &TestDatabase) -> Option<(i64, bool, bool, bool)> {
    connection(db)
        .query_row(
            "SELECT version, self_approve_configuration, self_release_reports, \
             self_verify_destinations FROM auth_v_separation_policy",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .unwrap()
}

fn roles_of(db: &TestDatabase, user_id: UserId) -> Vec<RoleCode> {
    db.db
        .read_blocking(move |conn| lis_auth::list_users(conn, test_start()))
        .unwrap()
        .into_iter()
        .find(|user| user.user_id == user_id)
        .unwrap()
        .roles
}

#[test]
fn every_duty_needs_a_second_person_until_the_laboratory_sets_a_policy() {
    let db = open();
    bootstrap(&db);
    let view = db
        .db
        .read_blocking(lis_auth::get_separation_policy)
        .unwrap();
    assert_eq!(
        (view.version, view.values, view.reason),
        (None, DEFAULT_SEPARATION_POLICY, None)
    );
    assert_eq!(current_separation(&db), DEFAULT_SEPARATION_POLICY);
    assert_eq!(separation_view(&db), None);
}

#[test]
fn a_one_person_first_run_grants_every_role_and_allows_every_duty() {
    let db = open();
    let input = CreateFirstAdministratorInput {
        username: "Director1".into(),
        display_name: "Test Director".into(),
        password: SecretString::from(PASSWORD),
        one_person: true,
        load_demo_data: None,
    };
    let id = write(&db, &sign_in_screen(), move |ctx| {
        lis_auth::create_first_administrator(ctx, input)
    })
    .unwrap();
    assert_eq!(roles_of(&db, id), RoleCode::ALL);
    let view = db
        .db
        .read_blocking(lis_auth::get_separation_policy)
        .unwrap();
    assert_eq!((view.version, view.values), (Some(1), ALL_ALLOWED));
    assert_eq!(
        view.reason.as_deref(),
        Some("First-run setup: one person runs this laboratory.")
    );
    assert_eq!(separation_view(&db), Some((1, true, true, true)));
    assert_eq!(
        audit_actions(&db),
        [
            (
                "auth.first_administrator_create".to_owned(),
                "succeeded".to_owned()
            ),
            (
                "auth.set_separation_policy".to_owned(),
                "succeeded".to_owned()
            ),
        ]
    );
}

#[test]
fn quality_authority_cannot_change_application_separation_policy() {
    let db = open();
    let (admin_id, _) = bootstrap(&db);
    let quality = user_principal(admin_id, &[Capability::ManageQuality]);
    assert!(is_denied(
        &set_separation(
            &db,
            &quality,
            separation_input(None, ALL_ALLOWED, "Supplied policy", PASSWORD)
        )
        .unwrap_err()
    ));
    assert_eq!(separation_view(&db), None);
}

#[test]
fn administrator_separation_policy_change_requires_reason_version_and_fresh_password() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let input = || separation_input(None, ALL_ALLOWED, "Director works alone", PASSWORD);
    let unexplained = separation_input(None, ALL_ALLOWED, "  ", PASSWORD);
    let error = set_separation(&db, &admin, unexplained).unwrap_err();
    assert_eq!(error.field_errors()[0].field, "reason");
    let stale = separation_input(Some(1), ALL_ALLOWED, "why", PASSWORD);
    assert!(matches!(
        set_separation(&db, &admin, stale),
        Err(AuthError::SeparationPolicyVersionConflict)
    ));
    let wrong = separation_input(None, ALL_ALLOWED, "why", "wrong passphrase");
    assert!(matches!(
        set_separation(&db, &admin, wrong).unwrap(),
        Attempt::Rejected(Rejection::WrongPassword)
    ));
    assert_eq!(separation_view(&db), None);

    let first = accepted(set_separation(&db, &admin, input()).unwrap());
    assert_eq!((first.version, first.values), (Some(1), ALL_ALLOWED));
    assert_eq!(first.set_by, Some(admin_id.to_string()));
    let release_only = SeparationPolicy {
        self_release_reports: true,
        ..DEFAULT_SEPARATION_POLICY
    };
    let second = separation_input(Some(1), release_only, "Second reviewer hired", PASSWORD);
    let second = accepted(set_separation(&db, &admin, second).unwrap());
    assert_eq!(second.version, Some(2));
    assert_eq!(current_separation(&db), release_only);
    assert_eq!(separation_view(&db), Some((2, false, true, false)));
    let (reason, details, authority): (String, String, String) = connection(&db)
        .query_row(
            "SELECT reason, details, authority FROM audit_events \
             WHERE action = 'auth.set_separation_policy' ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(reason, "Second reviewer hired");
    assert_eq!(authority, "approve_configuration");
    assert!(
        details.contains("\"self_release_reports\":true"),
        "{details}"
    );
}

#[test]
fn separation_policy_versions_are_append_only_and_gapless() {
    let db = open();
    let (_, admin) = bootstrap(&db);
    let input = separation_input(None, ALL_ALLOWED, "Director works alone", PASSWORD);
    accepted(set_separation(&db, &admin, input).unwrap());
    let conn = connection(&db);
    for sql in [
        "UPDATE auth_separation_policies SET self_release_reports = 0",
        "DELETE FROM auth_separation_policies",
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (3, 0, 0, 0, 'gap', 'x', 'y')",
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (2, 0, 0, 0, '', 'x', 'y')",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn only_the_one_active_account_can_set_up_a_one_person_laboratory() {
    let db = open();
    let (admin_id, admin) = bootstrap(&db);
    let other = create_user(&db, &admin, "Billing1", &[RoleCode::Billing]);
    assert!(matches!(
        designate(&db, &admin, "Director works alone"),
        Err(AuthError::NotOnlyActiveAccount)
    ));
    let no_rights = user_principal(admin_id, &[Capability::ManageQuality]);
    assert!(is_denied(&designate(&db, &no_rights, "why").unwrap_err()));

    write(&db, &admin, move |ctx| {
        lis_auth::disable_user(ctx, status_input(&other, "Left the laboratory"))
    })
    .unwrap();
    let error = designate(&db, &admin, " ").unwrap_err();
    assert_eq!(error.field_errors()[0].field, "reason");
    let summary = accepted(designate(&db, &admin, "Director works alone").unwrap());
    assert_eq!(summary.roles, RoleCode::ALL);
    assert_eq!(current_separation(&db), ALL_ALLOWED);
    let (reason, details): (String, String) = connection(&db)
        .query_row(
            "SELECT reason, details FROM audit_events \
             WHERE action = 'auth.user_roles_set' ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(reason, "Director works alone");
    assert!(details.contains("one_person_laboratory"), "{details}");
}
