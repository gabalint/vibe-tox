//! Backup targets: folders the laboratory chose for encrypted copies and audit anchors
//! (REQ-OPS-007, NFR-20; M4 design brief D3, §3.3). This module is the one home of the
//! target layout and the `vibe-tox-target-1` marker.
//!
//! ```text
//! <chosen folder>/VibeToxLIS/
//!   target.json   {"format":"vibe-tox-target-1","installationId","targetId","keyId","createdAt"}
//!   sets/…        backup sets (`backup`)
//!   artifacts/…   encrypted artifacts (`backup`)
//!   anchors/…     audit anchors (`anchors`)
//! ```
//!
//! Invariants:
//! - a target is *active* until retired, once; it is *available* when its folder holds a
//!   marker naming it, which also covers a removable drive that is plugged in again;
//! - a target is never inside the data folder, and a folder holding another
//!   installation's or another key's marker is refused;
//! - the marker is written before the row, and holds identifiers only: no label, path or
//!   patient data;
//! - audit details hold IDs and codes, never paths; logs carry target IDs only.

mod marker;

use std::fs;
use std::path::{Path, PathBuf};

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, InstallationId, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::backup::BackupError;
use crate::format::KeyId;
use crate::installation_id;
use marker::{Marker, read_marker, root_in, write_marker};
pub use marker::{is_available, root};
pub(crate) use marker::{marker_owner, resolve_root};

/// Audit action of adding a target.
pub const ADD_TARGET: &str = "ops.add_target";
/// Audit action of retiring a target.
pub const RETIRE_TARGET: &str = "ops.retire_target";

const LABEL_MAX_CHARS: usize = 80;
const REASON_MAX_CHARS: usize = 500;

/// Marker type of target IDs.
pub enum BackupTarget {}
/// Identifies a backup target; its marker carries the same ID.
pub type TargetId = Id<BackupTarget>;

lis_core::text_enum! {
    /// Where the folder lives; decides whether its copies count as off-host (NFR-20).
    TargetKind {
        /// A second internal disk of this computer.
        LocalDisk = "local_disk",
        /// A drive that is unplugged and rotated off the computer.
        Removable = "removable",
        /// A folder on a network share.
        NetworkShare = "network_share",
    }
}

impl TargetKind {
    /// Only removable and network-share targets hold copies outside this computer
    /// (M4 design brief D6).
    #[must_use]
    pub fn is_off_host(self) -> bool {
        match self {
            Self::LocalDisk => false,
            Self::Removable | Self::NetworkShare => true,
        }
    }
}

lis_core::text_enum! {
    /// What the target receives. Every target receives anchors.
    TargetPurpose {
        /// Backup sets and audit anchors.
        BackupAndAnchors = "backup_and_anchors",
        /// Audit anchors only, such as a drive kept by someone other than the host
        /// administrator (ADR 0008).
        AnchorsOnly = "anchors_only",
    }
}

impl TargetPurpose {
    /// True when backup sets go to the target.
    #[must_use]
    pub fn takes_backups(self) -> bool {
        match self {
            Self::BackupAndAnchors => true,
            Self::AnchorsOnly => false,
        }
    }
}

/// An active backup target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Its ID, also in its marker.
    pub id: TargetId,
    /// The laboratory's PHI-free name for it.
    pub label: String,
    /// Where it lives.
    pub kind: TargetKind,
    /// What it receives.
    pub purpose: TargetPurpose,
    /// The chosen folder, as the OS resolved it when added.
    pub path: PathBuf,
    /// Its revision, for retiring.
    pub revision: i64,
}

/// Input of [`add_target`]. The engine fills `folder` from an OS dialog, never from the UI.
#[derive(Debug, Clone)]
pub struct NewTarget {
    /// The laboratory's PHI-free name for the target.
    pub label: String,
    /// Where the folder lives.
    pub kind: TargetKind,
    /// What it receives.
    pub purpose: TargetPurpose,
    /// The chosen folder.
    pub folder: PathBuf,
}

const TARGET_COLUMNS: &str = "id, label, kind, purpose, path, revision";

fn target_from_row(row: &Row<'_>) -> rusqlite::Result<Target> {
    Ok(Target {
        id: row.get(0)?,
        label: row.get(1)?,
        kind: row.get(2)?,
        purpose: row.get(3)?,
        path: PathBuf::from(row.get::<_, String>(4)?),
        revision: row.get(5)?,
    })
}

