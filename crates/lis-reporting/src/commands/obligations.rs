//! Delivery obligations and attested attempts (REQ-RPT-003 local,
//! REQ-RPT-011, REQ-RPT-012, REQ-RPT-013). Every attempt and new obligation
//! rechecks the destination and the revision in this transaction; a retry
//! never creates a report.

use crate::queries::obligation::obligation_view;
use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp};
use lis_identity::ClientId;
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::user_id;
use crate::commands::routing::handed_off;
use crate::error::{ReportingError, gate, invalid, optional, required};
use crate::impact;
use crate::model::{
    DeliveryAttemptId, DeliveryEvidence, DeliveryFailureCode, DestinationId,
    ObligationClosureCause, ObligationId, ObligationKind, ObligationOrigin, ReportRevisionId,
    RevisionAvailability,
};
use crate::policy::DELIVERY_PAUSED_WHILE_IMPACT_OPEN;
use crate::queries::delivery::{DeliveryContext, order_client};
use crate::repo::delivery::{self, AttemptRow, DestinationRow, ObligationRow};
use crate::repo::reports::{self, RevisionRow};
use crate::views::ObligationView;

/// Input of `add_delivery_obligation`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AddDeliveryObligationInput {
    /// The current revision.
    pub revision_id: ReportRevisionId,
    /// A verified, active destination of the order's client.
    pub destination_id: DestinationId,
    /// Why.
    pub reason: String,
}

/// Input of `redirect_delivery_obligation`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RedirectDeliveryObligationInput {
    /// The open obligation.
    pub obligation_id: ObligationId,
    /// A verified, active destination of the order's client.
    pub destination_id: DestinationId,
    /// Why.
    pub reason: String,
}

/// Input of `record_delivery_attempt`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordDeliveryAttemptInput {
    /// The open obligation.
    pub obligation_id: ObligationId,
    /// What staff attest.
    pub evidence: DeliveryEvidence,
    /// When it happened; now when absent. Never in the future.
    #[serde(default)]
    #[ts(optional)]
    pub observed_at: Option<UtcTimestamp>,
    /// Required for, and only for, a failed attempt.
    #[serde(default)]
    #[ts(optional)]
    pub failure_code: Option<DeliveryFailureCode>,
    /// A note, as entered.
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
}

/// Adds an obligation for a current revision to another destination.
pub fn add_delivery_obligation(
    ctx: &mut CommandContext<'_>,
    input: AddDeliveryObligationInput,
) -> Result<ObligationView, ReportingError> {
    ctx.require(Capability::DeliverReports)?;
    let by = user_id(ctx)?.to_string();
    let reason = required("reason", &input.reason, 1000)?;
    let revision = reports::revision(ctx.tx(), input.revision_id)?
        .ok_or(ReportingError::NotFound("report revision"))?;
    check_revision(&revision, ObligationKind::Report)?;
    usable_destination(
        ctx.tx(),
        input.destination_id,
        order_client(ctx.tx(), revision.report_id)?,
    )?;
    ensure_new(
        ctx.tx(),
        &revision,
        input.destination_id,
        ObligationKind::Report,
    )?;
    let obligation = ObligationRow {
        id: ObligationId::generate(),
        revision_id: revision.id,
        destination_id: input.destination_id,
        kind: ObligationKind::Report,
        origin: ObligationOrigin::Added,
        origin_reason: Some(reason.clone()),
        redirected_from_id: None,
        created_at: ctx.now(),
        closure: None,
    };
    delivery::insert_obligation(ctx.tx(), &obligation, &by)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::ADD_OBLIGATION,
            Outcome::Succeeded,
        )
        .record("delivery_obligation", obligation.id)
        .detail("revision_id", AuditDetail::id(revision.id))
        .detail("destination_id", AuditDetail::id(input.destination_id))
        .reason(reason),
    )?;
    view(ctx.tx(), &obligation)
}

