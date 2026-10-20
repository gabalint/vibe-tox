//! Lifecycle steps shared by method and orderable versions (REQ-ANA-002).
//!
//! Each public command calls one `begin_*` function first (capability check,
//! acting user, load, state and revision checks), then its kind-specific
//! checks and writes, then the matching `finish_*` function (lifecycle write
//! and audit), all in the command's transaction.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::{Duty, SelfApprovalRefused};
use lis_core::{Actor, Capability, Id, Principal, UserId};
use lis_store::CommandContext;
use rusqlite::Connection;

use crate::error::CatalogError;
use crate::lifecycle::{VersionRef, VersionSummary, Versioned};
use crate::repo::lifecycle as repo;

/// A loaded version and the user acting on it.
pub(crate) struct Pending<K: Versioned> {
    pub(crate) actor: UserId,
    pub(crate) parent: Id<K::Parent>,
    pub(crate) version: VersionSummary<K>,
    /// The approver wrote the draft, as the separation-of-duties policy allowed
    /// (ADR 0015); set only by [`begin_approval`].
    pub(crate) self_approval: bool,
}

/// Configuration is authored and approved by people, so authorship and
/// separation of duties can be checked.
pub(crate) fn acting_user(principal: &Principal) -> Result<UserId, CatalogError> {
    match principal.actor() {
        Actor::User { user_id, .. } => Ok(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => Err(CatalogError::UserRequired),
    }
}

fn load<K: Versioned>(
    ctx: &mut CommandContext<'_>,
    capability: Capability,
    id: Id<K>,
) -> Result<Pending<K>, CatalogError> {
    ctx.require(capability)?;
    let actor = acting_user(ctx.principal())?;
    let stored =
        repo::version_state::<K>(ctx.tx(), id)?.ok_or(CatalogError::NotFound(K::RECORD_TYPE))?;
    Ok(Pending {
        actor,
        parent: stored.parent,
        version: stored.version,
        self_approval: false,
    })
}

/// Authorizes a new draft of `parent`; returns its author and version number.
pub(crate) fn begin_draft<K: Versioned>(
    ctx: &mut CommandContext<'_>,
    parent: Id<K::Parent>,
) -> Result<(UserId, i64), CatalogError> {
    ctx.require(Capability::ManageConfiguration)?;
    let author = acting_user(ctx.principal())?;
    if !repo::parent_exists::<K>(ctx.tx(), parent)? {
        return Err(CatalogError::NotFound(K::PARENT_RECORD_TYPE));
    }
    if repo::has_draft::<K>(ctx.tx(), parent)? {
        return Err(CatalogError::DraftExists);
    }
    Ok((author, repo::next_version::<K>(ctx.tx(), parent)?))
}

/// Audits a new draft written by the caller.
pub(crate) fn finish_draft<K: Versioned>(
    ctx: &CommandContext<'_>,
    id: Id<K>,
    parent: Id<K::Parent>,
    version: i64,
) -> Result<VersionRef<K>, CatalogError> {
    ctx.audit(
        AuditEvent::new(Stream::Clinical, K::DRAFT_ACTION, Outcome::Succeeded)
            .record(K::RECORD_TYPE, id)
            .new_revision(1)
            .detail("parent_id", AuditDetail::id(parent))
            .detail("version", AuditDetail::integer(version)),
    )?;
    Ok(VersionRef {
        id,
        version,
        revision: 1,
    })
}

/// Authorizes an edit: a draft, at the expected revision, by its author.
pub(crate) fn begin_edit<K: Versioned>(
    ctx: &mut CommandContext<'_>,
    id: Id<K>,
    expected_revision: i64,
) -> Result<Pending<K>, CatalogError> {
    let pending = load(ctx, Capability::ManageConfiguration, id)?;
    pending.version.lifecycle.ensure_draft()?;
    pending
        .version
        .lifecycle
        .ensure_revision(expected_revision)?;
    // Only the author edits, so an approver can never have edited the draft.
    if pending.version.lifecycle.created_by != pending.actor {
        return Err(CatalogError::NotDraftAuthor);
    }
    Ok(pending)
}

/// Bumps the revision of an edited draft and audits the edit.
pub(crate) fn finish_edit<K: Versioned>(
    ctx: &CommandContext<'_>,
    pending: &Pending<K>,
) -> Result<VersionRef<K>, CatalogError> {
    let old = pending.version.lifecycle.revision;
    repo::set_revision(ctx.tx(), pending.version.id, old + 1)?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, K::UPDATE_ACTION, Outcome::Succeeded)
            .record(K::RECORD_TYPE, pending.version.id)
            .old_revision(old)
            .new_revision(old + 1)
            .detail("version", AuditDetail::integer(pending.version.version)),
    )?;
    Ok(VersionRef {
        id: pending.version.id,
        version: pending.version.version,
        revision: old + 1,
    })
}

