//! Exact obligation details retain full attempt evidence; callers audit sensitive access.
use super::candidate::user_view;
use super::delivery::{DeliveryContext, status};
use crate::delivery::labels::delivery_status_label;
use crate::error::ReportingError;
use crate::model::{ObligationId, ObligationStatus};
use crate::repo::delivery::{self, ObligationRow};
use crate::repo::reports::{self, ReportRow};
use crate::views::{AttemptView, DeliveryStatusView, ObligationDestinationView, ObligationView};
use rusqlite::Connection;

/// Exact retained obligation lookup, including closed work and full attempt evidence.
/// Engine requires delivery authority and audits this read; queue page filters do not apply.
pub fn get_delivery_obligation(
    conn: &Connection,
    id: ObligationId,
) -> Result<ObligationView, ReportingError> {
    let row =
        delivery::obligation(conn, id)?.ok_or(ReportingError::NotFound("delivery obligation"))?;
    obligation_view(conn, &mut DeliveryContext::new(conn)?, &row)
}

/// One obligation as the delivery screens show it.
pub(crate) fn obligation_view(
    conn: &Connection,
    context: &mut DeliveryContext,
    obligation: &ObligationRow,
) -> Result<ObligationView, ReportingError> {
    let destination = delivery::destination(conn, obligation.destination_id)?
        .ok_or(ReportingError::Inconsistent("obligation destination"))?;
    let revision = reports::revision(conn, obligation.revision_id)?
        .ok_or(ReportingError::Inconsistent("obligation revision"))?;
    let report: ReportRow = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("obligation report"))?;
    let attempts = delivery::attempts(conn, obligation.id)?;
    let status = status(
        conn,
        context,
        obligation,
        &destination,
        &revision,
        &attempts,
    )?;
    let code = status.code();
    let allowed_actions = match code {
        ObligationStatus::Awaiting | ObligationStatus::Attention => {
            vec![
                "record_delivery_attempt",
                "redirect_delivery_obligation",
                "export_report_pdf",
            ]
        }
        ObligationStatus::Blocked => vec!["redirect_delivery_obligation"],
        ObligationStatus::Paused => vec!["redirect_delivery_obligation", "export_report_pdf"],
        ObligationStatus::HandedOff | ObligationStatus::Closed => Vec::new(),
    };
    Ok(ObligationView {
        obligation_id: obligation.id,
        client_id: context.client(conn, report.id)?,
        report_id: report.id,
        report_number: report.report_number,
        revision_id: revision.id,
        revision_no: revision.revision_no,
        kind: obligation.kind,
        destination: ObligationDestinationView {
            destination_id: destination.id,
            channel: destination.channel,
            label: destination.label.clone(),
            verified: destination.verification.is_some(),
        },
        status: DeliveryStatusView {
            code,
            label: delivery_status_label(destination.channel, &status),
        },
        attempts: attempts
            .iter()
            .map(|a| {
                Ok(AttemptView {
                    attempt_id: a.id,
                    evidence: a.evidence,
                    observed_at: a.observed_at,
                    recorded_by: user_view(conn, a.recorded_by)?,
                    failure_code: a.failure_code,
                    note: a.note.clone(),
                })
            })
            .collect::<Result<_, ReportingError>>()?,
        origin: obligation.origin,
        origin_reason: obligation.origin_reason.clone(),
        created_at: obligation.created_at,
        allowed_actions: allowed_actions.into_iter().map(str::to_owned).collect(),
    })
}
