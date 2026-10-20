//! Who acts, rechecked inside the command's own transaction (NFR-08; brief
//! §3.4 rows 1–2): the session's principal was resolved before the write
//! began, so release and withdrawal reread the user's grants as of now. The
//! laboratory's separation-of-duties policy is read the same way, when one
//! person does both steps of a duty ([`authorize_self`], ADR 0015).

use lis_auth::{Duty, SelfApprovalRefused, SessionUser};
use lis_core::{Actor, Capability, Denied, UserId};
use lis_store::{CommandContext, StoreError};
use rusqlite::Connection;

use crate::error::{ReportingError, gate};

/// The signed-in user, or [`ReportingError::UserRequired`].
pub(crate) fn user_id(ctx: &CommandContext<'_>) -> Result<UserId, ReportingError> {
    match ctx.principal().actor() {
        Actor::User { user_id, .. } => Ok(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => Err(ReportingError::UserRequired),
    }
}

/// Requires `capability` of the caller (row 1), then of the user's grants
/// as stored now (row 2). A grant revoked after the session's principal was
/// resolved is a denial, audited by the engine like any other.
pub(crate) fn current_user(
    ctx: &mut CommandContext<'_>,
    capability: Capability,
) -> Result<SessionUser, ReportingError> {
    ctx.require(capability)?;
    let user = lis_auth::session_user(ctx.tx(), user_id(ctx)?)?;
    if !user.capabilities().contains(&capability) {
        return Err(StoreError::Denied(Denied { capability }).into());
    }
    Ok(user)
}

/// The second step of `duty` by the person who did its first: releasing a
/// report whose results they reviewed (REQ-RES-005) or verifying a destination
/// they created (REQ-RPT-012). Allowed only when the laboratory's
/// separation-of-duties policy in force in `conn`'s transaction allows it and
/// the person confirmed it (ADR 0015). Refused on the gate `gate_code` while
/// the policy needs a second person, and on the `selfApproval` field when not
/// confirmed.
pub(crate) fn authorize_self(
    conn: &Connection,
    duty: Duty,
    confirmed: bool,
    gate_code: &str,
) -> Result<(), ReportingError> {
    let Err(refused) = lis_auth::current_separation_policy(conn)?.authorize_self(duty, confirmed)
    else {
        return Ok(());
    };
    Err(match refused {
        SelfApprovalRefused::NotAllowed(_) => {
            ReportingError::GateBlocked(vec![gate(gate_code, &refused.to_string())])
        }
        SelfApprovalRefused::NotConfirmed(duty) => {
            ReportingError::Invalid(duty.confirmation_field_error())
        }
    })
}
