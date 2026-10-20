use std::path::Path;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, InstrumentId};
use lis_store::CommandContext;

use crate::commands::required_instrument;
use crate::model::{
    Instrument, InstrumentTransport, RegisterInstrument, SetInstrumentEnabled, SetResultFolder,
    UpdateInstrumentConnection,
};
use crate::validation::{valid_text, validate_identity};
use crate::{InterfaceError, repo};

/// Registers a versioned analyzer connection (REQ-INT-009).
pub fn register_instrument(
    ctx: &mut CommandContext<'_>,
    input: RegisterInstrument,
) -> Result<Instrument, InterfaceError> {
    ctx.require(Capability::ManageConfiguration)?;
    validate_identity(&input.code, &input.name)?;
    input.connection.validate()?;
    if repo::code_in_use(ctx.tx(), &input.code)? {
        return Err(InterfaceError::InstrumentCodeInUse);
    }
    if repo::connection_in_use(ctx.tx(), &input.connection, None)? {
        return Err(InterfaceError::ConnectionInUse);
    }
    let instrument = Instrument {
        id: InstrumentId::generate(),
        code: input.code,
        name: input.name,
        connection: input.connection,
        // A watched folder is chosen afterwards, in a system dialog (F13).
        result_folder: None,
        enabled: input.enabled,
        revision: 1,
    };
    repo::insert_instrument(ctx.tx(), &instrument)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interface.register_instrument",
            Outcome::Succeeded,
        )
        .record("instrument", instrument.id)
        .new_revision(1)
        .detail(
            "transport",
            AuditDetail::code(instrument.connection.transport().as_str()),
        )
        .detail("enabled", AuditDetail::boolean(instrument.enabled)),
    )?;
    Ok(instrument)
}

/// Replaces connection settings under optimistic concurrency (REQ-INT-009).
pub fn update_instrument_connection(
    ctx: &mut CommandContext<'_>,
    input: UpdateInstrumentConnection,
) -> Result<Instrument, InterfaceError> {
    ctx.require(Capability::ManageConfiguration)?;
    input.connection.validate()?;
    let current = required_instrument(ctx, input.instrument_id)?;
    ensure_revision(&current, input.expected_revision)?;
    if repo::connection_in_use(ctx.tx(), &input.connection, Some(input.instrument_id))? {
        return Err(InterfaceError::ConnectionInUse);
    }
    let next_revision = current.revision + 1;
    repo::update_connection(
        ctx.tx(),
        input.instrument_id,
        &input.connection,
        next_revision,
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interface.update_connection",
            Outcome::Succeeded,
        )
        .record("instrument", input.instrument_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .detail(
            "transport",
            AuditDetail::code(input.connection.transport().as_str()),
        ),
    )?;
    let keeps_folder = input.connection.transport() == InstrumentTransport::ResultFile;
    Ok(Instrument {
        connection: input.connection,
        result_folder: current.result_folder.filter(|_| keeps_folder),
        revision: next_revision,
        ..current
    })
}

/// Enables or disables a listener configuration under optimistic concurrency.
pub fn set_instrument_enabled(
    ctx: &mut CommandContext<'_>,
    input: SetInstrumentEnabled,
) -> Result<Instrument, InterfaceError> {
    ctx.require(Capability::ManageConfiguration)?;
    let current = required_instrument(ctx, input.instrument_id)?;
    ensure_revision(&current, input.expected_revision)?;
    let next_revision = current.revision + 1;
    repo::update_enabled(ctx.tx(), input.instrument_id, input.enabled, next_revision)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interface.set_enabled",
            Outcome::Succeeded,
        )
        .record("instrument", input.instrument_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .detail("enabled", AuditDetail::boolean(input.enabled)),
    )?;
    Ok(Instrument {
        enabled: input.enabled,
        revision: next_revision,
        ..current
    })
}

/// Sets or clears the watched folder of a result-file instrument (F13). The
/// engine canonicalizes the folder the system dialog returned, checks that it
/// can be read and lists the files already in it; here it must be an absolute
/// path that no other instrument watches. Those files are recorded so the
/// poller never picks them up (brief §6, "Present at selection"); they can
/// still be loaded by hand. The poller reads the configuration at every scan,
/// so no listener restart is needed.
pub fn set_result_folder(
    ctx: &mut CommandContext<'_>,
    input: SetResultFolder,
) -> Result<Instrument, InterfaceError> {
    ctx.require(Capability::ManageConfiguration)?;
    let path = input.folder.as_ref().map(|folder| folder.path.as_str());
    if let Some(path) = path
        && !(valid_text(path, 4096) && Path::new(path).is_absolute())
    {
        return Err(InterfaceError::InvalidResultFolder);
    }
    let current = required_instrument(ctx, input.instrument_id)?;
    ensure_revision(&current, input.expected_revision)?;
    if current.connection.transport() != InstrumentTransport::ResultFile {
        return Err(InterfaceError::TransportMismatch);
    }
    if let Some(path) = path
        && repo::result_folder_in_use(ctx.tx(), path, input.instrument_id)?
    {
        return Err(InterfaceError::ResultFolderInUse);
    }
    let next_revision = current.revision + 1;
    repo::update_result_folder(ctx.tx(), input.instrument_id, path, next_revision)?;
    let present = input
        .folder
        .as_ref()
        .map_or(&[][..], |folder| &folder.present);
    if let Some(path) = path {
        for file in present {
            if file.file_name.expose().is_empty() {
                return Err(InterfaceError::InvalidFileName);
            }
            repo::insert_baseline_file(ctx.tx(), input.instrument_id, path, file, ctx.now())?;
        }
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interface.set_result_folder",
            Outcome::Succeeded,
        )
        .record("instrument", input.instrument_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .detail("folder_set", AuditDetail::boolean(path.is_some()))
        .detail("files_present", AuditDetail::count(present.len())),
    )?;
    Ok(Instrument {
        result_folder: input.folder.map(|folder| folder.path),
        revision: next_revision,
        ..current
    })
}

fn ensure_revision(instrument: &Instrument, expected: i64) -> Result<(), InterfaceError> {
    if instrument.revision == expected {
        Ok(())
    } else {
        Err(InterfaceError::RevisionConflict)
    }
}
