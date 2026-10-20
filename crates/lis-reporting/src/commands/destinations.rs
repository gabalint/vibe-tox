//! Delivery destinations (REQ-RPT-012 local, L10): born unverified,
//! verified by a second person, or by their creator where the laboratory's
//! separation-of-duties policy allows it (ADR 0015), never edited: replaced
//! or retired.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::Duty;
use lis_core::{Capability, UserId};
use lis_identity::{ClientId, ClientLocationId};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::{authorize_self, user_id};
use crate::error::{ReportingError, invalid, optional, required};
use crate::model::{DeliveryChannel, DestinationId};
use crate::queries::destinations::destination_view;
use crate::repo::delivery::{self, DestinationRow, Verification};
use crate::views::DestinationView;

/// Input of `create_delivery_destination`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateDeliveryDestinationInput {
    /// The ordering client.
    pub client_id: ClientId,
    /// One of its locations; client-level when absent.
    #[serde(default)]
    #[ts(optional)]
    pub client_location_id: Option<ClientLocationId>,
    /// The staff-operated channel.
    pub channel: DeliveryChannel,
    /// Fax number, email or postal address; optional for courier; none for pickup.
    #[serde(default)]
    #[ts(optional)]
    pub address: Option<String>,
    /// How staff recognise it.
    pub label: String,
}

/// Input of `verify_delivery_destination`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct VerifyDeliveryDestinationInput {
    /// The destination.
    pub destination_id: DestinationId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// How it was verified, for example a test fax confirmed by phone.
    pub method: String,
    /// The verifier confirms they created this destination themselves and verify
    /// it with no second person, as the laboratory's separation-of-duties policy
    /// allows (ADR 0015). Needed only in that case.
    pub self_approval: bool,
}

/// Input of `replace_delivery_destination`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ReplaceDeliveryDestinationInput {
    /// The destination to retire.
    pub destination_id: DestinationId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The new address, on the same channel.
    #[serde(default)]
    #[ts(optional)]
    pub address: Option<String>,
    /// The new label.
    pub label: String,
    /// Why.
    pub reason: String,
}

/// Input of `retire_delivery_destination`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RetireDeliveryDestinationInput {
    /// The destination.
    pub destination_id: DestinationId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why.
    pub reason: String,
}

/// Creates an unverified destination of a client or one of its locations.
pub fn create_delivery_destination(
    ctx: &mut CommandContext<'_>,
    input: CreateDeliveryDestinationInput,
) -> Result<DestinationView, ReportingError> {
    ctx.require(Capability::ManageConfiguration)?;
    let created_by = user_id(ctx)?;
    let client = lis_identity::get_client(ctx.tx(), input.client_id)?;
    if let Some(location) = input.client_location_id
        && !client.locations.iter().any(|l| l.id == location)
    {
        return Err(invalid(
            "clientLocationId",
            "not_of_client",
            "Choose a location of this client.",
        ));
    }
    let destination = DestinationRow {
        id: DestinationId::generate(),
        client_id: input.client_id,
        client_location_id: input.client_location_id,
        channel: input.channel,
        address: address(input.channel, input.address.as_deref())?,
        label: required("label", &input.label, 200)?,
        replaces_id: None,
        created_by,
        created_at: ctx.now(),
        retired_at: None,
        retire_reason: None,
        revision: 1,
        verification: None,
        reported_wrong_at: None,
    };
    delivery::insert_destination(ctx.tx(), &destination)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::CREATE_DESTINATION,
            Outcome::Succeeded,
        )
        .record("delivery_destination", destination.id)
        .new_revision(1)
        .detail("client_id", AuditDetail::id(destination.client_id))
        .detail("channel", AuditDetail::code(destination.channel.as_str())),
    )?;
    destination_view(ctx.tx(), &destination)
}

/// The address a channel needs, checked for its form: a fax number of 7–20
/// digits, an email address, a postal address, an optional courier note,
/// and nothing for pickup.
fn address(
    channel: DeliveryChannel,
    value: Option<&str>,
) -> Result<Option<String>, ReportingError> {
    let value = optional("address", value, 500)?;
    let bad = |message: &str| Err(invalid("address", "format", message));
    match (channel, value) {
        (DeliveryChannel::Pickup, Some(_)) => bad("Pickup at the laboratory takes no address."),
        (DeliveryChannel::Pickup | DeliveryChannel::Courier, None) => Ok(None),
        (DeliveryChannel::Courier | DeliveryChannel::Mail, Some(text)) => Ok(Some(text)),
        (DeliveryChannel::Fax | DeliveryChannel::Email | DeliveryChannel::Mail, None) => Err(
            invalid("address", "required", "Enter the address for this channel."),
        ),
        (DeliveryChannel::Fax, Some(text)) => {
            let digits = text.chars().filter(char::is_ascii_digit).count();
            let allowed = text
                .chars()
                .all(|c| c.is_ascii_digit() || " +-().".contains(c));
            if allowed && (7..=20).contains(&digits) {
                Ok(Some(text))
            } else {
                bad("Enter a fax number of 7 to 20 digits.")
            }
        }
        (DeliveryChannel::Email, Some(text)) => {
            let valid = text.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.contains('@')
            }) && !text.contains(char::is_whitespace);
            if valid {
                Ok(Some(text))
            } else {
                bad("Enter an email address.")
            }
        }
    }
}

