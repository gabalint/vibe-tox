//! Adding, changing, starting, stopping and checking connections. Each needs
//! `manage_configuration` and a signed-in person, keeps a history entry and
//! is audited on the `security` stream.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::{Duty, SelfApprovalRefused};
use lis_core::{Actor, Capability, GateReason, required_text};
use lis_store::CommandContext;

use super::error::{PartnerError, invalid};
use super::intake_settings::moved_to;
use super::model::{
    ConnectionChange, CreatePartnerConnection, PartnerConnectionId, PartnerConnectionSettings,
    RecordPartnerDestinationCheck, SetPartnerConnectionActive, SftpAuth, SftpSettings,
    UpdatePartnerConnection,
};
use super::repo::{self, ChangeRow, ConnectionRow, VerificationRow};
use super::settings::{valid_folder, valid_settings};
use super::sftp_settings::destination_of;

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
    let folder = match &settings.sftp {
        Some(sftp) => sftp.remote_folder.clone(),
        // An MLLP connection sends to, or listens for, an address: it has no folder.
        None if settings.over_mllp() => String::new(),
        None => {
            valid_folder(&input.folder)?;
            input.folder
        }
    };
    unique_name(ctx, &settings.name, None)?;
    let sealed_password = password_for(&settings, input.sealed_password, None)?;
    let moved_to = moved_to(&settings, input.moved_to, None)?;
    let row = ConnectionRow {
        id: PartnerConnectionId::generate(),
        destination: destination_of(&settings, &folder),
        settings,
        folder,
        moved_to,
        folder_set_by: actor,
        sealed_password,
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

/// Changes a connection's settings, its folder when one was chosen, and its SFTP password
/// when one was given. A new destination (folder, or server, user, port, remote folder or
/// fingerprint) needs its own destination check before patient data goes there.
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
    let folder = match (&settings.sftp, input.folder) {
        (Some(sftp), _) => sftp.remote_folder.clone(),
        (None, _) if settings.over_mllp() => String::new(),
        (None, Some(folder)) => {
            valid_folder(&folder)?;
            folder
        }
        // The current folder is kept only while the connection writes to a folder.
        (None, None) if current.settings.sftp.is_none() && !current.settings.over_mllp() => {
            current.folder.clone()
        }
        (None, None) => return Err(invalid("folder", "required", "Choose the folder.")),
    };
    // A kept password is never sent to another server or account: those need it again.
    let account = |sftp: &Option<SftpSettings>| {
        sftp.as_ref()
            .map(|sftp| (sftp.host.clone(), sftp.port, sftp.username.clone()))
    };
    let previous = (account(&settings.sftp) == account(&current.settings.sftp))
        .then(|| current.sealed_password.clone())
        .flatten();
    let sealed_password = password_for(&settings, input.sealed_password, previous)?;
    // Local Processed and Failed folders stay only while the connection reads a folder.
    let local = current
        .moved_to
        .as_ref()
        .filter(|_| current.settings.sftp.is_none());
    let moved_to = moved_to(&settings, input.moved_to, local)?;
    let destination = destination_of(&settings, &folder);
    let destination_changed = destination != current.destination;
    let row = ConnectionRow {
        settings,
        folder,
        moved_to,
        folder_set_by: if destination_changed {
            actor
        } else {
            current.folder_set_by.clone()
        },
        sealed_password,
        destination,
        revision: current.revision + 1,
        ..current
    };
    repo::update_connection(ctx.tx(), &row)?;
    record(ctx, &row, ConnectionChange::Updated, Some(reason.clone()))?;
    ctx.audit(
        event("interfaces.update_connection", &row)
            .old_revision(current.revision)
            .new_revision(row.revision)
            .detail("folder_changed", AuditDetail::boolean(destination_changed))
            .reason(reason),
    )?;
    Ok(row.id)
}

/// The sealed password to keep: only for SFTP password sign-in, the new one or else the
/// one already kept. The engine sealed it; the password itself never reaches this module.
fn password_for(
    settings: &PartnerConnectionSettings,
    new: Option<Vec<u8>>,
    kept: Option<Vec<u8>>,
) -> Result<Option<Vec<u8>>, PartnerError> {
    match settings.sftp.as_ref().map(|sftp| sftp.auth) {
        Some(SftpAuth::Password) => new.or(kept).map(Some).ok_or_else(|| {
            invalid(
                "settings.sftp.password",
                "required",
                "Enter the password for this server.",
            )
        }),
        Some(SftpAuth::Key) | None => Ok(None),
    }
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

/// Records how the current destination was confirmed as the partner's
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
    if repo::verification(ctx.tx(), current.id, &current.destination)?.is_some() {
        return Err(PartnerError::WrongState(
            "The destination check for this folder is already recorded.",
        ));
    }
    let self_approval = current.folder_set_by == actor;
    if self_approval {
        authorize_self(ctx, Duty::DestinationVerification, input.self_approval)?;
    }
    repo::insert_verification(
        ctx.tx(),
        current.id,
        &current.destination,
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
pub(super) fn person(ctx: &CommandContext<'_>) -> Result<String, PartnerError> {
    match ctx.principal().actor() {
        actor @ Actor::User { .. } => Ok(actor.id_text()),
        Actor::Instrument { .. } | Actor::System { .. } => {
            Err(PartnerError::WrongState("Sign in to change connections."))
        }
    }
}

pub(super) fn reason(text: &str) -> Result<String, PartnerError> {
    required_text("reason", text, REASON_MAX).map_err(PartnerError::Invalid)
}

/// The connection at the revision shown.
pub(super) fn current(
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

pub(super) fn record(
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

pub(super) fn event(action: &'static str, row: &ConnectionRow) -> AuditEvent {
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

pub(super) fn authorize_self(
    ctx: &CommandContext<'_>,
    duty: Duty,
    confirmed: bool,
) -> Result<(), PartnerError> {
    let policy = lis_auth::current_separation_policy(ctx.tx())?;
    match policy.authorize_self(duty, confirmed) {
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