/// Authorizes an approval: a draft, at the expected revision, approved by
/// someone other than its author, or by its author as the laboratory's
/// separation-of-duties policy allows, with `self_approval` confirmed.
pub(crate) fn begin_approval<K: Versioned>(
    ctx: &mut CommandContext<'_>,
    id: Id<K>,
    expected_revision: i64,
    self_approval: bool,
) -> Result<Pending<K>, CatalogError> {
    let mut pending = load(ctx, Capability::ApproveConfiguration, id)?;
    pending.version.lifecycle.ensure_draft()?;
    pending
        .version
        .lifecycle
        .ensure_revision(expected_revision)?;
    if pending.version.lifecycle.created_by == pending.actor {
        authorize_self_approval(ctx.tx(), self_approval, || CatalogError::SelfApproval)?;
        pending.self_approval = true;
    }
    Ok(pending)
}

/// REQ-ANA-009, REQ-OPS-015, ADR 0015: the author of configuration approves it
/// only when the laboratory's separation-of-duties policy in force allows it and
/// they `confirmed` it for this approval. Read inside the command's transaction.
/// Refuses with `not_allowed` while the policy needs a second person, and with
/// the `selfApproval` field error while the confirmation is missing.
pub(crate) fn authorize_self_approval(
    conn: &Connection,
    confirmed: bool,
    not_allowed: impl FnOnce() -> CatalogError,
) -> Result<(), CatalogError> {
    lis_auth::current_separation_policy(conn)?
        .authorize_self(Duty::ConfigurationApproval, confirmed)
        .map_err(|refused| match refused {
            SelfApprovalRefused::NotAllowed(_) => not_allowed(),
            SelfApprovalRefused::NotConfirmed(duty) => {
                CatalogError::Invalid(duty.confirmation_field_error())
            }
        })
}

/// Approves the draft. The previous current version's effective period ends
/// now and records this version as its replacement.
pub(crate) fn finish_approval<K: Versioned>(
    ctx: &CommandContext<'_>,
    pending: &Pending<K>,
) -> Result<VersionRef<K>, CatalogError> {
    let id = pending.version.id;
    if let Some(previous) = repo::current::<K>(ctx.tx(), pending.parent)? {
        let old = previous.lifecycle.revision;
        repo::supersede::<K>(ctx.tx(), previous.id, old + 1, ctx.now(), id)?;
        ctx.audit(
            AuditEvent::new(Stream::Clinical, K::SUPERSEDE_ACTION, Outcome::Succeeded)
                .record(K::RECORD_TYPE, previous.id)
                .old_revision(old)
                .new_revision(old + 1)
                .detail("version", AuditDetail::integer(previous.version))
                .detail("replaced_by", AuditDetail::id(id)),
        )?;
    }
    let old = pending.version.lifecycle.revision;
    let mut event = AuditEvent::new(Stream::Clinical, K::APPROVE_ACTION, Outcome::Succeeded)
        .record(K::RECORD_TYPE, id)
        .old_revision(old)
        .new_revision(old + 1)
        .detail("version", AuditDetail::integer(pending.version.version));
    if pending.self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    // The schema transition guard verifies this attributable event before it
    // permits draft -> approved. A later failure rolls both back together.
    repo::approve::<K>(ctx.tx(), id, old + 1, pending.actor, ctx.now())?;
    Ok(VersionRef {
        id,
        version: pending.version.version,
        revision: old + 1,
    })
}

/// Authorizes a retirement: an approved version at the expected revision.
pub(crate) fn begin_retirement<K: Versioned>(
    ctx: &mut CommandContext<'_>,
    id: Id<K>,
    expected_revision: i64,
) -> Result<Pending<K>, CatalogError> {
    let pending = load(ctx, Capability::ApproveConfiguration, id)?;
    pending.version.lifecycle.ensure_approved()?;
    pending
        .version
        .lifecycle
        .ensure_revision(expected_revision)?;
    Ok(pending)
}

/// Retires the version with a reason; its content and history stay unchanged.
pub(crate) fn finish_retirement<K: Versioned>(
    ctx: &CommandContext<'_>,
    pending: &Pending<K>,
    reason: String,
) -> Result<VersionRef<K>, CatalogError> {
    let id = pending.version.id;
    let old = pending.version.lifecycle.revision;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, K::RETIRE_ACTION, Outcome::Succeeded)
            .record(K::RECORD_TYPE, id)
            .old_revision(old)
            .new_revision(old + 1)
            .reason(reason.clone())
            .detail("version", AuditDetail::integer(pending.version.version)),
    )?;
    // The schema transition guard verifies this attributable event before it
    // permits approved -> retired. A later failure rolls both back together.
    repo::retire::<K>(ctx.tx(), id, old + 1, pending.actor, ctx.now(), &reason)?;
    Ok(VersionRef {
        id,
        version: pending.version.version,
        revision: old + 1,
    })
}
