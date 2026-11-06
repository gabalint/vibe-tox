//! The message log, its CSV export, Unrouted messages and sending a failed message again.

use std::collections::HashMap;
use std::path::PathBuf;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{
    ListPartnerMessages, MessageSource, PartnerMessagePage, PartnerMessageRow,
    ResendPartnerDelivery, ResolveUnroutedMessage, UnroutedMessage, WaitReason,
};
use lis_reporting::{ReportRevisionId, ReportingError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::{
    EXPORT_PARTNER_MESSAGES, LIST_PARTNER_MESSAGES, LIST_UNROUTED_PARTNER_MESSAGES,
    RESEND_PARTNER_DELIVERY, RESOLVE_UNROUTED_MESSAGE, ReportRoutingError,
};
use crate::engine::Engine;
use crate::pipeline::Failure;
use crate::report_rendering::ExportFile;
use crate::report_rendering::csv::cell as csv_cell;

const CSV_HEADER: &str = "time_utc,direction,connection,message,record,transport,\
     application,waiting,attempts,given_up,next_attempt_utc,error,file,unrouted,decision,\
     reason,superseded";

/// Where the log export was written: always the user's Downloads folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedMessageLog {
    pub file_name: String,
    #[ts(type = "\"downloads\"")]
    pub folder: &'static str,
    #[ts(type = "number")]
    pub rows: usize,
}

impl Engine {
    /// `list_partner_messages`: the log, newest first. Rows carry record numbers, codes
    /// and file names, never message content.
    pub async fn list_partner_messages(
        &self,
        input: ListPartnerMessages,
    ) -> Result<PartnerMessagePage, CommandError> {
        self.run(&LIST_PARTNER_MESSAGES, |call| async move {
            self.read(&call, move |conn| log_page(conn, &input)).await
        })
        .await
    }

    /// `export_partner_messages`: the rows the same filter shows, as a CSV file in
    /// `folder`, the Downloads folder the desktop shell resolves.
    pub async fn export_partner_messages(
        &self,
        input: ListPartnerMessages,
        folder: Option<PathBuf>,
    ) -> Result<ExportedMessageLog, CommandError> {
        self.run(&EXPORT_PARTNER_MESSAGES, |call| async move {
            self.authorize(&call).await?;
            let folder = folder.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Unavailable,
                    "The Downloads folder is not available on this computer.",
                )
            })?;
            let page = self.read(&call, move |conn| log_page(conn, &input)).await?;
            let rows = page.rows.len();
            let bytes = csv(&page.rows).into_bytes();
            let sha256 = hex::encode(Sha256::digest(&bytes));
            let stamp = self.clock.now().to_string();
            let file = ExportFile {
                folder,
                stem: format!("interface-log-{}", &stamp[..10]),
                extension: "csv",
                sha256,
                source: ("rows", AuditDetail::count(rows)),
            };
            let file_name = self
                .export_file(&call, file, bytes, move |ctx, name| {
                    ctx.audit(
                        AuditEvent::new(
                            Stream::Security,
                            EXPORT_PARTNER_MESSAGES.action,
                            Outcome::Succeeded,
                        )
                        .record("partner_message_log", name)
                        .detail("rows", AuditDetail::count(rows)),
                    )
                    .map(drop)
                })
                .await?;
            Ok(ExportedMessageLog {
                file_name,
                folder: "downloads",
                rows,
            })
        })
        .await
    }

    /// `list_unrouted_partner_messages`: messages waiting for a decision, oldest first.
    pub async fn list_unrouted_partner_messages(
        &self,
    ) -> Result<Vec<UnroutedMessage>, CommandError> {
        self.run(&LIST_UNROUTED_PARTNER_MESSAGES, |call| async move {
            self.read(
                &call,
                lis_interfaces::partner::list_unrouted_partner_messages,
            )
            .await
        })
        .await
    }

    /// `resolve_unrouted_message`: send once, route again, deliver another way or dismiss.
    pub async fn resolve_unrouted_message(
        &self,
        input: ResolveUnroutedMessage,
    ) -> Result<(), CommandError> {
        self.run(&RESOLVE_UNROUTED_MESSAGE, |call| async move {
            self.write(&call, move |ctx| {
                lis_interfaces::partner::resolve_unrouted_message(ctx, input).map(drop)
            })
            .await
        })
        .await
    }

    /// `resend_partner_delivery`: a failed message waits again for the poller.
    pub async fn resend_partner_delivery(
        &self,
        input: ResendPartnerDelivery,
    ) -> Result<(), CommandError> {
        self.run(&RESEND_PARTNER_DELIVERY, |call| async move {
            self.write(&call, move |ctx| {
                lis_interfaces::partner::resend_partner_delivery(ctx, input).map(drop)
            })
            .await
        })
        .await
    }
}

