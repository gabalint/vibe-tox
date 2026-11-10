//! The message log, its CSV export, Unrouted messages, sending a failed message again, and
//! a person's follow-up of rejected messages.

use std::collections::HashMap;
use std::path::PathBuf;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{
    AckCode, ListPartnerMessages, MessageSource, PartnerMessagePage, PartnerMessageRow,
    PartnerProtocol, ReceivedState, RejectReceivedMessage, ResendPartnerDelivery,
    ResolvePartnerRejection, ResolveUnroutedMessage, TransportOutcome, UnroutedMessage, WaitReason,
};
use lis_reporting::{ReportRevisionId, ReportingError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::{
    EXPORT_PARTNER_MESSAGES, LIST_PARTNER_MESSAGES, LIST_UNROUTED_PARTNER_MESSAGES,
    REJECT_RECEIVED_MESSAGE, RESEND_PARTNER_DELIVERY, RESOLVE_PARTNER_REJECTION,
    RESOLVE_UNROUTED_MESSAGE, ReportRoutingError,
};
use crate::commands::people::{display_names, name_of};
use crate::engine::Engine;
use crate::pipeline::Failure;
use crate::report_rendering::ExportFile;
use crate::report_rendering::csv::cell as csv_cell;

const CSV_HEADER: &str = "time_utc,direction,connection,message,record,transport,\
     application,waiting,failed_attempts,given_up,next_attempt_utc,error,file,unrouted,decision,\
     reason,superseded,ack_text";

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

    /// `reject_received_message`: a message waiting for its patient or held for review makes
    /// no order; the person's reason resolves it.
    pub async fn reject_received_message(
        &self,
        input: RejectReceivedMessage,
    ) -> Result<(), CommandError> {
        self.run(&REJECT_RECEIVED_MESSAGE, |call| async move {
            self.write(&call, move |ctx| {
                lis_interfaces::partner::reject_received_message(ctx, input).map(drop)
            })
            .await
        })
        .await
    }

    /// `resolve_partner_rejection`: a rejected message leaves Rejected, with the note.
    pub async fn resolve_partner_rejection(
        &self,
        input: ResolvePartnerRejection,
    ) -> Result<(), CommandError> {
        self.run(&RESOLVE_PARTNER_REJECTION, |call| async move {
            self.write(&call, move |ctx| {
                lis_interfaces::partner::resolve_partner_rejection(ctx, input)
            })
            .await
        })
        .await
    }
}

/// The log page, with report revisions that wait on their impact review saying so (L13),
/// and HL7 messages held for a code mapping or a patient identifier: the interfaces module
/// does not know reports.
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
        } else if let Some(held) = super::mappings::hold_reason(conn, row)? {
            row.waiting = Some(held);
        }
    }
    let resolved = page
        .rows
        .iter()
        .filter_map(|row| row.rejection_resolved.as_ref());
    let names = display_names(conn, resolved.map(|done| done.resolved.by.as_str()))?;
    for done in page
        .rows
        .iter_mut()
        .filter_map(|row| row.rejection_resolved.as_mut())
    {
        done.resolved.by_display = name_of(&names, &done.resolved.by);
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
            transport_text(
                row.transport,
                row.connection
                    .as_ref()
                    .map(|connection| connection.protocol),
            )
            .to_owned(),
            application_text(
                row.received,
                row.ack_code,
                row.awaiting_ack,
                row.outcome_unknown,
            )
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
            row.ack_text.clone().unwrap_or_default(),
        ];
        let line: Vec<String> = cells.iter().map(|cell| csv_cell(cell)).collect();
        text.push_str(&line.join(","));
        text.push_str("\r\n");
    }
    text
}

/// The transport as the Logs page shows it: a message that left over MLLP was sent, to an
/// SFTP server uploaded, to a folder written.
fn transport_text(transport: TransportOutcome, protocol: Option<PartnerProtocol>) -> &'static str {
    match (transport, protocol) {
        (TransportOutcome::Written, Some(PartnerProtocol::Sftp)) => "uploaded",
        (
            TransportOutcome::Written,
            Some(PartnerProtocol::MllpConnect | PartnerProtocol::MllpListen),
        ) => "sent",
        _ => transport.as_str(),
    }
}

/// The application outcome as the Logs page shows it: what a received message became, the
/// partner's acknowledgement code, still awaiting one, or unknowable (a written file).
fn application_text(
    received: Option<ReceivedState>,
    ack: Option<AckCode>,
    awaiting_ack: bool,
    outcome_unknown: bool,
) -> &'static str {
    if let Some(state) = received {
        return state.as_str();
    }
    if let Some(ack) = ack {
        return ack.as_str();
    }
    if awaiting_ack {
        return "awaiting_ack";
    }
    if outcome_unknown {
        "outcome_unknown"
    } else {
        ""
    }
}

#[cfg(test)]
mod csv_tests {
    use super::*;

    #[test]
    fn transport_reads_as_the_page_shows_it() {
        let written = TransportOutcome::Written;
        assert_eq!(
            transport_text(written, Some(PartnerProtocol::MllpConnect)),
            "sent"
        );
        assert_eq!(
            transport_text(written, Some(PartnerProtocol::Sftp)),
            "uploaded"
        );
        assert_eq!(
            transport_text(written, Some(PartnerProtocol::Folder)),
            "written"
        );
        assert_eq!(
            transport_text(TransportOutcome::Failed, Some(PartnerProtocol::MllpConnect)),
            "failed"
        );
    }

    #[test]
    fn application_names_the_acknowledgement_or_what_was_received() {
        assert_eq!(
            application_text(None, Some(AckCode::Accepted), false, false),
            "AA"
        );
        assert_eq!(application_text(None, None, true, false), "awaiting_ack");
        assert_eq!(application_text(None, None, false, true), "outcome_unknown");
        assert_eq!(application_text(None, None, false, false), "");
        assert_eq!(
            application_text(
                Some(ReceivedState::Rejected),
                Some(AckCode::Rejected),
                false,
                false
            ),
            "rejected"
        );
    }
}