/// Moves an open obligation to another destination: a new obligation of the
/// same kind and revision, and the old one closed as `redirected`.
pub fn redirect_delivery_obligation(
    ctx: &mut CommandContext<'_>,
    input: RedirectDeliveryObligationInput,
) -> Result<ObligationView, ReportingError> {
    ctx.require(Capability::DeliverReports)?;
    let by = user_id(ctx)?.to_string();
    let reason = required("reason", &input.reason, 1000)?;
    let (old, revision) = open_obligation(ctx.tx(), input.obligation_id)?;
    if old.destination_id == input.destination_id {
        return Err(invalid(
            "destinationId",
            "same_destination",
            "Choose a different destination.",
        ));
    }
    usable_destination(
        ctx.tx(),
        input.destination_id,
        order_client(ctx.tx(), revision.report_id)?,
    )?;
    let obligation = ObligationRow {
        id: ObligationId::generate(),
        destination_id: input.destination_id,
        origin: ObligationOrigin::Redirected,
        origin_reason: Some(reason.clone()),
        redirected_from_id: Some(old.id),
        created_at: ctx.now(),
        closure: None,
        ..old
    };
    ensure_new(ctx.tx(), &revision, input.destination_id, old.kind)?;
    delivery::insert_obligation(ctx.tx(), &obligation, &by)?;
    delivery::close_obligation(
        ctx.tx(),
        old.id,
        ObligationClosureCause::Redirected,
        Some(obligation.id),
        &by,
        ctx.now(),
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::REDIRECT_OBLIGATION,
            Outcome::Succeeded,
        )
        .record("delivery_obligation", obligation.id)
        .detail("redirected_from_id", AuditDetail::id(old.id))
        .detail("destination_id", AuditDetail::id(input.destination_id))
        .reason(reason),
    )?;
    view(ctx.tx(), &obligation)
}

/// Records an attested attempt on an open obligation. Never "delivered":
/// staff attest a hand-off, a failure or an unknown outcome (REQ-RPT-011).
pub fn record_delivery_attempt(
    ctx: &mut CommandContext<'_>,
    input: RecordDeliveryAttemptInput,
) -> Result<ObligationView, ReportingError> {
    ctx.require(Capability::DeliverReports)?;
    let recorded_by = user_id(ctx)?;
    let note = optional("note", input.note.as_deref(), 1000)?;
    let observed_at = input.observed_at.unwrap_or(ctx.now());
    if observed_at > ctx.now() {
        return Err(invalid(
            "observedAt",
            "future",
            "The time cannot be in the future.",
        ));
    }
    let failed = input.evidence == DeliveryEvidence::Failed;
    if failed != input.failure_code.is_some() {
        return Err(invalid(
            "failureCode",
            "required",
            "Give a failure code for, and only for, a failed attempt.",
        ));
    }
    let (obligation, revision) = open_obligation(ctx.tx(), input.obligation_id)?;
    usable_destination(
        ctx.tx(),
        obligation.destination_id,
        order_client(ctx.tx(), revision.report_id)?,
    )?;
    if DELIVERY_PAUSED_WHILE_IMPACT_OPEN
        && obligation.kind == ObligationKind::Report
        && impact::impact(ctx.tx(), &revision)?.pauses_delivery()
    {
        return Err(ReportingError::ImpactOpen);
    }
    let attempt = AttemptRow {
        id: DeliveryAttemptId::generate(),
        obligation_id: obligation.id,
        evidence: input.evidence,
        observed_at,
        failure_code: input.failure_code,
        note,
        recorded_by,
        recorded_at: ctx.now(),
    };
    delivery::insert_attempt(ctx.tx(), &attempt)?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        actions::RECORD_DELIVERY,
        Outcome::Succeeded,
    )
    .record("delivery_attempt", attempt.id)
    .detail("obligation_id", AuditDetail::id(obligation.id))
    .detail("evidence", AuditDetail::code(attempt.evidence.as_str()));
    if let Some(code) = attempt.failure_code {
        event = event.detail("failure_code", AuditDetail::code(code.as_str()));
    }
    ctx.audit(event)?;
    view(ctx.tx(), &obligation)
}