/// The log page, with report revisions that wait on their impact review saying so (L13):
/// the interfaces module does not know reports.
fn log_page(
    conn: &rusqlite::Connection,
    input: &ListPartnerMessages,
) -> Result<PartnerMessagePage, ReportRoutingError> {
    let mut page = lis_interfaces::partner::list_partner_messages(conn, input)?;
    let mut paused: HashMap<String, bool> = HashMap::new();
    for row in &mut page.rows {
        let queued = matches!(
            row.waiting,
            Some(WaitReason::Queued | WaitReason::Scheduled)
        );
        if !queued {
            continue;
        }
        if impact_pause(conn, row, &mut paused)? {
            row.waiting = Some(WaitReason::ImpactReview);
        } else if !super::mappings::missing_for(conn, row)?.is_empty() {
            row.waiting = Some(WaitReason::NotMapped);
        }
    }
    Ok(page)
}

/// Whether a report revision's row waits on its impact review, once per revision.
fn impact_pause(
    conn: &rusqlite::Connection,
    row: &PartnerMessageRow,
    paused: &mut HashMap<String, bool>,
) -> Result<bool, ReportRoutingError> {
    let Some(id) = row.source_id.as_deref() else {
        return Ok(false);
    };
    if row.source != MessageSource::ReportRevision {
        return Ok(false);
    }
    if let Some(waits) = paused.get(id) {
        return Ok(*waits);
    }
    let revision_id: ReportRevisionId = id
        .parse()
        .map_err(|_| ReportingError::Inconsistent("partner message revision"))?;
    let waits = lis_reporting::revision_delivery_paused(conn, revision_id)?;
    paused.insert(id.to_owned(), waits);
    Ok(waits)
}

/// The rows as CSV, codes as stored; never "delivered" (REQ-RPT-011).
fn csv(rows: &[PartnerMessageRow]) -> String {
    let mut text = String::from(CSV_HEADER);
    text.push_str("\r\n");
    for row in rows {
        let number = row.source_number.unwrap_or_default();
        let label = row.source_label.as_deref().unwrap_or_default();
        let record = match row.source {
            MessageSource::BillingHandoff => format!("billing_handoff {number}"),
            MessageSource::ReportRevision => format!("{label} r{number}"),
            MessageSource::ReportWithdrawal => format!("{label} withdrawal r{number}"),
            MessageSource::Test => "test".to_owned(),
            MessageSource::Order => format!("order {label}"),
        };
        let direction = if row.received.is_some() { "in" } else { "out" };
        let cells = [
            row.at.to_string(),
            direction.to_owned(),
            row.connection
                .as_ref()
                .map(|connection| connection.name.clone())
                .unwrap_or_default(),
            row.message_type.as_str().to_owned(),
            record,
            row.transport.as_str().to_owned(),
            if row.outcome_unknown {
                "outcome_unknown"
            } else {
                ""
            }
            .to_owned(),
            row.waiting
                .map(|reason| reason.as_str())
                .unwrap_or_default()
                .to_owned(),
            row.attempts.to_string(),
            row.given_up.to_string(),
            row.next_attempt_at
                .map(|at| at.to_string())
                .unwrap_or_default(),
            row.error_code.clone().unwrap_or_default(),
            row.written_path.clone().unwrap_or_default(),
            row.unrouted.to_string(),
            row.resolution
                .map(|resolution| resolution.as_str())
                .unwrap_or_default()
                .to_owned(),
            row.reason
                .map(|reason| reason.as_str())
                .unwrap_or_default()
                .to_owned(),
            row.superseded.to_string(),
        ];
        let line: Vec<String> = cells.iter().map(|cell| csv_cell(cell)).collect();
        text.push_str(&line.join(","));
        text.push_str("\r\n");
    }
    text
}
