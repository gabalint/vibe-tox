//! First-run setup (REQ-IDN-004): the first account, created while none exists. For a
//! laboratory that one person runs, the account gets every role and the separation
//! policy lets it do each duty alone (ADR 0015).

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, UserId};
use lis_store::CommandContext;
use secrecy::SecretString;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::admin::{account_created_event, insert_account, validate_account};
use crate::duties::acting_user;
use crate::error::AuthError;
use crate::one_person::{allow_first_run_account, first_run_roles};
use crate::repo;

/// Input of `create_first_administrator`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateFirstAdministratorInput {
    /// Sign-in name.
    pub username: String,
    /// The person's name.
    pub display_name: String,
    /// Their own (not temporary) password.
    #[ts(type = "string")]
    pub password: SecretString,
    /// One person runs this laboratory: the account gets every role and may do each
    /// duty alone (ADR 0015).
    pub one_person: bool,
    /// Prepare the bundled catalog and synthetic records after first sign-in.
    /// The engine accepts this only in a demo installation.
    #[serde(default)]
    #[ts(optional)]
    pub load_demo_data: Option<bool>,
}

/// First-run setup: creates the administrator while no account exists. The
/// check runs inside the write transaction, so it holds against a race. For a
/// one-person laboratory the account gets every role, and the separation policy
/// lets it do each duty alone (ADR 0015).
pub fn create_first_administrator(
    ctx: &mut CommandContext<'_>,
    input: CreateFirstAdministratorInput,
) -> Result<UserId, AuthError> {
    if repo::any_user(ctx.tx())? {
        return Err(AuthError::AlreadySetUp);
    }
    let demo_setup = validate_setup_options(input.one_person, input.load_demo_data)?;
    let account = validate_account(
        &input.username,
        &input.display_name,
        input.password,
        "password",
    )?;
    let roles = first_run_roles(input.one_person);
    let id = insert_account(ctx, &account, &roles, false)?;
    ctx.audit(account_created_event(
        actions::FIRST_ADMINISTRATOR_CREATE,
        id,
        &account,
        &roles,
    ))?;
    if input.one_person {
        allow_first_run_account(ctx)?;
    }
    if demo_setup {
        repo::request_demo_setup(ctx.tx(), id, ctx.now())?;
        ctx.audit(
            AuditEvent::new(
                Stream::Security,
                "auth.demo_setup_requested",
                Outcome::Succeeded,
            )
            .record("user", id),
        )?;
    }
    Ok(id)
}

/// Whether this signed-in user owns the pending first-run demo preparation.
/// Refuses another account; a finished or unrequested setup is an idempotent no-op.
pub fn demo_setup_pending_for(ctx: &mut CommandContext<'_>) -> Result<bool, AuthError> {
    ctx.require(Capability::ManageConfiguration)?;
    ctx.require(Capability::EnterOrders)?;
    let Some(owner) = repo::pending_demo_setup_user(ctx.tx())? else {
        return Ok(false);
    };
    if acting_user(ctx) != Some(owner) {
        return Err(AuthError::DemoSetupAccount);
    }
    Ok(true)
}

/// Records completion in the same transaction as the engine's catalog and demo loads.
pub fn complete_demo_setup(ctx: &mut CommandContext<'_>) -> Result<(), AuthError> {
    if !demo_setup_pending_for(ctx)? {
        return Ok(());
    }
    let user = acting_user(ctx).ok_or(AuthError::DemoSetupAccount)?;
    repo::finish_demo_setup(ctx.tx(), ctx.now())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "auth.demo_setup_completed",
            Outcome::Succeeded,
        )
        .record("user", user),
    )?;
    Ok(())
}

/// The one home for first-run data-choice constraints.
pub(crate) fn validate_setup_options(
    one_person: bool,
    load_demo_data: Option<bool>,
) -> Result<bool, AuthError> {
    let demo_setup = load_demo_data.unwrap_or(false);
    if demo_setup && !one_person {
        return Err(AuthError::Invalid(vec![FieldError {
            field: "onePerson".to_owned(),
            code: "required".to_owned(),
            message: "Demo preparation needs the one-person account so it can load every record."
                .to_owned(),
        }]));
    }
    Ok(demo_setup)
}
