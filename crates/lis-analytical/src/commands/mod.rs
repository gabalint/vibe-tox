//! State-changing use cases. Each checks its capability first, validates its
//! input, rechecks its gates inside the transaction, changes state and
//! appends a clinical-stream audit event in the same transaction.

mod au_sources;
pub(crate) mod components;
pub(crate) mod conflicts;
pub(crate) mod deviations;
pub(crate) mod file_sources;
pub(crate) mod lots;
pub(crate) mod patient;
pub(crate) mod performing_site;
pub(crate) mod qc;
pub(crate) mod readiness;
pub(crate) mod runs;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_catalog::{MethodVersionId, MethodVersionView};
use lis_core::{Actor, FieldError, GateReason, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::Connection;

use crate::error::{AnalyticalError, gate};
use crate::model::{CivilDate, LotState, is_expired};
pub(crate) use crate::queries::instrument_name;
use crate::repo::lots::LotRow;

pub(crate) const CODE_MAX: usize = 64;
pub(crate) const NAME_MAX: usize = 200;
pub(crate) const TEXT_MAX: usize = 1000;

/// The stored actor text of the command's principal.
fn actor(ctx: &CommandContext<'_>) -> String {
    ctx.principal().actor().id_text()
}

/// Approvals and QC decisions are made by a signed-in person, never a system principal.
fn require_user(ctx: &CommandContext<'_>) -> Result<String, AnalyticalError> {
    match ctx.principal().actor() {
        Actor::User { user_id, .. } => Ok(user_id.to_string()),
        Actor::Instrument { .. } | Actor::System { .. } => Err(AnalyticalError::UserRequired),
    }
}

/// A succeeded clinical-stream audit event.
fn clinical(action: &'static str) -> AuditEvent {
    AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
}

/// A required reason, as entered.
fn reason(value: &str) -> Result<String, FieldError> {
    lis_core::required_text("reason", value, TEXT_MAX)
}

/// Approved versions remain usable for accepted pinned work after routine
/// supersession (WFREV-02). Explicit retirement still blocks use.
fn approved_method(
    conn: &Connection,
    id: MethodVersionId,
) -> Result<(MethodVersionView, Option<GateReason>), AnalyticalError> {
    let method = lis_catalog::get_method_version(conn, id)?;
    let blocked = (method.lifecycle.status != lis_catalog::VersionStatus::Approved).then(|| {
        gate(
            "method_not_current",
            "The method version is not approved or has been explicitly retired.",
        )
    });
    Ok((method, blocked))
}

/// Why a lot cannot be used for new work at `now`: withdrawn or expired (REQ-ANA-007).
fn lot_gates(lot: &LotRow, now: UtcTimestamp) -> Result<Vec<GateReason>, AnalyticalError> {
    let mut gates = Vec::new();
    if lot.state == LotState::Withdrawn {
        gates.push(gate("lot_withdrawn", "A lot was withdrawn from use."));
    }
    if is_expired(CivilDate::parse("expiresOn", &lot.expires_on)?, now) {
        gates.push(gate("lot_expired", "A lot is past its expiry date."));
    }
    Ok(gates)
}
