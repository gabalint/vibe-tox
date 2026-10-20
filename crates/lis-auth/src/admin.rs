//! User administration (REQ-IDN-004, REQ-OPS-001); first-run setup is in `setup`.
//! Every change is audited on the security stream; changes to an existing
//! account need the expected revision and a reason.
//!
//! Separation of duties: nobody administers their own account; clinical roles
//! and credential resets need `manage_quality`; other account work needs
//! `manage_users`. Sensitive changes reauthenticate the actor, resets end the
//! target's sessions, and last-holder guards preserve both duties.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UserId};
use lis_store::CommandContext;
use secrecy::SecretString;
use serde::Deserialize;
use ts_rs::TS;

use crate::account::{Username, parse_display_name, parse_reason};
use crate::actions;
use crate::duties::{
    acting_user, ensure_not_last, refuse_own_account, require_credential_reset_authority,
    require_role_authority,
};
use crate::error::{Attempt, AuthError, Fields};
use crate::password::{self, NewPassword};
use crate::queries::{UserSummary, user_summary};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
use crate::repo::{self, NewUser, UserRow, UserStatus};
use crate::roles::RoleCode;
use crate::sessions;
use crate::throttle;

/// Input of `create_user`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateUserInput {
    /// Sign-in name.
    pub username: String,
    /// The person's name.
    pub display_name: String,
    /// Roles to grant; may be empty.
    pub roles: Vec<RoleCode>,
    /// Replaced by the user at first sign-in.
    #[ts(type = "string")]
    pub temporary_password: SecretString,
    /// Fresh confirmation of the administrator's selected method (REQ-IDN-004).
    pub password: Reauthentication,
}

/// Input of `set_user_roles`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetUserRolesInput {
    /// The account.
    pub user_id: UserId,
    /// The complete new set of roles.
    pub roles: Vec<RoleCode>,
    /// The revision the administrator saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why.
    pub reason: String,
    /// Fresh confirmation of the acting user's selected method (REQ-IDN-004).
    pub password: Reauthentication,
}

/// Input of `disable_user` and `enable_user`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ChangeUserStatusInput {
    /// The account.
    pub user_id: UserId,
    /// The revision the administrator saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why.
    pub reason: String,
}

/// Input of `reset_user_credentials`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResetUserCredentialsInput {
    /// The account.
    pub user_id: UserId,
    /// Replaced by the user at next sign-in.
    #[ts(type = "string")]
    pub temporary_password: SecretString,
    /// Also remove the authenticator, so the user enrols a new one.
    pub reset_mfa: bool,
    /// The revision the administrator saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why.
    pub reason: String,
    /// Fresh confirmation of the administrator's selected method (REQ-IDN-004).
    pub password: Reauthentication,
}

/// A validated new account with its password already hashed.
pub(crate) struct ValidAccount {
    pub(crate) username: Username,
    pub(crate) display_name: String,
    pub(crate) password_hash: String,
}

pub(crate) fn validate_account(
    username: &str,
    display_name: &str,
    password: SecretString,
    password_field: &str,
) -> Result<ValidAccount, AuthError> {
    let mut fields = Fields::default();
    let parsed_username = fields.check("username", Username::parse(username));
    let display_name = fields.check("displayName", parse_display_name(display_name));
    let password = fields.check(password_field, NewPassword::parse(password, username));
    let (Some(username), Some(display_name), Some(password)) =
        (parsed_username, display_name, password)
    else {
        return Err(fields.into_error());
    };
    Ok(ValidAccount {
        username,
        display_name,
        password_hash: password::hash(&password)?,
    })
}

/// Inserts the account with its roles and returns its ID.
pub(crate) fn insert_account(
    ctx: &CommandContext<'_>,
    account: &ValidAccount,
    roles: &BTreeSet<RoleCode>,
    must_change_password: bool,
) -> Result<UserId, AuthError> {
    let id = UserId::generate();
    insert_account_at(ctx, account, roles, must_change_password, id)?;
    Ok(id)
}

