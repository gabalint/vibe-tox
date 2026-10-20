//! Identity hold review (REQ-IDN-001). Merging is REQ-IDN-002, later.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{IdentityError, ensure_revision};
use crate::model::{ActorRef, IdentityHoldId, REASON_MAX, required_text};
use crate::repo::holds as repo;

/// Input of [`confirm_distinct_subjects`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ConfirmDistinctSubjects {
    /// The open possible-duplicate hold.
    pub hold_id: IdentityHoldId,
    /// The hold revision the reviewer saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The reviewer's evidence that the subjects are different people.
    pub reason: String,
}

/// Result of [`confirm_distinct_subjects`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IdentityHoldResolved {
    /// The hold.
    pub hold_id: IdentityHoldId,
    /// Its new revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// Resolves an open possible-duplicate hold as "confirmed distinct", with a reason.
pub fn confirm_distinct_subjects(
    ctx: &mut CommandContext<'_>,
    input: ConfirmDistinctSubjects,
) -> Result<IdentityHoldResolved, IdentityError> {
    ctx.require(Capability::ManageQuality)?;
    let actor = ActorRef::user(ctx.principal())?;
    let reason = required_text("reason", &input.reason, REASON_MAX)?;
    let hold = repo::hold_state(ctx.tx(), input.hold_id)?
        .ok_or(IdentityError::NotFound("identity hold"))?;
    if !hold.open {
        return Err(IdentityError::HoldResolved);
    }
    ensure_revision(hold.revision, input.expected_revision)?;
    if !repo::evidence_is_current(ctx.tx(), input.hold_id)? {
        return Err(IdentityError::RevisionConflict);
    }
    let revision = hold.revision + 1;
    repo::resolve_confirmed_distinct(
        ctx.tx(),
        input.hold_id,
        revision,
        ctx.now(),
        &reason,
        &actor,
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.confirm_distinct",
            Outcome::Succeeded,
        )
        .record("identity_hold", input.hold_id)
        .old_revision(hold.revision)
        .new_revision(revision)
        .reason(reason)
        .detail("subject_id", AuditDetail::id(hold.subject_id)),
    )?;
    Ok(IdentityHoldResolved {
        hold_id: input.hold_id,
        revision,
    })
}
