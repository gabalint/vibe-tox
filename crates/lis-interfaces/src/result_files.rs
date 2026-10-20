//! Result-file receipts (REQ-INT-009; result-files brief F1, F13, F14, F16).
//!
//! A file becomes an ordinary terminal receipt with the transport
//! `result_file`, written in one transaction with its file facts, its row in
//! the ledger of files already received, an audit event and, unless it
//! duplicates an earlier receipt, the `ifc.receipt.completed` outbox item. The
//! caller stores the exact bytes in the artifact store first, so nothing is
//! parsed before the bytes are durable. File names are stored, never logged
//! or audited.
//!
//! Who acts (F16): a folder pickup is the instrument's own principal, as for
//! the ASTM listeners; a manual load is a user with `perform_analysis`,
//! recorded as the one who loaded it.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Actor, Capability, Denied, InstrumentId, UserId, UtcTimestamp};
use lis_store::{ArtifactDigest, CommandContext, StoreError};
use rusqlite::Connection;

use crate::commands::{enqueue_import, require_matching_instrument};
use crate::model::{
    AbortCode, FileFacts, FileSource, Instrument, InstrumentTransport, Receipt, ReceiptFileView,
    ReceiptId, ReceiptState, ReceiveResultFileInput,
};
use crate::{InterfaceError, MAX_RESULT_FILE_BYTES, repo};

/// Longest file name accepted, in bytes; file systems allow at most 255 characters.
const MAX_FILE_NAME_BYTES: usize = 1024;

/// Records a received result file (F1): a complete receipt whose duplicate
/// check, audit and import follow the ASTM receipt rules.
pub fn receive_result_file(
    ctx: &mut CommandContext<'_>,
    input: ReceiveResultFileInput,
) -> Result<Receipt, InterfaceError> {
    let admitted = admit(ctx, &input.file)?;
    if input.file.byte_len > MAX_RESULT_FILE_BYTES {
        return Err(InterfaceError::InvalidFileSize);
    }
    let digest = verified_digest(&input)?;
    let file = &input.file;
    // F12: identical bytes from the same instrument give a duplicate receipt.
    let duplicate_of = repo::first_duplicate(ctx.tx(), file.instrument_id, &digest)?;
    let now = ctx.now();
    let receipt = Receipt {
        message_sha256: Some(digest.clone()),
        artifact_digest: Some(digest.clone()),
        duplicate_of,
        ..file_receipt(file.instrument_id, now, ReceiptState::Complete)
    };
    admitted.record(ctx, &receipt, file)?;
    ctx.audit(
        file_event("interface.receive_result_file", &receipt, file)
            .detail(
                "message_sha256",
                AuditDetail::sha256(&digest).map_err(StoreError::from)?,
            )
            .detail("duplicate", AuditDetail::boolean(duplicate_of.is_some())),
    )?;
    if duplicate_of.is_none() {
        enqueue_import(ctx, receipt.id)?;
    }
    Ok(receipt)
}

/// Records a result file over [`MAX_RESULT_FILE_BYTES`] that was not read: an
/// aborted receipt (`file_too_large`) with no artifact (F14).
pub fn record_oversize_file(
    ctx: &mut CommandContext<'_>,
    file: FileFacts,
) -> Result<Receipt, InterfaceError> {
    let admitted = admit(ctx, &file)?;
    if file.byte_len <= MAX_RESULT_FILE_BYTES {
        return Err(InterfaceError::InvalidFileSize);
    }
    let receipt = Receipt {
        abort_reason: Some(AbortCode::FileTooLarge),
        ..file_receipt(file.instrument_id, ctx.now(), ReceiptState::Aborted)
    };
    admitted.record(ctx, &receipt, &file)?;
    ctx.audit(
        file_event("interface.record_oversize_file", &receipt, &file).detail(
            "reason_code",
            AuditDetail::code(AbortCode::FileTooLarge.as_str()),
        ),
    )?;
    Ok(receipt)
}

/// The receipt of the file with this name, size and modification time, if the
/// instrument received it before (F13). The caller authorizes.
pub fn seen_file(
    conn: &Connection,
    instrument_id: InstrumentId,
    file_name: &str,
    byte_len: u64,
    modified_at: UtcTimestamp,
) -> Result<Option<ReceiptId>, InterfaceError> {
    repo::seen_receipt(conn, instrument_id, file_name, byte_len, modified_at)
}

/// The file facts of a result-file receipt; `None` for an ASTM receipt. The
/// caller authorizes.
pub fn receipt_file(
    conn: &Connection,
    receipt_id: ReceiptId,
) -> Result<Option<ReceiptFileView>, InterfaceError> {
    repo::receipt_file(conn, receipt_id)
}

/// Whether the file with this name, size and modification time was already in
/// `folder` when it was chosen for the instrument (brief §6, "Present at
/// selection"): the poller skips it. The caller authorizes.
pub fn present_at_selection(
    conn: &Connection,
    instrument_id: InstrumentId,
    folder: &str,
    file_name: &str,
    byte_len: u64,
    modified_at: UtcTimestamp,
) -> Result<bool, InterfaceError> {
    repo::in_baseline(
        conn,
        instrument_id,
        folder,
        file_name,
        byte_len,
        modified_at,
    )
}

