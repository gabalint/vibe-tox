//! The table-driven authorization test over the command registry (REQ-IDN-003,
//! NFR-08): every capability command refuses a principal without its
//! capability and writes nothing but the denial event; every session command
//! refuses a caller without a session and writes nothing at all.

use lis_auth::{
    ChangeInitialPasswordInput, ChangePasswordInput, ChangeUserStatusInput,
    CreateFirstAdministratorInput, CreateUserInput, DesignateOnePersonLaboratoryInput,
    ResetUserCredentialsInput, SetUserRolesInput, SignInInput, UnlockSessionInput, VerifyMfaInput,
};
use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::registry::{Access, COMMANDS, CommandSpec};
use lis_engine::{AuditStream, Engine, ListAuditEventsInput, VerifyAuditChainInput};

use crate::fixture::{PASSWORD, TestEngine, secret};

/// Calls the engine method behind `name` with plausible input aimed at `target`.
async fn call(engine: &Engine, name: &str, target: UserId) -> Result<(), CommandError> {
    if let Some(result) = crate::remediation::remediation_call(engine, name).await {
        return result;
    }
    if let Some(result) = crate::billing_intake::billing_intake_call(engine, name).await {
        return result;
    }
    if let Some(result) = security_key_call(engine, name).await {
        return result;
    }
    if let Some(result) = session_call(engine, name).await {
        return result;
    }
    if let Some(result) = admin_call(engine, name, target).await {
        return result;
    }
    if let Some(result) = m2_call(engine, name, target).await {
        return result;
    }
    if let Some(result) = m3_m4_call(engine, name, target).await {
        return result;
    }
    if let Some(result) = wave10_call(engine, name, target).await {
        return result;
    }
    panic!("no test call for registered command {name}")
}

// Wave 3+: each area chains its own `<area>_call` from its own test module, grouped by
// milestone to keep each dispatcher within the complexity budget.
async fn m2_call(engine: &Engine, name: &str, target: UserId) -> Option<Result<(), CommandError>> {
    // identity: begin
    if let Some(result) = crate::identity::identity_call(engine, name, target).await {
        return Some(result);
    }
    // identity: end
    // catalog: begin
    if let Some(result) = crate::catalog::catalog_call(engine, name, target).await {
        return Some(result);
    }
    // The Catalog Preview commands are `Session` in the registry, because it has no
    // either-of access; `starter_catalog.rs` tests their manage-or-approve rule.
    if let Some(result) = crate::starter_catalog::starter_catalog_call(engine, name, target).await {
        return Some(result);
    }
    // catalog: end
    // interfaces: begin
    if let Some(result) = crate::interfaces::interfaces_call(engine, name, target).await {
        return Some(result);
    }
    // interfaces: end
    // orders: begin
    if let Some(result) = crate::orders::orders_call(engine, name, target).await {
        return Some(result);
    }
    // orders: end
    // specimens: begin
    if let Some(result) = crate::specimens::specimens_call(engine, name, target).await {
        return Some(result);
    }
    // specimens: end
    // analytical: begin
    if let Some(result) = crate::analytical::analytical_call(engine, name, target).await {
        return Some(result);
    }
    // analytical: end
    // results: begin
    if let Some(result) = crate::results::results_call(engine, name, target).await {
        return Some(result);
    }
    // results: end
    None
}

async fn m3_m4_call(
    engine: &Engine,
    name: &str,
    target: UserId,
) -> Option<Result<(), CommandError>> {
    // reporting: begin
    if let Some(result) = crate::reporting::reporting_call(engine, name, target).await {
        return Some(result);
    }
    // reporting: end
    // billing: begin
    if let Some(result) = crate::billing::billing_call(engine, name, target).await {
        return Some(result);
    }
    // billing: end
    // work_queues: begin
    if let Some(result) = crate::work_queues::work_queues_call(engine, name, target).await {
        return Some(result);
    }
    // work_queues: end
    // backup: begin
    if let Some(result) = crate::m4_backup::backup_call(engine, name, target).await {
        return Some(result);
    }
    // backup: end
    // runtime: begin
    if let Some(result) = crate::m4_runtime::runtime_call(engine, name, target).await {
        return Some(result);
    }
    // runtime: end
    // anchors: begin
    if let Some(result) = crate::m4_anchors::anchors_call(engine, name, target).await {
        return Some(result);
    }
    // anchors: end
    // demo: begin
    if let Some(result) = crate::demo_lab::demo_call(engine, name, target).await {
        return Some(result);
    }
    // demo: end
    // result_files: begin
    if let Some(result) = crate::result_files_intake::result_files_call(engine, name, target).await
    {
        return Some(result);
    }
    // result_files: end
    // file_templates: begin
    if let Some(result) = crate::file_templates::file_templates_call(engine, name, target).await {
        return Some(result);
    }
    // file_templates: end
    None
}

