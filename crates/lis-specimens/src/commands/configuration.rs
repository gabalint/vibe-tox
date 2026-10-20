//! Storage locations and label printers (configuration).

use std::net::IpAddr;

use lis_audit::{AuditDetail, AuditEvent};
use lis_core::Capability;
use lis_labels::{DEFAULT_TCP_PORT, PrinterDestinations, QueueName};
use lis_store::{CommandContext, StoreError};
use serde::Deserialize;
use ts_rs::TS;

use super::{clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, ensure_revision, invalid};
use crate::input::{self, NAME_MAX, TEXT_MAX};
use crate::model::{LocationId, PrinterId, PrinterTransport};
use crate::repo::config as repo;
use crate::views::{LabelPrinterView, StorageLocationView};

/// The revision a configuration record gets: 1 when created; when updating
/// an existing record, its current revision plus one if the caller saw it.
fn next_revision(
    current: Option<i64>,
    expected: Option<i64>,
    what: &'static str,
) -> Result<i64, SpecimenError> {
    match (current, expected) {
        (None, None) => Ok(1),
        (Some(current), Some(expected)) => {
            ensure_revision(current, expected)?;
            Ok(current + 1)
        }
        (Some(_), None) => Err(SpecimenError::RevisionConflict),
        (None, Some(_)) => Err(invalid(
            "expectedRevision",
            "not_applicable",
            &format!("A new {what} has no revision yet."),
        )),
    }
}

/// Input of [`configure_location`]. Without `locationId`, creates a location.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigureStorageLocationInput {
    /// The location to update; absent to create one.
    #[serde(default)]
    #[ts(optional)]
    pub location_id: Option<LocationId>,
    /// The revision shown, when updating.
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub expected_revision: Option<i64>,
    /// Unique code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Storage condition, such as "2–8 °C".
    #[serde(default)]
    #[ts(optional)]
    pub storage_condition: Option<String>,
    /// Whether containers may be moved here.
    pub active: bool,
}

/// Creates or updates a storage location.
pub fn configure_location(
    ctx: &mut CommandContext<'_>,
    input: ConfigureStorageLocationInput,
) -> Result<StorageLocationView, SpecimenError> {
    require_user(ctx, Capability::ManageConfiguration)?;
    let code = input::code("code", &input.code)?;
    let name = input::required_text("name", &input.name, NAME_MAX)?;
    let storage_condition = input::optional_text(
        "storageCondition",
        input.storage_condition.as_deref(),
        TEXT_MAX,
    )?;
    let id = input.location_id.unwrap_or_else(LocationId::generate);
    let current = match input.location_id {
        Some(id) => Some(
            repo::location(ctx.tx(), id)?
                .ok_or(SpecimenError::NotFound("location"))?
                .revision,
        ),
        None => None,
    };
    let revision = next_revision(current, input.expected_revision, "location")?;
    if repo::location_code_taken(ctx.tx(), &code, id)? {
        return Err(SpecimenError::Duplicate("location"));
    }
    let location = StorageLocationView {
        location_id: id,
        code,
        name,
        storage_condition,
        active: input.active,
        revision,
    };
    repo::save_location(ctx.tx(), &location, ctx.now())?;
    ctx.audit(
        configuration_event(actions::CONFIGURE_LOCATION, current, revision)
            .record("location", id)
            .detail("active", AuditDetail::boolean(input.active)),
    )?;
    Ok(location)
}

/// Input of [`configure_printer`]. Without `printerId`, creates a printer.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigureLabelPrinterInput {
    /// The printer to update; absent to create one.
    #[serde(default)]
    #[ts(optional)]
    pub printer_id: Option<PrinterId>,
    /// The revision shown, when updating.
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub expected_revision: Option<i64>,
    /// Unique code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// TCP or CUPS.
    pub transport: PrinterTransport,
    /// Private or link-local IP address for TCP; host names are not accepted.
    #[serde(default)]
    #[ts(optional)]
    pub ip_address: Option<String>,
    /// TCP port; 9100 when absent.
    #[serde(default)]
    #[ts(optional)]
    pub port: Option<u16>,
    /// CUPS raw queue name.
    #[serde(default)]
    #[ts(optional)]
    pub queue: Option<String>,
    /// Whether labels may be sent to it.
    pub active: bool,
}