/// Every active target, oldest first.
pub fn active(conn: &Connection) -> Result<Vec<Target>, BackupError> {
    let sql = format!(
        "SELECT {TARGET_COLUMNS} FROM ops_targets WHERE retired_at IS NULL ORDER BY created_at, id"
    );
    let targets = conn
        .prepare_cached(&sql)?
        .query_map([], target_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(targets)
}

/// Adds a target (REQ-OPS-007): checks the folder, writes the marker, then the row, and
/// audits `ops.add_target`. Refuses a folder inside `data_dir`, an unwritable folder, a
/// folder that is already an active target, and a marker of another installation or key.
pub fn add_target(
    ctx: &mut CommandContext<'_>,
    data_dir: &Path,
    key_id: KeyId,
    input: &NewTarget,
) -> Result<Target, BackupError> {
    ctx.require(Capability::BackupRestore)?;
    let label =
        required_text("label", &input.label, LABEL_MAX_CHARS).map_err(BackupError::Invalid)?;
    let installation = installation_id(ctx.tx())?.ok_or(BackupError::NoInstallation)?;
    let folder = usable_folder(data_dir, &input.folder)?;
    let path_text = folder.to_str().ok_or(BackupError::TargetFolderUnusable)?;
    if active_with_path(ctx.tx(), path_text)? {
        return Err(BackupError::TargetAlreadyAdded);
    }
    let root = root_in(&folder);
    let existing = read_marker(&root).map_err(|_| BackupError::TargetFolderUnusable)?;
    if let Some(marker) = existing {
        check_existing_marker(ctx.tx(), &marker, installation, key_id)?;
    }
    let target = Target {
        id: TargetId::generate(),
        label,
        kind: input.kind,
        purpose: input.purpose,
        path: folder.clone(),
        revision: 1,
    };
    write_marker(&root, installation, &target, key_id, ctx.now())?;
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_targets (id, label, kind, purpose, path, created_by, created_at, \
             revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
        )?
        .execute(params![
            target.id,
            target.label,
            target.kind,
            target.purpose,
            path_text,
            ctx.principal().actor().id_text(),
            ctx.now(),
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, ADD_TARGET, Outcome::Succeeded)
            .record("backup_target", target.id)
            .new_revision(1)
            .detail("kind", AuditDetail::code(target.kind.as_str()))
            .detail("purpose", AuditDetail::code(target.purpose.as_str())),
    )?;
    Ok(target)
}

/// Retires an active target (`ops.retire_target`). Its copies and marker stay where they
/// are; nothing is written to it afterwards.
pub fn retire_target(
    ctx: &mut CommandContext<'_>,
    target_id: TargetId,
    expected_revision: i64,
    reason: &str,
) -> Result<(), BackupError> {
    ctx.require(Capability::BackupRestore)?;
    let reason = required_text("reason", reason, REASON_MAX_CHARS).map_err(BackupError::Invalid)?;
    let current: Option<(i64, bool)> = ctx
        .tx()
        .prepare_cached("SELECT revision, retired_at IS NOT NULL FROM ops_targets WHERE id = ?1")?
        .query_row([target_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let (revision, retired) = current.ok_or(BackupError::NotFound("backup target"))?;
    if retired {
        return Err(BackupError::TargetRetired);
    }
    if revision != expected_revision {
        return Err(BackupError::RevisionConflict("backup target"));
    }
    ctx.tx()
        .prepare_cached(
            "UPDATE ops_targets SET retired_by = ?2, retired_at = ?3, retire_reason = ?4, \
             revision = revision + 1 WHERE id = ?1",
        )?
        .execute(params![
            target_id,
            ctx.principal().actor().id_text(),
            ctx.now(),
            reason,
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, RETIRE_TARGET, Outcome::Succeeded)
            .record("backup_target", target_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(reason),
    )?;
    Ok(())
}

/// The chosen folder resolved by the OS, refused when missing or inside the data folder.
fn usable_folder(data_dir: &Path, folder: &Path) -> Result<PathBuf, BackupError> {
    let folder = fs::canonicalize(folder).map_err(|_| BackupError::TargetFolderUnusable)?;
    if !folder.is_dir() {
        return Err(BackupError::TargetFolderUnusable);
    }
    let data_dir = fs::canonicalize(data_dir)?;
    if folder.starts_with(&data_dir) {
        return Err(BackupError::TargetInsideDataFolder);
    }
    Ok(folder)
}

fn active_with_path(conn: &Connection, path: &str) -> Result<bool, BackupError> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM ops_targets WHERE path = ?1 AND retired_at IS NULL")?
        .exists([path])?)
}

/// A marker of this installation and key may be taken over, unless an active target
/// still owns it (for example the same drive under another path).
fn check_existing_marker(
    conn: &Connection,
    marker: &Marker,
    installation: InstallationId,
    key_id: KeyId,
) -> Result<(), BackupError> {
    if marker.installation_id != installation {
        return Err(BackupError::TargetOfAnotherInstallation);
    }
    if marker.key_id != key_id.to_string() {
        return Err(BackupError::TargetKeyMismatch);
    }
    let owned = conn
        .prepare_cached("SELECT 1 FROM ops_targets WHERE id = ?1 AND retired_at IS NULL")?
        .exists([marker.target_id])?;
    if owned {
        return Err(BackupError::TargetAlreadyAdded);
    }
    Ok(())
}