/// An active destination at the revision shown.
fn active(
    ctx: &CommandContext<'_>,
    id: DestinationId,
    expected_revision: i64,
) -> Result<DestinationRow, ReportingError> {
    let destination =
        delivery::destination(ctx.tx(), id)?.ok_or(ReportingError::NotFound("destination"))?;
    if destination.revision != expected_revision {
        return Err(ReportingError::RevisionConflict);
    }
    if destination.retired_at.is_some() {
        return Err(ReportingError::WrongState("destination"));
    }
    Ok(destination)
}

/// A second person verifies a destination before its first delivery (L10,
/// REQ-RPT-012), or its creator where the separation-of-duties policy in
/// force allows it and they confirm it (ADR 0015); the event is then marked
/// as a self-approval.
pub fn verify_delivery_destination(
    ctx: &mut CommandContext<'_>,
    input: VerifyDeliveryDestinationInput,
) -> Result<DestinationView, ReportingError> {
    ctx.require(Capability::ManageConfiguration)?;
    let verified_by = user_id(ctx)?;
    let method = required("method", &input.method, 500)?;
    let mut destination = active(ctx, input.destination_id, input.expected_revision)?;
    if destination.verification.is_some() {
        return Err(ReportingError::WrongState("destination"));
    }
    let by_creator = destination.created_by == verified_by;
    if by_creator {
        authorize_self(
            ctx.tx(),
            Duty::DestinationVerification,
            input.self_approval,
            "second_person_required",
        )?;
    }
    let verification = Verification {
        method,
        verified_by,
        verified_at: ctx.now(),
    };
    delivery::insert_verification(ctx.tx(), destination.id, &verification)?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        actions::VERIFY_DESTINATION,
        Outcome::Succeeded,
    )
    .record("delivery_destination", destination.id)
    .detail("created_by", AuditDetail::id(destination.created_by));
    if by_creator {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    destination.verification = Some(verification);
    destination_view(ctx.tx(), &destination)
}

/// Retires a destination and creates its unverified replacement on the
/// same client, location and channel. Open obligations need a redirect.
pub fn replace_delivery_destination(
    ctx: &mut CommandContext<'_>,
    input: ReplaceDeliveryDestinationInput,
) -> Result<DestinationView, ReportingError> {
    ctx.require(Capability::ManageConfiguration)?;
    let by = user_id(ctx)?;
    let reason = required("reason", &input.reason, 1000)?;
    let old = active(ctx, input.destination_id, input.expected_revision)?;
    let replacement = DestinationRow {
        id: DestinationId::generate(),
        address: address(old.channel, input.address.as_deref())?,
        label: required("label", &input.label, 200)?,
        replaces_id: Some(old.id),
        created_by: by,
        created_at: ctx.now(),
        retired_at: None,
        retire_reason: None,
        revision: 1,
        verification: None,
        reported_wrong_at: None,
        ..old
    };
    retire(ctx, &old, by, &reason)?;
    delivery::insert_destination(ctx.tx(), &replacement)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::REPLACE_DESTINATION,
            Outcome::Succeeded,
        )
        .record("delivery_destination", replacement.id)
        .new_revision(1)
        .detail("old_destination_id", AuditDetail::id(old.id))
        .detail("new_destination_id", AuditDetail::id(replacement.id))
        .reason(reason),
    )?;
    destination_view(ctx.tx(), &replacement)
}

/// Retires a destination; its open obligations show as blocked until redirected.
pub fn retire_delivery_destination(
    ctx: &mut CommandContext<'_>,
    input: RetireDeliveryDestinationInput,
) -> Result<DestinationView, ReportingError> {
    ctx.require(Capability::ManageConfiguration)?;
    let by = user_id(ctx)?;
    let reason = required("reason", &input.reason, 1000)?;
    let destination = active(ctx, input.destination_id, input.expected_revision)?;
    retire(ctx, &destination, by, &reason)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::RETIRE_DESTINATION,
            Outcome::Succeeded,
        )
        .record("delivery_destination", destination.id)
        .old_revision(destination.revision)
        .new_revision(destination.revision + 1)
        .reason(reason),
    )?;
    let retired = delivery::destination(ctx.tx(), destination.id)?
        .ok_or(ReportingError::NotFound("destination"))?;
    destination_view(ctx.tx(), &retired)
}

fn retire(
    ctx: &CommandContext<'_>,
    destination: &DestinationRow,
    by: UserId,
    reason: &str,
) -> Result<(), ReportingError> {
    if delivery::retire_destination(
        ctx.tx(),
        destination.id,
        destination.revision,
        by,
        ctx.now(),
        reason,
    )? {
        Ok(())
    } else {
        Err(ReportingError::RevisionConflict)
    }
}
