//! Adding, changing, starting, stopping and checking connections. Each needs
//! `manage_configuration` and a signed-in person, keeps a history entry and
//! is audited on the `security` stream.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::{Duty, SelfApprovalRefused};
use lis_core::{Actor, Capability, GateReason, required_text};
use lis_store::CommandContext;

use super::error::{PartnerError, invalid};
use super::model::{
    ConnectionChange, CreatePartnerConnection, PartnerConnectionId, RecordPartnerDestinationCheck,
    SetPartnerConnectionActive, UpdatePartnerConnection,
};
use super::repo::{self, ChangeRow, ConnectionRow, VerificationRow};
use super::settings::{valid_folder, valid_settings};

const REASON_MAX: usize = 500;
const RECORD_TYPE: &str = "partner_connection";

/// Adds a connection (REQ-INT-001). It cannot send patient data until its
/// destination check is recorded (REQ-RPT-012).
pub fn create_partner_connection(
    ctx: &mut CommandContext<'_>,
    input: CreatePartnerConnection,
) -> Result<PartnerConnectionId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let settings = valid_settings(input.settings)?;
    valid_folder(&input.folder)?;
    unique_name(ctx, &settings.name, None)?;
    let row = ConnectionRow {
        id: PartnerConnectionId::generate(),
        settings,
        folder: input.folder,
        folder_set_by: actor,
        active: input.active,
        revision: 1,
    };
    repo::insert_connection(ctx.tx(), &row)?;
    record(ctx, &row, ConnectionChange::Created, None)?;
    ctx.audit(
        event("interfaces.create_connection", &row)
            .new_revision(1)
            .detail("active", AuditDetail::boolean(row.active)),
    )?;
    Ok(row.id)
}

/// Changes a connection's settings, and its folder when one was chosen. A new
/// folder needs its own destination check before patient data goes there.
pub fn update_partner_connection(
    ctx: &mut CommandContext<'_>,
    input: UpdatePartnerConnection,
) -> Result<PartnerConnectionId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let reason = reason(&input.reason)?;
    let current = current(ctx, input.connection_id, input.expected_revision)?;
    let settings = valid_settings(input.settings)?;
    if settings.message_type != current.settings.message_type {
        return Err(invalid(
            "settings.messageType",
            "fixed",
            "The message type of a connection cannot change. Add another connection.",
        ));
    }
    unique_name(ctx, &settings.name, Some(current.id))?;
    let (folder, folder_set_by) = match input.folder {
        Some(folder) if folder != current.folder => {
            valid_folder(&folder)?;
            (folder, actor)
        }
        Some(_) | None => (current.folder.clone(), current.folder_set_by.clone()),
    };
    let folder_changed = folder != current.folder;
    let row = ConnectionRow {
        settings,
        folder,
        folder_set_by,
        revision: current.revision + 1,
        ..current
    };
    repo::update_connection(ctx.tx(), &row)?;
    record(ctx, &row, ConnectionChange::Updated, Some(reason.clone()))?;
    ctx.audit(
        event("interfaces.update_connection", &row)
            .old_revision(current.revision)
            .new_revision(row.revision)
            .detail("folder_changed", AuditDetail::boolean(folder_changed))
            .reason(reason),
    )?;
    Ok(row.id)
}

/// Starts or stops a connection. A stopped connection writes nothing; its
/// messages wait and are written after it starts.
pub fn set_partner_connection_active(
    ctx: &mut CommandContext<'_>,
    input: SetPartnerConnectionActive,
) -> Result<PartnerConnectionId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    person(ctx)?;
    let reason = reason(&input.reason)?;
    let current = current(ctx, input.connection_id, input.expected_revision)?;
    if current.active == input.active {
        return Err(PartnerError::WrongState(if input.active {
            "This connection is already started."
        } else {
            "This connection is already stopped."
        }));
    }
    let row = ConnectionRow {
        active: input.active,
        revision: current.revision + 1,
        ..current.clone()
    };
    repo::update_connection(ctx.tx(), &row)?;
    let change = if input.active {
        ConnectionChange::Started
    } else {
        ConnectionChange::Stopped
    };
    record(ctx, &row, change, Some(reason.clone()))?;
    ctx.audit(
        event("interfaces.set_connection_active", &row)
            .old_revision(current.revision)
            .new_revision(row.revision)
            .detail("active", AuditDetail::boolean(input.active))
            .reason(reason),
    )?;
    Ok(row.id)
}