pub(crate) fn insert_account_at(
    ctx: &CommandContext<'_>,
    account: &ValidAccount,
    roles: &BTreeSet<RoleCode>,
    must_change_password: bool,
    id: UserId,
) -> Result<(), AuthError> {
    let user = NewUser {
        id,
        username: &account.username,
        display_name: &account.display_name,
        password_hash: &account.password_hash,
        must_change_password,
    };
    repo::insert(ctx.tx(), &user, ctx.now())?;
    for &role in roles {
        repo::grant(ctx.tx(), id, role, acting_user(ctx), ctx.now())?;
    }
    Ok(())
}

fn codes(roles: &BTreeSet<RoleCode>) -> Vec<&'static str> {
    roles.iter().map(|role| role.as_str()).collect()
}

pub(crate) fn account_created_event(
    action: &'static str,
    id: UserId,
    account: &ValidAccount,
    roles: &BTreeSet<RoleCode>,
) -> AuditEvent {
    AuditEvent::new(Stream::Security, action, Outcome::Succeeded)
        .record("user", id)
        .new_revision(1)
        .detail(
            "username",
            AuditDetail::workforce_username(account.username.as_str()),
        )
        .detail("roles", AuditDetail::codes(codes(roles)))
}

/// Creates an account with a temporary password, after the administrator's
/// password. Clinical roles also need `manage_quality`.
pub fn create_user(
    ctx: &mut CommandContext<'_>,
    input: CreateUserInput,
) -> Result<Attempt<UserSummary>, AuthError> {
    ctx.require(Capability::ManageUsers)?;
    let roles: BTreeSet<RoleCode> = input.roles.into_iter().collect();
    require_role_authority(ctx, &roles)?;
    let account = validate_account(
        &input.username,
        &input.display_name,
        input.temporary_password,
        "temporaryPassword",
    )?;
    if let Attempt::Rejected(rejection) =
        reauthenticate(ctx, &input.password, ReauthPurpose::AccountCreation)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    let id = insert_account(ctx, &account, &roles, true)?;
    ctx.audit(account_created_event(
        actions::USER_CREATE,
        id,
        &account,
        &roles,
    ))?;
    user_summary(ctx.tx(), id, ctx.now()).map(Attempt::Accepted)
}

/// Loads the target of an administrative change after checking the reason and revision.
fn load_for_change(
    ctx: &CommandContext<'_>,
    user_id: UserId,
    expected_revision: i64,
    reason: &str,
) -> Result<(UserRow, String), AuthError> {
    let reason = parse_reason(reason).map_err(|problem| problem.invalid("reason"))?;
    let user = repo::load(ctx.tx(), user_id)?;
    user.ensure_revision(expected_revision)?;
    Ok((user, reason))
}

/// The audit event of an administrative change of `user` that produced `revision`.
fn change_event(action: &'static str, user: &UserRow, revision: i64, reason: String) -> AuditEvent {
    AuditEvent::new(Stream::Security, action, Outcome::Succeeded)
        .record("user", user.id)
        .old_revision(user.revision)
        .new_revision(revision)
        .reason(reason)
}

/// Replaces the roles of another account after the actor's password; revoked
/// grants stay on record. Each added or removed role needs its
/// [`RoleCode::authority`]; an unchanged set needs `manage_users`.
pub fn set_user_roles(
    ctx: &mut CommandContext<'_>,
    input: SetUserRolesInput,
) -> Result<Attempt<UserSummary>, AuthError> {
    let current: BTreeSet<RoleCode> = repo::current_roles(ctx.tx(), input.user_id)?
        .into_iter()
        .collect();
    let wanted: BTreeSet<RoleCode> = input.roles.into_iter().collect();
    let removed: BTreeSet<RoleCode> = current.difference(&wanted).copied().collect();
    let added: BTreeSet<RoleCode> = wanted.difference(&current).copied().collect();
    let changed: BTreeSet<RoleCode> = removed.union(&added).copied().collect();
    if changed.is_empty() {
        ctx.require(Capability::ManageUsers)?;
    }
    require_role_authority(ctx, &changed)?;
    if let Some(refused) = refuse_own_account(ctx, actions::USER_ROLES_SET, input.user_id)? {
        return Ok(refused);
    }
    let (user, reason) =
        load_for_change(ctx, input.user_id, input.expected_revision, &input.reason)?;
    if removed.contains(&RoleCode::Administrator) {
        ensure_not_last(ctx, &user, RoleCode::Administrator, || {
            AuthError::LastAdministrator
        })?;
    }
    if let Attempt::Rejected(rejection) =
        reauthenticate(ctx, &input.password, ReauthPurpose::RoleChange)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    for &role in &removed {
        repo::revoke(ctx.tx(), user.id, role, acting_user(ctx), ctx.now())?;
    }
    for &role in &added {
        repo::grant(ctx.tx(), user.id, role, acting_user(ctx), ctx.now())?;
    }
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    let event = change_event(actions::USER_ROLES_SET, &user, revision, reason)
        .detail("added", AuditDetail::codes(codes(&added)))
        .detail("removed", AuditDetail::codes(codes(&removed)));
    ctx.audit(event)?;
    user_summary(ctx.tx(), user.id, ctx.now()).map(Attempt::Accepted)
}

