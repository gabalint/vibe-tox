//! The recovery kit (`vtkit-1`; REQ-OPS-007, NFR-30; M4 design brief §3.3, §3.9): a
//! plain-text file holding the installation key and what a replacement computer needs to
//! restore. This module is the one home of the kit text and of the `vtk1-` key line.
//!
//! Invariants:
//! - the key line is the only line that holds key material; the fingerprint is the key
//!   ID, derived one-way;
//! - an intent is committed before the file is written; the file is removed when its
//!   completion record fails; audit holds the kit number, key ID and a filename digest,
//!   never a path, user-typed filename or key;
//! - a kit is *outdated* when none was exported, or the key ID or the backup locations
//!   changed since the latest export;
//! - only the latest kit can be confirmed as stored, and where it is stored is the
//!   confirmation's audit reason.
//!
//! The engine adds fresh authentication and the save dialog.

mod status;

use std::fs;
use std::path::{Path, PathBuf};

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::DataKey;
use lis_core::{Capability, Id, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::backup::BackupError;
use crate::backup::durable::write_owner_file;
use crate::backup::kit_text::{KitHeader, kit_text};
use crate::format::{BackupKey, KeyId};
use crate::installation_id;
use crate::targets;
use status::targets_digest;
pub use status::{
    KitRecord, KitStatus, OutdatedReason, StorageConfirmation, latest_key_fingerprint, status,
    status_without_key,
};

/// Audit action of a kit export.
pub const EXPORT_RECOVERY_KIT: &str = "ops.export_recovery_kit";
/// Audit action committed before recovery-kit key material is written to disk.
pub const EXPORT_RECOVERY_KIT_INTENT: &str = "ops.export_recovery_kit_intent";
/// Audit action of a storage confirmation.
pub const CONFIRM_RECOVERY_KIT: &str = "ops.confirm_recovery_kit";

const REFERENCE_MAX_CHARS: usize = 200;
const NOTE_MAX_CHARS: usize = 500;

pub use crate::backup::kit_text::{key_text, parse_key};

/// Marker type of kit export IDs.
pub enum RecoveryKit {}
/// Identifies one kit export.
pub type KitId = Id<RecoveryKit>;
/// Marker type of storage confirmation IDs.
pub enum KitConfirmation {}
/// Identifies one storage confirmation.
pub type ConfirmationId = Id<KitConfirmation>;

/// What the kit records besides the key; the engine supplies it.
#[derive(Debug, Clone, Copy)]
pub struct KitInput<'a> {
    /// The exporting user's sign-in name.
    pub created_by_name: &'a str,
    /// The OS name and version, as the engine reports it.
    pub operating_system: &'a str,
    /// The application's data folder.
    pub data_dir: &'a Path,
    /// The laboratory's description of where the disk-encryption recovery key is kept.
    pub disk_encryption_reference: &'a str,
}

/// A written kit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitExport {
    /// The kit number, from 1.
    pub kit_no: i64,
    /// The file name written.
    pub file_name: String,
}

/// A recovery-kit export authorized and audited before its key material leaves memory.
/// The engine commits the transaction returning this value, writes it outside every
/// transaction, then records it in a second transaction.
pub struct PreparedKit {
    kit_id: KitId,
    kit_no: i64,
    key_id: KeyId,
    targets_digest: String,
    reference: String,
    destination: PathBuf,
    file_name: String,
    text: zeroize::Zeroizing<String>,
}

impl std::fmt::Debug for PreparedKit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedKit")
            .field("kit_id", &self.kit_id)
            .field("kit_no", &self.kit_no)
            .field("key_id", &self.key_id)
            .field("file_name", &self.file_name)
            .field("text", &"[redacted]")
            .finish()
    }
}

/// The default file name the save dialog proposes: `vibe-tox-recovery-kit-<YYYY-MM-DD>.txt`.
#[must_use]
pub fn default_file_name(now: UtcTimestamp) -> String {
    let date: String = now.to_string().chars().take(10).collect();
    format!("vibe-tox-recovery-kit-{date}.txt")
}

/// Authorizes and audits the intent to export a kit. Commit this transaction before
/// calling [`write_prepared`], so a crash can never leave an unaudited key file.
pub fn prepare_export(
    ctx: &mut CommandContext<'_>,
    installation_key: &DataKey,
    input: &KitInput<'_>,
    destination: &Path,
) -> Result<PreparedKit, BackupError> {
    ctx.require(Capability::BackupRestore)?;
    let reference = required_text(
        "diskEncryptionReference",
        input.disk_encryption_reference,
        REFERENCE_MAX_CHARS,
    )
    .map_err(BackupError::Invalid)?;
    let installation = installation_id(ctx.tx())?.ok_or(BackupError::NoInstallation)?;
    let kit_no = next_kit_no(ctx.tx())?;
    let locations = targets::active(ctx.tx())?;
    let key_id = BackupKey::derive(installation_key).key_id();
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(BackupError::KitNotWritten)?
        .to_owned();
    let file_name_sha256 = hex::encode(Sha256::digest(file_name.as_bytes()));
    let header = KitHeader {
        installation: installation.to_string(),
        kit_no,
        created_at: ctx.now(),
        created_by_name: input.created_by_name,
        operating_system: input.operating_system,
        data_dir: input.data_dir,
        reference: &reference,
    };
    let text = kit_text(installation_key, &header, &locations);
    let kit_id = KitId::generate();
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            EXPORT_RECOVERY_KIT_INTENT,
            Outcome::Succeeded,
        )
        .record("recovery_kit", kit_id)
        .detail("kit_no", AuditDetail::integer(kit_no))
        .detail("key_id", AuditDetail::machine_token(&key_id.to_string())?)
        .detail("file_name_sha256", AuditDetail::sha256(&file_name_sha256)?),
    )?;
    Ok(PreparedKit {
        kit_id,
        kit_no,
        key_id,
        targets_digest: targets_digest(&locations),
        reference,
        destination: destination.to_path_buf(),
        file_name,
        text,
    })
}