/// Records how the current folder was confirmed as the partner's
/// (REQ-RPT-012). A second person checks it, or the person who chose it where
/// the laboratory's separation-of-duties policy allows and they confirm it
/// (ADR 0015); the event is then marked as a self-approval.
pub fn record_partner_destination_check(
    ctx: &mut CommandContext<'_>,
    input: RecordPartnerDestinationCheck,
) -> Result<PartnerConnectionId, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let method =
        required_text("method", &input.method, REASON_MAX).map_err(PartnerError::Invalid)?;
    let current = current(ctx, input.connection_id, input.expected_revision)?;
    if repo::verification(ctx.tx(), current.id, &current.folder)?.is_some() {
        return Err(PartnerError::WrongState(
            "The destination check for this folder is already recorded.",
        ));
    }
    let self_approval = current.folder_set_by == actor;
    if self_approval {
        authorize_self(ctx, input.self_approval)?;
    }
    repo::insert_verification(
        ctx.tx(),
        current.id,
        &current.folder,
        &VerificationRow {
            method: method.clone(),
            verified_by: actor,
            verified_at: ctx.now(),
            self_approval,
        },
    )?;
    let row = ConnectionRow {
        revision: current.revision + 1,
        ..current.clone()
    };
    repo::update_connection(ctx.tx(), &row)?;
    record(ctx, &row, ConnectionChange::Verified, Some(method))?;
    let mut event = event("interfaces.verify_destination", &row)
        .old_revision(current.revision)
        .new_revision(row.revision);
    if self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    Ok(row.id)
}

/// The signed-in person's ID; listeners and workers cannot configure.
fn person(ctx: &CommandContext<'_>) -> Result<String, PartnerError> {
    match ctx.principal().actor() {
        actor @ Actor::User { .. } => Ok(actor.id_text()),
        Actor::Instrument { .. } | Actor::System { .. } => {
            Err(PartnerError::WrongState("Sign in to change connections."))
        }
    }
}

fn reason(text: &str) -> Result<String, PartnerError> {
    required_text("reason", text, REASON_MAX).map_err(PartnerError::Invalid)
}

/// The connection at the revision shown.
fn current(
    ctx: &CommandContext<'_>,
    id: PartnerConnectionId,
    expected_revision: i64,
) -> Result<ConnectionRow, PartnerError> {
    let row = repo::load_connection(ctx.tx(), id)?.ok_or(PartnerError::NotFound("connection"))?;
    if row.revision == expected_revision {
        Ok(row)
    } else {
        Err(PartnerError::RevisionConflict)
    }
}

fn unique_name(
    ctx: &CommandContext<'_>,
    name: &str,
    except: Option<PartnerConnectionId>,
) -> Result<(), PartnerError> {
    if repo::name_in_use(ctx.tx(), name, except)? {
        return Err(invalid(
            "settings.name",
            "in_use",
            "Another connection has this name.",
        ));
    }
    Ok(())
}

fn record(
    ctx: &CommandContext<'_>,
    row: &ConnectionRow,
    change: ConnectionChange,
    reason: Option<String>,
) -> Result<(), PartnerError> {
    repo::insert_change(
        ctx.tx(),
        row.id,
        &ChangeRow {
            revision: row.revision,
            change,
            settings: repo::snapshot(row)?,
            reason,
            changed_by: ctx.principal().actor().id_text(),
            changed_at: ctx.now(),
        },
    )
}

fn event(action: &'static str, row: &ConnectionRow) -> AuditEvent {
    AuditEvent::new(Stream::Security, action, Outcome::Succeeded)
        .record(RECORD_TYPE, row.id)
        .detail(
            "message_type",
            AuditDetail::code(row.settings.message_type.as_str()),
        )
        .detail(
            "protocol",
            AuditDetail::code(row.settings.protocol.as_str()),
        )
}

fn authorize_self(ctx: &CommandContext<'_>, confirmed: bool) -> Result<(), PartnerError> {
    let policy = lis_auth::current_separation_policy(ctx.tx())?;
    match policy.authorize_self(Duty::DestinationVerification, confirmed) {
        Ok(()) => Ok(()),
        Err(refused @ SelfApprovalRefused::NotAllowed(_)) => {
            Err(PartnerError::GateBlocked(vec![GateReason {
                code: "second_person_required".to_owned(),
                message: refused.to_string(),
            }]))
        }
        Err(SelfApprovalRefused::NotConfirmed(duty)) => {
            Err(PartnerError::Invalid(duty.confirmation_field_error()))
        }
    }
}