/// Disables an account so it can no longer sign in.
pub fn disable_user(
    ctx: &mut CommandContext<'_>,
    input: ChangeUserStatusInput,
) -> Result<UserSummary, AuthError> {
    change_status(ctx, input, UserStatus::Disabled)
}

/// Enables a disabled account again.
pub fn enable_user(
    ctx: &mut CommandContext<'_>,
    input: ChangeUserStatusInput,
) -> Result<UserSummary, AuthError> {
    change_status(ctx, input, UserStatus::Active)
}

fn change_status(
    ctx: &mut CommandContext<'_>,
    input: ChangeUserStatusInput,
    status: UserStatus,
) -> Result<UserSummary, AuthError> {
    ctx.require(Capability::ManageUsers)?;
    let (user, reason) =
        load_for_change(ctx, input.user_id, input.expected_revision, &input.reason)?;
    if user.status == status {
        return Err(AuthError::StatusUnchanged);
    }
    let action = match status {
        UserStatus::Disabled => {
            ensure_not_last(ctx, &user, RoleCode::Administrator, || {
                AuthError::LastAdministrator
            })?;
            // Otherwise disabling the quality manager would reopen the
            // bootstrap in which `manage_users` grants that role alone.
            ensure_not_last(ctx, &user, RoleCode::QualityManager, || {
                AuthError::LastQualityManager
            })?;
            actions::USER_DISABLE
        }
        UserStatus::Active => actions::USER_ENABLE,
    };
    repo::set_status(ctx.tx(), user.id, status)?;
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    ctx.audit(change_event(action, &user, revision, reason))?;
    user_summary(ctx.tx(), user.id, ctx.now())
}

/// Sets a temporary password for another account after the administrator's
/// password, optionally removes the authenticator, clears any lockout of the
/// username and ends the account's open sessions. The engine also discards
/// any sign-in challenge in progress.
pub fn reset_user_credentials(
    ctx: &mut CommandContext<'_>,
    input: ResetUserCredentialsInput,
) -> Result<Attempt<UserSummary>, AuthError> {
    require_credential_reset_authority(ctx, input.user_id)?;
    if let Some(refused) = refuse_own_account(ctx, actions::CREDENTIALS_RESET, input.user_id)? {
        return Ok(refused);
    }
    let (user, reason) =
        load_for_change(ctx, input.user_id, input.expected_revision, &input.reason)?;
    if user.authentication_method == crate::AuthenticationMethod::SecurityKey {
        return Err(AuthError::SecurityKeyRequired);
    }
    let temporary = NewPassword::parse(input.temporary_password, user.username.as_str())
        .map_err(|problem| problem.invalid("temporaryPassword"))?;
    if let Attempt::Rejected(rejection) =
        reauthenticate(ctx, &input.password, ReauthPurpose::CredentialReset)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    repo::set_password(ctx.tx(), user.id, &password::hash(&temporary)?, true)?;
    crate::unlock_pin::remove_after_reset(ctx, user.id)?;
    if input.reset_mfa {
        repo::set_totp(ctx.tx(), user.id, None)?;
    }
    throttle::clear(ctx.tx(), user.username.as_str())?;
    sessions::end_sessions_of(ctx, user.id)?;
    let revision = repo::bump_revision(ctx.tx(), user.id, ctx.now())?;
    let event = change_event(actions::CREDENTIALS_RESET, &user, revision, reason)
        .detail("mfa_reset", AuditDetail::boolean(input.reset_mfa));
    ctx.audit(event)?;
    user_summary(ctx.tx(), user.id, ctx.now()).map(Attempt::Accepted)
}
