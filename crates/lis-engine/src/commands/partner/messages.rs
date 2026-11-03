//! The message log, its CSV export, Unrouted messages and sending a failed message again.

use std::path::PathBuf;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{CommandError, ErrorCode};
use lis_interfaces::partner::{
    ListPartnerMessages, PartnerMessagePage, PartnerMessageRow, ResendPartnerDelivery,
    ResolveUnroutedMessage, UnroutedMessage,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::{
    EXPORT_PARTNER_MESSAGES, LIST_PARTNER_MESSAGES, LIST_UNROUTED_PARTNER_MESSAGES,
    RESEND_PARTNER_DELIVERY, RESOLVE_UNROUTED_MESSAGE,
};
use crate::engine::Engine;
use crate::pipeline::Failure;
use crate::report_rendering::ExportFile;

const CSV_HEADER: &str = "time_utc,direction,connection,message,record,transport,\
     application,waiting,attempts,given_up,next_attempt_utc,error,file,unrouted,decision";

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
            self.read(&call, move |conn| {
                lis_interfaces::partner::list_partner_messages(conn, &input)
            })
            .await
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
            let page = self
                .read(&call, move |conn| {
                    lis_interfaces::partner::list_partner_messages(conn, &input)
                })
                .await?;
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

/// The rows as CSV, codes as stored; never "delivered" (REQ-RPT-011).
fn csv(rows: &[PartnerMessageRow]) -> String {
    let mut text = String::from(CSV_HEADER);
    text.push_str("\r\n");
    for row in rows {
        let record = match row.source_number {
            Some(number) => format!("{} {number}", row.source.as_str()),
            None => row.source.as_str().to_owned(),
        };
        let cells = [
            row.at.to_string(),
            "out".to_owned(),
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
        ];
        let line: Vec<String> = cells.iter().map(|cell| cell_text(cell)).collect();
        text.push_str(&line.join(","));
        text.push_str("\r\n");
    }
    text
}

/// One quoted CSV cell. A leading `=`, `+`, `-` or `@` is prefixed with `'` so a
/// spreadsheet never runs it as a formula.
fn cell_text(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_quoted_and_formulas_disarmed() {
        assert_eq!(cell_text("a\"b"), "\"a\"\"b\"");
        assert_eq!(cell_text("=SUM(A1)"), "\"'=SUM(A1)\"");
    }
}