/// The instrument, when it can receive a result file now: it exists, is
/// enabled and its connection type is result files. Intake reads this before
/// it stores a file's bytes, and the receipt rechecks it in its transaction.
/// The caller authorizes.
pub fn receivable_instrument(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<Instrument, InterfaceError> {
    let instrument =
        repo::instrument_by_id(conn, instrument_id)?.ok_or(InterfaceError::InstrumentNotFound)?;
    if !instrument.enabled {
        return Err(InterfaceError::InstrumentDisabled);
    }
    if instrument.connection.transport() != InstrumentTransport::ResultFile {
        return Err(InterfaceError::TransportMismatch);
    }
    Ok(instrument)
}

/// Every result-file instrument, for the engine's folder poller, which runs
/// without a signed-in user (REQ-OPS-013). Folder paths are laboratory
/// configuration, not patient data.
pub fn result_file_instruments(conn: &Connection) -> Result<Vec<Instrument>, InterfaceError> {
    let mut instruments = repo::all_instruments(conn)?;
    instruments
        .retain(|instrument| instrument.connection.transport() == InstrumentTransport::ResultFile);
    Ok(instruments)
}

/// What the gates of a file receipt found.
struct Admitted {
    /// The user of a manual load.
    loaded_by: Option<UserId>,
    /// False when the ledger already holds this file, which only a manual load
    /// may receive again (as a duplicate receipt).
    first_time: bool,
}

impl Admitted {
    /// Writes the receipt and its file facts and, for a file not received before, its
    /// row in the ledger.
    fn record(
        &self,
        ctx: &CommandContext<'_>,
        receipt: &Receipt,
        file: &FileFacts,
    ) -> Result<(), InterfaceError> {
        repo::insert_file_receipt(ctx.tx(), receipt, file, self.loaded_by)?;
        if self.first_time {
            repo::insert_seen_file(ctx.tx(), file, receipt.id)?;
        }
        Ok(())
    }
}

/// The gates both file receipts share, read inside the transaction: who acts
/// (F16), the file name, the instrument (enabled, `result_file`), and the
/// ledger of files already received; for a pickup also the folder it was
/// listed in, which must still be the instrument's, and the files present
/// when that folder was chosen (§6).
fn admit(ctx: &mut CommandContext<'_>, file: &FileFacts) -> Result<Admitted, InterfaceError> {
    let loaded_by = authorize(ctx, file)?;
    let name = file.file_name.expose();
    if name.is_empty() || name.len() > MAX_FILE_NAME_BYTES {
        return Err(InterfaceError::InvalidFileName);
    }
    let instrument = receivable_instrument(ctx.tx(), file.instrument_id)?;
    let seen = repo::seen_receipt(
        ctx.tx(),
        file.instrument_id,
        name,
        file.byte_len,
        file.modified_at,
    )?;
    if let FileSource::Folder(folder) = &file.source {
        match instrument.result_folder.as_deref() {
            None => return Err(InterfaceError::ResultFolderNotSet),
            Some(watched) if watched != folder => return Err(InterfaceError::ResultFolderChanged),
            Some(_) => {}
        }
        if seen.is_some() {
            return Err(InterfaceError::FileAlreadyReceived);
        }
        let (len, modified_at) = (file.byte_len, file.modified_at);
        if repo::in_baseline(ctx.tx(), file.instrument_id, folder, name, len, modified_at)? {
            return Err(InterfaceError::FilePresentAtSelection);
        }
    }
    Ok(Admitted {
        loaded_by,
        first_time: seen.is_none(),
    })
}

/// F16: a pickup needs the instrument's own principal; a manual load needs a
/// user with `perform_analysis`, who is returned to be recorded.
fn authorize(
    ctx: &mut CommandContext<'_>,
    file: &FileFacts,
) -> Result<Option<UserId>, InterfaceError> {
    match file.source {
        FileSource::Folder(_) => {
            ctx.require(Capability::InstrumentIntake)?;
            require_matching_instrument(ctx, file.instrument_id)?;
            Ok(None)
        }
        FileSource::Upload => {
            ctx.require(Capability::PerformAnalysis)?;
            match ctx.principal().actor() {
                Actor::User { user_id, .. } => Ok(Some(*user_id)),
                Actor::Instrument { .. } | Actor::System { .. } => {
                    Err(InterfaceError::Store(StoreError::from(Denied {
                        capability: Capability::PerformAnalysis,
                    })))
                }
            }
        }
    }
}

/// The digest both columns record. The artifact store names a file by the
/// SHA-256 of its bytes, so the two must be one valid digest.
fn verified_digest(input: &ReceiveResultFileInput) -> Result<String, InterfaceError> {
    let digest: ArtifactDigest = input
        .artifact_digest
        .parse()
        .map_err(|_| InterfaceError::ArtifactMismatch)?;
    if input.sha256 != input.artifact_digest {
        return Err(InterfaceError::ArtifactMismatch);
    }
    Ok(digest.to_string())
}

/// A file receipt that became `state` at `now`: a file is received whole, so it
/// starts and ends at once, without a peer or frames.
fn file_receipt(instrument_id: InstrumentId, now: UtcTimestamp, state: ReceiptState) -> Receipt {
    Receipt {
        id: ReceiptId::generate(),
        instrument_id,
        transport: InstrumentTransport::ResultFile,
        peer: None,
        started_at: now,
        completed_at: Some(now),
        state,
        abort_reason: None,
        frame_count: 0,
        message_sha256: None,
        artifact_digest: None,
        duplicate_of: None,
    }
}

/// The audit event of a file receipt: instrument, origin and size, never the
/// file name.
fn file_event(action: &'static str, receipt: &Receipt, file: &FileFacts) -> AuditEvent {
    AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
        .record("interface_receipt", receipt.id)
        .detail("instrument_id", AuditDetail::id(file.instrument_id))
        .detail("origin", AuditDetail::code(file.source.origin().as_str()))
        .detail("byte_len", AuditDetail::unsigned(file.byte_len))
}
