//! Separation-of-duties guards of user administration (REQ-IDN-004; security
//! review 2026-10-13): the capability each role change needs, the refusal of
//! administration of one's own account, and the last-holder guards.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Actor, Capability, UserId};
use lis_store::CommandContext;

use crate::error::{Attempt, AuthError, Rejection};
use crate::repo::{self, UserRow, UserStatus};
use crate::roles::{RoleCode, quality_bootstrap};

/// The acting user, if a person (not first-run setup) runs the command.
pub(crate) fn acting_user(ctx: &CommandContext<'_>) -> Option<UserId> {
    match ctx.principal().actor() {
        Actor::User { user_id, .. } => Some(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => None,
    }
}

/// Requires the capability each of `changed` roles needs (REQ-IDN-004).
pub(crate) fn require_role_authority(
    ctx: &mut CommandContext<'_>,
    changed: &BTreeSet<RoleCode>,
) -> Result<(), AuthError> {
    let bootstrap = changed.contains(&RoleCode::QualityManager) && quality_bootstrap(ctx.tx())?;
    for role in changed {
        ctx.require(role.authority(bootstrap))?;
    }
    Ok(())
}

/// Credential reset always needs account administration. If the target has
/// any clinical role it also needs quality authority, so an administrator
/// alone cannot take over a releaser's credentials (Q-37).
pub(crate) fn require_credential_reset_authority(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
) -> Result<(), AuthError> {
    ctx.require(Capability::ManageUsers)?;
    if repo::current_roles(ctx.tx(), user_id)?
        .into_iter()
        .any(RoleCode::is_clinical)
    {
        ctx.require(Capability::ManageQuality)?;
    }
    Ok(())
}

/// Refuses administration of the actor's own account, with a committed
/// `denied` event: one person cannot raise their own authority or take over
/// their own sign-in through administration.
pub(crate) fn refuse_own_account<T>(
    ctx: &CommandContext<'_>,
    action: &'static str,
    user_id: UserId,
) -> Result<Option<Attempt<T>>, AuthError> {
    if acting_user(ctx) != Some(user_id) {
        return Ok(None);
    }
    ctx.audit(
        AuditEvent::new(Stream::Security, action, Outcome::Denied)
            .record("user", user_id)
            .detail("rule", AuditDetail::code("own_account")),
    )?;
    Ok(Some(Attempt::Rejected(Rejection::OwnAccount)))
}

/// Lock-out guard: refuses a change that would leave no active holder of
/// `role` other than `user`, failing with `error`.
pub(crate) fn ensure_not_last(
    ctx: &CommandContext<'_>,
    user: &UserRow,
    role: RoleCode,
    error: fn() -> AuthError,
) -> Result<(), AuthError> {
    let is_active_holder = user.status == UserStatus::Active
        && repo::current_roles(ctx.tx(), user.id)?.contains(&role);
    if is_active_holder && repo::active_holders(ctx.tx(), role, Some(user.id))? == 0 {
        return Err(error());
    }
    Ok(())
}