async fn wave10_call(
    engine: &Engine,
    name: &str,
    target: UserId,
) -> Option<Result<(), CommandError>> {
    // result_files_import: begin
    if let Some(result) = crate::rf_lab::result_import_call(engine, name, target).await {
        return Some(result);
    }
    // result_files_import: end
    None
}

async fn session_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    if let Some(result) = pin_call(engine, name).await {
        return Some(result);
    }
    Some(match name {
        "setup_status" => engine.setup_status().await.map(drop),
        "create_first_administrator" => {
            engine
                .create_first_administrator(CreateFirstAdministratorInput {
                    username: "admin9".into(),
                    display_name: "Nine".into(),
                    password: secret(PASSWORD),
                    one_person: false,
                    load_demo_data: None,
                })
                .await
        }
        "sign_in" => engine
            .sign_in(SignInInput {
                username: "nobody".into(),
                password: secret(PASSWORD),
            })
            .await
            .map(drop),
        "change_initial_password" => engine
            .change_initial_password(ChangeInitialPasswordInput {
                challenge_id: lis_auth::ChallengeId::generate(),
                new_password: secret(PASSWORD),
            })
            .await
            .map(drop),
        "verify_mfa" => engine
            .verify_mfa(VerifyMfaInput {
                challenge_id: lis_auth::ChallengeId::generate(),
                code: secret("123456"),
            })
            .await
            .map(drop),
        "current_session" => engine.current_session().await.map(drop),
        "list_listener_status" => engine.list_listener_status().await.map(drop),
        "touch_session" => engine.touch_session().await,
        "lock_session" => engine.lock_session().await,
        "unlock_session" => engine
            .unlock_session(UnlockSessionInput {
                password: secret(PASSWORD),
            })
            .await
            .map(drop),
        "sign_out" => engine.sign_out().await,
        "change_password" => {
            engine
                .change_password(ChangePasswordInput {
                    current_password: secret(PASSWORD),
                    new_password: secret("another passphrase 3"),
                })
                .await
        }
        _ => return None,
    })
}

