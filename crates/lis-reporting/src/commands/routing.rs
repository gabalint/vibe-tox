//! Which obligations a release or withdrawal creates and closes (brief §3.5
//! step 2, §4.2; L11; REQ-RPT-003, REQ-RPT-013).

use lis_orders::OrderId;
use lis_store::CommandContext;
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::model::{
    DeliveryEvidence, DestinationId, ObligationClosureCause, ObligationId, ObligationKind,
    ObligationOrigin, ReportId, ReportRevisionId,
};
use crate::repo::delivery::{self, ObligationRow};

/// Obligations to create on a revision and earlier ones to close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObligationPlan {
    kind: ObligationKind,
    create: Vec<(DestinationId, ObligationOrigin)>,
    close: Vec<ObligationId>,
    cause: ObligationClosureCause,
}

impl ObligationPlan {
    pub(crate) fn obligation_count(&self) -> usize {
        self.create.len()
    }

    /// Writes the plan in the caller's transaction.
    pub(crate) fn apply(
        &self,
        ctx: &CommandContext<'_>,
        revision_id: ReportRevisionId,
    ) -> Result<(), ReportingError> {
        let by = crate::authority::user_id(ctx)?.to_string();
        for (destination_id, origin) in &self.create {
            let obligation = ObligationRow {
                id: ObligationId::generate(),
                revision_id,
                destination_id: *destination_id,
                kind: self.kind,
                origin: *origin,
                origin_reason: None,
                redirected_from_id: None,
                created_at: ctx.now(),
                closure: None,
            };
            delivery::insert_obligation(ctx.tx(), &obligation, &by)?;
        }
        for obligation_id in &self.close {
            delivery::close_obligation(ctx.tx(), *obligation_id, self.cause, None, &by, ctx.now())?;
        }
        Ok(())
    }
}

/// Release: the active destinations of the order's location, else of the
/// client (L11), plus every destination of an earlier revision
/// (`correction_follow_up`); earlier report obligations never handed off
/// are closed as `newer_revision`.
pub(crate) fn release_plan(
    conn: &Connection,
    report_id: ReportId,
    order_id: OrderId,
) -> Result<ObligationPlan, ReportingError> {
    let order = lis_orders::get_order(conn, order_id)?;
    let mut create: Vec<(DestinationId, ObligationOrigin)> =
        delivery::routing(conn, order.client_id, order.client_location_id)?
            .into_iter()
            .map(|d| (d.id, ObligationOrigin::Release))
            .collect();
    let earlier: Vec<ObligationRow> = delivery::report_obligations(conn, report_id)?
        .into_iter()
        .filter(|o| o.kind == ObligationKind::Report)
        .collect();
    for obligation in &earlier {
        if !create
            .iter()
            .any(|(id, _)| *id == obligation.destination_id)
        {
            create.push((
                obligation.destination_id,
                ObligationOrigin::CorrectionFollowUp,
            ));
        }
    }
    Ok(ObligationPlan {
        kind: ObligationKind::Report,
        create,
        close: open_ids(conn, &earlier)?,
        cause: ObligationClosureCause::NewerRevision,
    })
}

/// Withdrawal: a notice for every destination that ever had an obligation
/// on the report; open report obligations are closed as `report_withdrawn`.
pub(crate) fn withdrawal_plan(
    conn: &Connection,
    report_id: ReportId,
) -> Result<ObligationPlan, ReportingError> {
    let all = delivery::report_obligations(conn, report_id)?;
    let mut create: Vec<(DestinationId, ObligationOrigin)> = Vec::new();
    for obligation in &all {
        if !create
            .iter()
            .any(|(id, _)| *id == obligation.destination_id)
        {
            create.push((obligation.destination_id, ObligationOrigin::Withdrawal));
        }
    }
    let reports: Vec<ObligationRow> = all
        .into_iter()
        .filter(|o| o.kind == ObligationKind::Report)
        .collect();
    Ok(ObligationPlan {
        kind: ObligationKind::WithdrawalNotice,
        create,
        close: open_ids(conn, &reports)?,
        cause: ObligationClosureCause::ReportWithdrawn,
    })
}

/// Obligations neither closed nor handed off.
fn open_ids(
    conn: &Connection,
    obligations: &[ObligationRow],
) -> Result<Vec<ObligationId>, ReportingError> {
    let mut open = Vec::new();
    for obligation in obligations.iter().filter(|o| o.closure.is_none()) {
        if !handed_off(conn, obligation.id)? {
            open.push(obligation.id);
        }
    }
    Ok(open)
}

/// Whether any attempt of the obligation was handed off.
pub(crate) fn handed_off(
    conn: &Connection,
    obligation_id: ObligationId,
) -> Result<bool, ReportingError> {
    Ok(delivery::attempts(conn, obligation_id)?
        .iter()
        .any(|a| a.evidence == DeliveryEvidence::HandedOff))
}
