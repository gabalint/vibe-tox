//! A laboratory that one person runs (ADR 0015). CLIA lets a qualified laboratory
//! director also act as technical supervisor, general supervisor and testing personnel
//! (42 CFR 493.1445(a)), so that person needs every role, and every duty must be
//! allowed to them alone. Nobody grants roles to themselves otherwise, so this is the
//! only way, and only while they are the laboratory's one active account: at first-run
//! setup, or later with [`designate_one_person_laboratory`]. Each self-approval still
//! needs its own confirmation ([`crate::SeparationPolicy::authorize_self`]).

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::account::parse_reason;
use crate::actions;
use crate::duties::acting_user;
use crate::error::{Attempt, AuthError};
use crate::queries::{UserSummary, user_summary};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
use crate::repo;
use crate::roles::RoleCode;
use crate::separation::{ONE_PERSON_SEPARATION_POLICY, record_separation_policy};

/// The reason recorded when first-run setup chooses a one-person laboratory.
pub(crate) const FIRST_RUN_REASON: &str = "First-run setup: one person runs this laboratory.";

/// Input of `designate_one_person_laboratory`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DesignateOnePersonLaboratoryInput {
    /// Why, for example that the director runs the laboratory alone.
    pub reason: String,
    /// Fresh confirmation of the acting user's selected method (REQ-IDN-004).
    pub password: Reauthentication,
}

/// Gives the laboratory's one active account every role and allows every duty to one
/// person (`manage_users`, fresh password, reason). Refused while any other account is
/// active. Audited as `auth.user_roles_set` with the rule `one_person_laboratory`, and
/// `auth.set_separation_policy`.
pub fn designate_one_person_laboratory(
    ctx: &mut CommandContext<'_>,
    input: DesignateOnePersonLaboratoryInput,
) -> Result<Attempt<UserSummary>, AuthError> {
    ctx.require(Capability::ManageUsers)?;
    let Some(user_id) = acting_user(ctx) else {
        return Err(AuthError::NotOnlyActiveAccount);
    };
    let reason = parse_reason(&input.reason).map_err(|problem| problem.invalid("reason"))?;
    if repo::other_active_accounts(ctx.tx(), user_id)? > 0 {
        return Err(AuthError::NotOnlyActiveAccount);
    }
    if let Attempt::Rejected(rejection) =
        reauthenticate(ctx, &input.password, ReauthPurpose::OnePersonLaboratory)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    let user = repo::load(ctx.tx(), user_id)?;
    let current: BTreeSet<RoleCode> = repo::current_roles(ctx.tx(), user_id)?
        .into_iter()
        .collect();
    let added: Vec<&'static str> = RoleCode::ALL
        .into_iter()
        .filter(|role| !current.contains(role))
        .map(|role| {
            repo::grant(ctx.tx(), user_id, role, Some(user_id), ctx.now()).map(|()| role.as_str())
        })
        .collect::<Result<_, _>>()?;
    let revision = repo::bump_revision(ctx.tx(), user_id, ctx.now())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::USER_ROLES_SET,
            Outcome::Succeeded,
        )
        .record("user", user_id)
        .old_revision(user.revision)
        .new_revision(revision)
        .reason(reason.clone())
        .detail("rule", AuditDetail::code("one_person_laboratory"))
        .detail("added", AuditDetail::codes(added)),
    )?;
    record_separation_policy(ctx, ONE_PERSON_SEPARATION_POLICY, reason)?;
    user_summary(ctx.tx(), user_id, ctx.now()).map(Attempt::Accepted)
}

/// First-run setup of a one-person laboratory: the separation policy for the new and
/// only account, which already holds every role.
pub(crate) fn allow_first_run_account(ctx: &CommandContext<'_>) -> Result<(), AuthError> {
    record_separation_policy(
        ctx,
        ONE_PERSON_SEPARATION_POLICY,
        FIRST_RUN_REASON.to_owned(),
    )
}

/// The roles of a new first account: every role for a one-person laboratory, otherwise
/// the administrator alone.
pub(crate) fn first_run_roles(one_person: bool) -> BTreeSet<RoleCode> {
    if one_person {
        RoleCode::ALL.into_iter().collect()
    } else {
        BTreeSet::from([RoleCode::Administrator])
    }
}