async fn pin_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "get_unlock_pin_status" => engine.get_unlock_pin_status().await.map(drop),
        "set_unlock_pin" => {
            engine
                .set_unlock_pin(lis_auth::SetUnlockPinInput {
                    pin: secret("482951"),
                    password: secret(PASSWORD).into(),
                })
                .await
        }
        "disable_unlock_pin" => {
            engine
                .disable_unlock_pin(lis_auth::DisableUnlockPinInput {
                    password: secret(PASSWORD).into(),
                })
                .await
        }
        "unlock_with_pin" => engine
            .unlock_with_pin(lis_auth::UnlockWithPinInput {
                pin: secret("482951"),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn admin_call(
    engine: &Engine,
    name: &str,
    target: UserId,
) -> Option<Result<(), CommandError>> {
    let status = || ChangeUserStatusInput {
        user_id: target,
        expected_revision: 1,
        reason: "Test".into(),
    };
    Some(match name {
        "list_roles" => engine.list_roles().await.map(drop),
        "list_users" => engine.list_users().await.map(drop),
        "create_user" => engine
            .create_user(CreateUserInput {
                username: "intruder".into(),
                display_name: "Intruder".into(),
                roles: vec![lis_auth::RoleCode::Administrator],
                temporary_password: secret(PASSWORD),
                password: secret(PASSWORD).into(),
            })
            .await
            .map(drop),
        "set_user_roles" => engine
            .set_user_roles(SetUserRolesInput {
                user_id: target,
                roles: vec![],
                expected_revision: 1,
                reason: "Test".into(),
                password: secret(PASSWORD).into(),
            })
            .await
            .map(drop),
        "disable_user" => engine.disable_user(status()).await.map(drop),
        "enable_user" => engine.enable_user(status()).await.map(drop),
        "reset_user_credentials" => engine
            .reset_user_credentials(ResetUserCredentialsInput {
                user_id: target,
                temporary_password: secret(PASSWORD),
                reset_mfa: true,
                expected_revision: 1,
                reason: "Test".into(),
                password: secret(PASSWORD).into(),
            })
            .await
            .map(drop),
        "designate_one_person_laboratory" => engine
            .designate_one_person_laboratory(DesignateOnePersonLaboratoryInput {
                reason: "Test".into(),
                password: secret(PASSWORD).into(),
            })
            .await
            .map(drop),
        "list_audit_events" => engine
            .list_audit_events(ListAuditEventsInput {
                stream: AuditStream::Security,
                before_seq: None,
                limit: 10,
                action: None,
                record_type: None,
                record_id: None,
                self_approvals_only: None,
            })
            .await
            .map(drop),
        "verify_audit_chain" => engine
            .verify_audit_chain(VerifyAuditChainInput {
                stream: AuditStream::Security,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// Every command the engine runs (`app_info` is answered by the shell alone).
fn engine_commands() -> impl Iterator<Item = &'static CommandSpec> {
    COMMANDS.iter().filter(|spec| spec.name != "app_info")
}

#[tokio::test]
async fn every_capability_command_refuses_a_user_without_it() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let admin = t.engine.list_users().await.unwrap().remove(0).user_id;
    t.create_user("noroles", &[]).await;
    t.first_sign_in("noroles").await;
    let mut checked = 0;
    for spec in engine_commands() {
        let capability = match spec.access {
            Access::Capability(capability) | Access::EitherCapability(_, capability) => capability,
            Access::Public | Access::AnySession | Access::Session => continue,
        };
        let before = t.fingerprint();
        let result = call(&t.engine, spec.name, admin).await;
        assert_eq!(
            result.unwrap_err().code(),
            ErrorCode::Forbidden,
            "{}",
            spec.name
        );
        let after = t.fingerprint();
        let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
        let removed = before.iter().filter(|row| !after.contains(row)).count();
        assert_eq!((added.len(), removed), (1, 0), "{}: {added:?}", spec.name);
        let expected = format!(
            "Text(\"security\"), Integer({}),",
            t.events("security").len()
        );
        assert!(
            added[0].starts_with("audit_events: [") && added[0].contains(&expected),
            "{added:?}"
        );
        let (action, outcome) = t.events("security").pop().unwrap();
        assert_eq!(
            (action.as_str(), outcome.as_str()),
            (spec.action, "denied"),
            "{}",
            spec.name
        );
        assert!(
            added[0].contains(&format!("{{\\\"capability\\\":\\\"{capability}\\\"}}")),
            "{added:?}"
        );
        checked += 1;
    }
    // Which access each command gets is reviewed in the registry; here the loop
    // must simply not be vacuous.
    assert!(checked > 0, "no capability command was checked");
}

#[tokio::test]
async fn every_session_command_refuses_a_caller_without_a_session() {
    let t = TestEngine::start().await;
    t.bootstrap().await;
    for spec in engine_commands().filter(|spec| spec.access != Access::Public) {
        let before = t.fingerprint();
        let result = call(&t.engine, spec.name, UserId::generate()).await;
        assert_eq!(
            result.unwrap_err().code(),
            ErrorCode::Unauthenticated,
            "{}",
            spec.name
        );
        assert_eq!(t.fingerprint(), before, "{} wrote something", spec.name);
    }
}

#[tokio::test]
async fn every_registered_command_has_a_test_call() {
    let t = TestEngine::start().await;
    for spec in engine_commands() {
        // Panics for a registered command the dispatcher above does not know.
        let _ = call(&t.engine, spec.name, UserId::generate()).await;
    }
}

/// Any session-dependent command must stop before entering native hardware.
struct AuthorizationKeyClient {
    public: bool,
}
impl lis_engine::SecurityKeyClient for AuthorizationKeyClient {
    async fn register(
        &self,
        _: String,
        _: serde_json::Value,
    ) -> Result<serde_json::Value, CommandError> {
        assert!(self.public, "unauthorized native registration");
        Err(CommandError::new(
            ErrorCode::Conflict,
            "Synthetic cancellation",
            lis_core::CorrelationId::generate(),
        ))
    }
    async fn authenticate(
        &self,
        _: String,
        _: serde_json::Value,
    ) -> Result<serde_json::Value, CommandError> {
        assert!(self.public, "unauthorized native assertion");
        Err(CommandError::new(
            ErrorCode::Conflict,
            "Synthetic cancellation",
            lis_core::CorrelationId::generate(),
        ))
    }
}

async fn security_key_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    if let Some(result) = security_key_public_call(engine, name).await {
        return Some(result);
    }
    security_key_session_call(engine, name).await
}

async fn security_key_public_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    use lis_engine::{
        KeyRecoveryAcknowledgement, KeySignInChoicesInput, SecurityKeyRecoveryInput,
        SecurityKeySignInInput, SecurityKeySignupInput,
    };
    let public = AuthorizationKeyClient { public: true };
    Some(match name {
        "list_key_sign_in_choices" => engine
            .list_key_sign_in_choices(KeySignInChoicesInput {
                username: "nobody".into(),
            })
            .await
            .map(drop),
        "security_key_signup" => engine
            .security_key_signup(
                SecurityKeySignupInput {
                    username: "nobody".into(),
                    display_name: "Nobody".into(),
                    one_person: false,
                    load_demo_data: None,
                    key_label: "Synthetic key".into(),
                },
                &public,
            )
            .await
            .map(drop),
        "acknowledge_key_recovery" => engine
            .acknowledge_key_recovery(KeyRecoveryAcknowledgement {
                recovery_code: secret("synthetic"),
            })
            .await
            .map(drop),
        "security_key_sign_in" => engine
            .security_key_sign_in(
                SecurityKeySignInInput {
                    username: "nobody".into(),
                    credential_id: lis_auth::SecurityKeyId::generate(),
                },
                &public,
            )
            .await
            .map(drop),
        "security_key_recover" => engine
            .security_key_recover(
                SecurityKeyRecoveryInput {
                    username: "nobody".into(),
                    recovery_code: secret("synthetic"),
                    key_label: "Synthetic key".into(),
                },
                &public,
            )
            .await
            .map(drop),
        _ => return None,
    })
}

async fn security_key_session_call(
    engine: &Engine,
    name: &str,
) -> Option<Result<(), CommandError>> {
    use lis_engine::{
        RevokeSecurityKeyInput, SecurityKeyAction, SecurityKeyActionInput, SecurityKeyEnrollInput,
        SecurityKeyUnlockInput,
    };
    let id = lis_auth::SecurityKeyId::generate();
    let session = AuthorizationKeyClient { public: false };
    Some(match name {
        "get_authentication_status" => engine.get_authentication_status().await.map(drop),
        "security_key_unlock" => engine
            .security_key_unlock(SecurityKeyUnlockInput { credential_id: id }, &session)
            .await
            .map(drop),
        "security_key_enroll" => engine
            .security_key_enroll(
                SecurityKeyEnrollInput {
                    key_label: "Synthetic key".into(),
                    password: secret(PASSWORD).into(),
                    credential_id: None,
                },
                &session,
            )
            .await
            .map(drop),
        "security_key_action" => engine
            .security_key_action(
                SecurityKeyActionInput {
                    credential_id: id,
                    action: SecurityKeyAction::DesignateOnePersonLaboratory(
                        DesignateOnePersonLaboratoryInput {
                            reason: "Test".into(),
                            password: secret(PASSWORD).into(),
                        },
                    ),
                },
                None,
                &session,
            )
            .await
            .map(drop),
        "revoke_security_key" => {
            engine
                .revoke_security_key(RevokeSecurityKeyInput {
                    credential_id: id,
                    expected_revision: 1,
                    password: secret(PASSWORD).into(),
                    reason: "Test".into(),
                })
                .await
        }
        _ => return None,
    })
}