/// Writes an authorized kit outside a database transaction. Unix permissions are 0600;
/// Windows inherits the selected folder's ACL.
pub fn write_prepared(prepared: &PreparedKit) -> Result<(), BackupError> {
    write_owner_file(&prepared.destination, prepared.text.as_bytes()).map_err(|error| {
        tracing::warn!(kit_no = prepared.kit_no, error_kind = ?error.kind(), "recovery kit not written");
        BackupError::KitNotWritten
    })
}

/// Records a successfully written kit and its completion audit event.
pub fn record_prepared(
    ctx: &mut CommandContext<'_>,
    prepared: &PreparedKit,
) -> Result<KitExport, BackupError> {
    ctx.require(Capability::BackupRestore)?;
    if next_kit_no(ctx.tx())? != prepared.kit_no {
        return Err(BackupError::KitNotLatest);
    }
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_recovery_kits (id, kit_no, key_id, targets_digest, \
             disk_encryption_reference, file_name, exported_by, exported_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?
        .execute(params![
            prepared.kit_id,
            prepared.kit_no,
            prepared.key_id.to_string(),
            prepared.targets_digest,
            prepared.reference,
            prepared.file_name,
            ctx.principal().actor().id_text(),
            ctx.now(),
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, EXPORT_RECOVERY_KIT, Outcome::Succeeded)
            .record("recovery_kit", prepared.kit_id)
            .detail("kit_no", AuditDetail::integer(prepared.kit_no))
            .detail(
                "key_id",
                AuditDetail::machine_token(&prepared.key_id.to_string())?,
            ),
    )?;
    Ok(KitExport {
        kit_no: prepared.kit_no,
        file_name: prepared.file_name.clone(),
    })
}

/// Removes a kit whose completion could not be recorded.
pub fn discard_prepared(prepared: &PreparedKit) {
    if let Err(error) = fs::remove_file(&prepared.destination)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::error!(kit_no = prepared.kit_no, error_kind = ?error.kind(), "unrecorded recovery kit not removed");
    }
}

/// Compatibility helper for domain tests. Production callers use the three-phase
/// [`prepare_export`], [`write_prepared`], [`record_prepared`] sequence so file I/O is
/// outside transactions.
pub fn export_kit(
    ctx: &mut CommandContext<'_>,
    installation_key: &DataKey,
    input: &KitInput<'_>,
    destination: &Path,
) -> Result<KitExport, BackupError> {
    let prepared = prepare_export(ctx, installation_key, input, destination)?;
    write_prepared(&prepared)?;
    let recorded = record_prepared(ctx, &prepared);
    if recorded.is_err() {
        discard_prepared(&prepared);
    }
    recorded
}

/// Confirms that the latest kit, number `kit_no`, is stored off this computer
/// (`ops.confirm_recovery_kit`; NFR-30 quarterly confirmation). `note` says where.
pub fn confirm_storage(
    ctx: &mut CommandContext<'_>,
    kit_no: i64,
    note: &str,
) -> Result<(), BackupError> {
    ctx.require(Capability::BackupRestore)?;
    let note = required_text("note", note, NOTE_MAX_CHARS).map_err(BackupError::Invalid)?;
    let latest: Option<(KitId, i64)> = ctx
        .tx()
        .prepare_cached("SELECT id, kit_no FROM ops_recovery_kits ORDER BY kit_no DESC LIMIT 1")?
        .query_row([], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let (kit_id, latest_no) = latest.ok_or(BackupError::NotFound("recovery kit"))?;
    if latest_no != kit_no {
        return Err(BackupError::KitNotLatest);
    }
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_kit_confirmations (id, kit_id, confirmed_by, confirmed_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![
            ConfirmationId::generate(),
            kit_id,
            ctx.principal().actor().id_text(),
            ctx.now(),
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, CONFIRM_RECOVERY_KIT, Outcome::Succeeded)
            .record("recovery_kit", kit_id)
            .detail("kit_no", AuditDetail::integer(kit_no))
            .reason(note),
    )?;
    Ok(())
}

fn next_kit_no(conn: &Connection) -> rusqlite::Result<i64> {
    conn.prepare_cached("SELECT coalesce(max(kit_no), 0) + 1 FROM ops_recovery_kits")?
        .query_row([], |row| row.get(0))
}
