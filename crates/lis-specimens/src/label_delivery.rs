//! Label rendering and hand-off for the `spc.label.print` worker (REQ-SPC-001).
//!
//! The worker runs outside any transaction: it reads a [`LabelRequest`],
//! calls [`deliver_label`] (render with `lis-labels`, then send), and records
//! the [`LabelDelivery`] with [`record_label_outcome`](crate::record_label_outcome).
//! A failed hand-off is recorded, not retried: a label that appears much
//! later at the bench would be more confusing than a visible failure the
//! accessioner answers with a reprint.

use std::net::IpAddr;

use lis_core::Phi;
use lis_labels::{
    LabelTemplateRef, Marking, PrintError, PrinterDestinations, QueueName, Transport,
};
use rusqlite::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::SpecimenError;
use crate::model::{LabelJobId, LabelOutcome, PrinterTransport};
use crate::repo::{config, labels};
use crate::views::LabelPrinterView;

/// Outbox topic of label print jobs; the payload is `{"labelJobId": "<id>"}`.
pub const LABEL_PRINT_TOPIC: &str = "spc.label.print";

/// The label template every container label uses in M2 (risk R2).
pub(crate) const SPECIMEN_TEMPLATE: (&str, u32) = ("specimen", 1);

/// Field limits of `specimen` v1; longer values are cut at a character
/// boundary and the outcome records `truncated`.
const SUBJECT_NAME_MAX: usize = 28;
const BIRTH_DATE_MAX: usize = 10;
const ACCESSION_MAX: usize = 16;
const MATRIX_MAX: usize = 12;

/// The job ID in an `spc.label.print` payload.
#[must_use]
pub fn label_job_id(payload: &Value) -> Option<LabelJobId> {
    payload.get("labelJobId")?.as_str()?.parse().ok()
}

/// Everything needed to render and send one label. Holds PHI.
pub struct LabelRequest {
    job_id: LabelJobId,
    template: LabelTemplateRef,
    printer: LabelPrinterView,
    container_code: String,
    subject_name: Phi<String>,
    birth_date: Phi<String>,
    accession_number: String,
    matrix: String,
}

/// What happened to one label job, for [`record_label_outcome`](crate::record_label_outcome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelDelivery {
    /// The job.
    pub job_id: LabelJobId,
    /// Handed off or failed.
    pub outcome: LabelOutcome,
    /// A PHI-free code for a failure.
    pub error_code: Option<&'static str>,
    /// SHA-256 of the rendered bytes, when rendering succeeded.
    pub label_sha256: Option<String>,
    /// Whether a field was cut to fit, when rendering succeeded.
    pub truncated: Option<bool>,
}

/// The label request of a job, or `None` when its outcome is already
/// recorded, so a redelivered outbox item never prints twice.
pub fn label_request(
    conn: &Connection,
    job_id: LabelJobId,
) -> Result<Option<LabelRequest>, SpecimenError> {
    let content = labels::job_content(conn, job_id)?.ok_or(SpecimenError::NotFound("label job"))?;
    if content.has_outcome {
        return Ok(None);
    }
    let printer =
        config::printer(conn, content.printer_id)?.ok_or(SpecimenError::NotFound("printer"))?;
    let subject = lis_identity::get_subject(conn, content.subject_id)?;
    let current = subject
        .assertions
        .last()
        .ok_or(SpecimenError::NotFound("subject"))?;
    let subject_name = match &current.given_names {
        Some(given) => format!("{}, {}", current.family_name.expose(), given.expose()),
        None => current.family_name.expose().clone(),
    };
    Ok(Some(LabelRequest {
        job_id,
        template: LabelTemplateRef {
            id: content.template_id,
            version: content.template_version,
        },
        printer,
        container_code: content.label_code,
        subject_name: Phi::new(subject_name),
        birth_date: current.birth_date.clone(),
        accession_number: content.accession_number,
        matrix: content.specimen_type_code,
    }))
}

/// Renders the label with the installation's `marking` (the demo laboratory's
/// labels say "DEMO") and hands it to the printer if `destinations` admits its
/// address. Blocking network or spooler I/O: run it off the async runtime and
/// outside any transaction.
#[must_use]
pub fn deliver_label(
    request: &LabelRequest,
    destinations: PrinterDestinations,
    marking: Marking,
) -> LabelDelivery {
    let failed = |error_code| LabelDelivery {
        job_id: request.job_id,
        outcome: LabelOutcome::Failed,
        error_code: Some(error_code),
        label_sha256: None,
        truncated: None,
    };
    if !request.printer.active {
        return failed("printer_inactive");
    }
    let mut truncated = false;
    let mut fit = |value: &str, max: usize| {
        let cut: String = value.chars().take(max).collect();
        truncated |= cut.len() < value.len();
        cut
    };
    let fields = [
        ("container_id", request.container_code.clone()),
        (
            "subject_name",
            fit(request.subject_name.expose(), SUBJECT_NAME_MAX),
        ),
        (
            "birth_date",
            fit(request.birth_date.expose(), BIRTH_DATE_MAX),
        ),
        ("accession", fit(&request.accession_number, ACCESSION_MAX)),
        ("matrix", fit(&request.matrix, MATRIX_MAX)),
    ];
    let borrowed: Vec<(&str, &str)> = fields
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let Ok(bytes) = lis_labels::render_marked(&request.template, &borrowed, marking) else {
        return failed("render_failed");
    };
    let digest = hex::encode(Sha256::digest(&bytes));
    let sent = transport(&request.printer).and_then(|transport| {
        transport
            .print(&bytes, destinations)
            .map_err(|error| print_error_code(&error))
    });
    LabelDelivery {
        job_id: request.job_id,
        outcome: if sent.is_ok() {
            LabelOutcome::HandedOff
        } else {
            LabelOutcome::Failed
        },
        error_code: sent.err(),
        label_sha256: Some(digest),
        truncated: Some(truncated),
    }
}