/// An obligation neither closed nor handed off, with its revision usable
/// for the obligation's kind.
pub(crate) fn open_obligation(
    conn: &Connection,
    id: ObligationId,
) -> Result<(ObligationRow, RevisionRow), ReportingError> {
    let obligation =
        delivery::obligation(conn, id)?.ok_or(ReportingError::NotFound("delivery obligation"))?;
    if obligation.closure.is_some() || handed_off(conn, obligation.id)? {
        return Err(ReportingError::WrongState("delivery obligation"));
    }
    let revision = reports::revision(conn, obligation.revision_id)?
        .ok_or(ReportingError::Inconsistent("obligation revision"))?;
    check_revision(&revision, obligation.kind)?;
    Ok((obligation, revision))
}

/// One obligation per revision, destination and kind.
fn ensure_new(
    conn: &Connection,
    revision: &RevisionRow,
    destination_id: DestinationId,
    kind: ObligationKind,
) -> Result<(), ReportingError> {
    let exists = delivery::report_obligations(conn, revision.report_id)?
        .iter()
        .any(|o| {
            o.revision_id == revision.id && o.destination_id == destination_id && o.kind == kind
        });
    if exists {
        Err(ReportingError::Duplicate("delivery obligation"))
    } else {
        Ok(())
    }
}

/// A report PDF goes out only from a current revision; a notice only once
/// the report is withdrawn.
fn check_revision(revision: &RevisionRow, kind: ObligationKind) -> Result<(), ReportingError> {
    match (kind, revision.availability) {
        (ObligationKind::Report, RevisionAvailability::Current)
        | (ObligationKind::WithdrawalNotice, RevisionAvailability::Withdrawn) => Ok(()),
        (ObligationKind::Report, RevisionAvailability::Withdrawn) => {
            Err(ReportingError::ReportWithdrawn)
        }
        (ObligationKind::Report, RevisionAvailability::Superseded)
        | (
            ObligationKind::WithdrawalNotice,
            RevisionAvailability::Current | RevisionAvailability::Superseded,
        ) => Err(ReportingError::WrongState("report revision")),
    }
}

/// RPT-012: verified, active, not reported wrong and of the order's client.
fn usable_destination(
    conn: &Connection,
    id: DestinationId,
    client_id: ClientId,
) -> Result<DestinationRow, ReportingError> {
    let destination =
        delivery::destination(conn, id)?.ok_or(ReportingError::NotFound("destination"))?;
    let mut gates = Vec::new();
    if destination.client_id != client_id {
        gates.push(gate(
            "destination_not_of_client",
            "The destination belongs to another client.",
        ));
    }
    if destination.retired_at.is_some() {
        gates.push(gate("destination_retired", "The destination is retired."));
    }
    // A wrong address is wrong for every obligation, so one attempt
    // reporting it blocks the destination until it is replaced or retired.
    if destination.reported_wrong_at.is_some() {
        gates.push(gate(
            "destination_reported_wrong",
            "A delivery attempt found this destination wrong; replace or retire it, then redirect.",
        ));
    }
    if destination.verification.is_none() {
        gates.push(gate(
            "destination_not_verified",
            "Nobody has verified the destination yet.",
        ));
    }
    if gates.is_empty() {
        Ok(destination)
    } else {
        Err(ReportingError::GateBlocked(gates))
    }
}

fn view(conn: &Connection, obligation: &ObligationRow) -> Result<ObligationView, ReportingError> {
    let stored = delivery::obligation(conn, obligation.id)?
        .ok_or(ReportingError::NotFound("delivery obligation"))?;
    obligation_view(conn, &mut DeliveryContext::new(conn)?, &stored)
}