/// Creates or updates a label printer (REQ-SPC-001). TCP printers are
/// addressed by a private or link-local IP address, so no DNS lookup can
/// stall or redirect printing and labels cannot leave the laboratory network
/// (NFR-32); CUPS queue names are checked so they cannot become `lp` options.
/// The audit event records the full destination.
pub fn configure_printer(
    ctx: &mut CommandContext<'_>,
    input: ConfigureLabelPrinterInput,
    destinations: PrinterDestinations,
) -> Result<LabelPrinterView, SpecimenError> {
    require_user(ctx, Capability::ManageConfiguration)?;
    let code = input::code("code", &input.code)?;
    let name = input::required_text("name", &input.name, NAME_MAX)?;
    let id = input.printer_id.unwrap_or_else(PrinterId::generate);
    let current = match input.printer_id {
        Some(id) => Some(
            repo::printer(ctx.tx(), id)?
                .ok_or(SpecimenError::NotFound("printer"))?
                .revision,
        ),
        None => None,
    };
    let revision = next_revision(current, input.expected_revision, "printer")?;
    if repo::printer_code_taken(ctx.tx(), &code, id)? {
        return Err(SpecimenError::Duplicate("printer"));
    }
    let mut printer = LabelPrinterView {
        printer_id: id,
        code,
        name,
        transport: input.transport,
        ip_address: None,
        port: None,
        queue: None,
        active: input.active,
        revision,
    };
    set_address(&mut printer, &input, destinations)?;
    repo::save_printer(ctx.tx(), &printer, ctx.now())?;
    let event = configuration_event(actions::CONFIGURE_PRINTER, current, revision)
        .record("printer", id)
        .detail("transport", AuditDetail::code(input.transport.as_str()))
        .detail("active", AuditDetail::boolean(input.active));
    ctx.audit(with_destination(event, &printer)?)?;
    Ok(printer)
}

/// Checks and sets the transport's address fields; the other transport's
/// fields must be absent, and a TCP address must be one `destinations` admits.
fn set_address(
    printer: &mut LabelPrinterView,
    input: &ConfigureLabelPrinterInput,
    destinations: PrinterDestinations,
) -> Result<(), SpecimenError> {
    match input.transport {
        PrinterTransport::Tcp => {
            if input.queue.is_some() {
                return Err(invalid(
                    "queue",
                    "not_applicable",
                    "TCP printers have no queue.",
                ));
            }
            let ip: IpAddr = input
                .ip_address
                .as_deref()
                .unwrap_or_default()
                .trim()
                .parse()
                .map_err(|_| {
                    invalid("ipAddress", "invalid_ip", "Enter the printer's IP address.")
                })?;
            lis_labels::check_destination(ip, destinations).map_err(|_| {
                invalid(
                    "ipAddress",
                    "not_lab_network",
                    "Enter a private or link-local address on the laboratory network.",
                )
            })?;
            if input.port == Some(0) {
                return Err(invalid(
                    "port",
                    "out_of_range",
                    "Enter a port from 1 to 65535.",
                ));
            }
            printer.ip_address = Some(ip.to_string());
            printer.port = Some(input.port.unwrap_or(DEFAULT_TCP_PORT));
        }
        PrinterTransport::Cups => {
            if input.ip_address.is_some() || input.port.is_some() {
                return Err(invalid(
                    "ipAddress",
                    "not_applicable",
                    "CUPS printers are addressed by queue name only.",
                ));
            }
            let queue = input.queue.as_deref().unwrap_or_default().trim();
            QueueName::new(queue).map_err(|_| {
                invalid(
                    "queue",
                    "invalid_queue",
                    "Use letters, digits, dots, dashes or underscores, not starting with a dash.",
                )
            })?;
            printer.queue = Some(queue.to_owned());
        }
    }
    Ok(())
}

/// Adds the printer's destination (IP address and port, or queue) as typed
/// details. These are network or spooler names, not patient data.
fn with_destination(
    mut event: AuditEvent,
    printer: &LabelPrinterView,
) -> Result<AuditEvent, SpecimenError> {
    let token = |value: &str| AuditDetail::machine_token(value).map_err(StoreError::from);
    if let Some(ip) = &printer.ip_address {
        event = event.detail("ip_address", token(ip)?);
    }
    if let Some(port) = printer.port {
        event = event.detail("port", AuditDetail::unsigned(port.into()));
    }
    if let Some(queue) = &printer.queue {
        // CUPS queue names are case-insensitive; machine tokens are lowercase.
        event = event.detail("queue", token(&queue.to_ascii_lowercase())?);
    }
    Ok(event)
}

fn configuration_event(action: &'static str, current: Option<i64>, revision: i64) -> AuditEvent {
    let event = clinical(action).new_revision(revision);
    match current {
        Some(current) => event.old_revision(current),
        None => event,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_needs_no_revision_and_updating_needs_the_current_one() {
        assert_eq!(next_revision(None, None, "printer").unwrap(), 1);
        assert_eq!(next_revision(Some(3), Some(3), "printer").unwrap(), 4);
        assert!(matches!(
            next_revision(Some(3), Some(2), "printer"),
            Err(SpecimenError::RevisionConflict)
        ));
        assert!(matches!(
            next_revision(Some(3), None, "printer"),
            Err(SpecimenError::RevisionConflict)
        ));
    }
}