/// The transport of a configured printer, or a PHI-free error code.
fn transport(printer: &LabelPrinterView) -> Result<Transport, &'static str> {
    match printer.transport {
        PrinterTransport::Tcp => {
            let ip: IpAddr = printer
                .ip_address
                .as_deref()
                .and_then(|ip| ip.parse().ok())
                .ok_or("printer_misconfigured")?;
            let mut transport = Transport::tcp(ip);
            if let (Transport::Tcp { address, .. }, Some(port)) = (&mut transport, printer.port) {
                address.set_port(port);
            }
            Ok(transport)
        }
        PrinterTransport::Cups => {
            let queue = QueueName::new(printer.queue.as_deref().unwrap_or_default())
                .map_err(|error| print_error_code(&error))?;
            Ok(Transport::Cups { queue })
        }
    }
}

fn print_error_code(error: &PrintError) -> &'static str {
    match error {
        PrintError::InvalidQueueName => "invalid_queue",
        PrintError::UnsupportedTransport => "printer_unavailable",
        PrintError::ConnectionRefused => "connection_refused",
        PrintError::ConnectTimeout => "connect_timeout",
        PrintError::WriteTimeout => "write_timeout",
        PrintError::DestinationRefused => "destination_refused",
        PrintError::Network { .. } => "network_error",
        PrintError::SpoolerUnavailable { .. } => "spooler_unavailable",
        PrintError::SpoolerRejected { .. } => "spooler_rejected",
        PrintError::SpoolerTimeout => "spooler_timeout",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PrinterId;

    const LAB: PrinterDestinations = PrinterDestinations::LabNetwork;

    fn request(subject_name: &str, printer: LabelPrinterView) -> LabelRequest {
        LabelRequest {
            job_id: LabelJobId::generate(),
            template: LabelTemplateRef {
                id: SPECIMEN_TEMPLATE.0.to_owned(),
                version: SPECIMEN_TEMPLATE.1,
            },
            printer,
            container_code: "C00000001".to_owned(),
            subject_name: Phi::new(subject_name.to_owned()),
            birth_date: Phi::new("1980-01-01".to_owned()),
            accession_number: "A26000001".to_owned(),
            matrix: "syn-ur".to_owned(),
        }
    }

    fn cups(queue: &str, active: bool) -> LabelPrinterView {
        LabelPrinterView {
            printer_id: PrinterId::generate(),
            code: "bench".to_owned(),
            name: "Synthetic".to_owned(),
            transport: PrinterTransport::Cups,
            ip_address: None,
            port: None,
            queue: Some(queue.to_owned()),
            active,
            revision: 1,
        }
    }

    #[test]
    fn long_names_are_cut_at_a_character_boundary_and_flagged() {
        // The queue is invalid, so nothing is sent, but the label is rendered.
        let name = "SYNTHETIC-LONGFAMILYNAME, Patiënt Alpha Beta";
        let delivery = deliver_label(
            &request(name, cups("-invalid", true)),
            LAB,
            Marking::Unmarked,
        );
        assert_eq!(delivery.outcome, LabelOutcome::Failed);
        assert_eq!(delivery.error_code, Some("invalid_queue"));
        assert_eq!(delivery.truncated, Some(true));
        assert_eq!(delivery.label_sha256.map(|digest| digest.len()), Some(64));
        let short = deliver_label(
            &request("SYNTHETIC, Patiënt", cups("-invalid", true)),
            LAB,
            Marking::Unmarked,
        );
        assert_eq!(short.truncated, Some(false));
    }

    #[test]
    fn an_inactive_printer_gets_nothing() {
        let delivery = deliver_label(
            &request("SYNTHETIC, Alpha", cups("labels", false)),
            LAB,
            Marking::Unmarked,
        );
        assert_eq!(
            (delivery.outcome, delivery.error_code, delivery.label_sha256),
            (LabelOutcome::Failed, Some("printer_inactive"), None)
        );
    }

    #[test]
    fn payloads_without_a_job_id_are_refused() {
        assert_eq!(
            label_job_id(&serde_json::json!({ "labelJobId": "nope" })),
            None
        );
        assert_eq!(label_job_id(&serde_json::json!({})), None);
    }
}
