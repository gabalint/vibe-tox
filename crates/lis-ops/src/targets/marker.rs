//! The target folder layout and its `vibe-tox-target-1` marker file. Reads are bounded
//! and refuse links; the marker holds identifiers only.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use lis_core::{InstallationId, UtcTimestamp};
use serde::{Deserialize, Serialize};

use super::{Target, TargetId};
use crate::backup::BackupError;
use crate::backup::durable::write_file;
use crate::format::KeyId;

/// The folder the application owns inside a chosen folder.
const ROOT_FOLDER: &str = "VibeToxLIS";
const MARKER_FILE: &str = "target.json";
const MARKER_FORMAT: &str = "vibe-tox-target-1";
/// A marker is a few hundred bytes; anything larger is not one.
const MARKER_MAX_BYTES: u64 = 4096;

/// The `target.json` marker.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Marker {
    format: String,
    pub(super) installation_id: InstallationId,
    pub(super) target_id: TargetId,
    pub(super) key_id: String,
    created_at: UtcTimestamp,
}

/// The folder the application owns inside the target's chosen folder.
#[must_use]
pub fn root(target: &Target) -> PathBuf {
    root_in(&target.path)
}

/// The folder the application owns inside a chosen folder.
pub(crate) fn root_in(folder: &Path) -> PathBuf {
    folder.join(ROOT_FOLDER)
}

/// The target root a chosen folder names: its `VibeToxLIS` subfolder when it has one,
/// otherwise the folder itself (someone chose the root directly).
pub(crate) fn resolve_root(folder: &Path) -> PathBuf {
    let nested = root_in(folder);
    if nested.is_dir() {
        nested
    } else {
        folder.to_path_buf()
    }
}

/// The installation and key ID the marker in `root` names; `None` without a marker.
pub(crate) fn marker_owner(root: &Path) -> Result<Option<(InstallationId, String)>, BackupError> {
    Ok(read_marker(root)?.map(|marker| (marker.installation_id, marker.key_id)))
}

/// True when the target's folder is reachable and its marker names the target. Reads
/// one small file; call it outside a write transaction.
#[must_use]
pub fn is_available(target: &Target) -> bool {
    matches!(read_marker(&root(target)), Ok(Some(marker)) if marker.target_id == target.id)
}

pub(super) fn write_marker(
    root: &Path,
    installation: InstallationId,
    target: &Target,
    key_id: KeyId,
    now: UtcTimestamp,
) -> Result<(), BackupError> {
    let marker = Marker {
        format: MARKER_FORMAT.to_owned(),
        installation_id: installation,
        target_id: target.id,
        key_id: key_id.to_string(),
        created_at: now,
    };
    let mut bytes = serde_json::to_vec(&marker).map_err(|_| BackupError::TargetNotWritable)?;
    bytes.push(b'\n');
    ensure_target_root(root)
        .and_then(|()| write_file(&root.join(MARKER_FILE), &bytes))
        .map_err(|error| {
            tracing::warn!(target_id = %target.id, error_kind = ?error.kind(), "backup target marker not written");
            BackupError::TargetNotWritable
        })
}

fn ensure_target_root(root: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => return Err(std::io::Error::other("target root is not a directory")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(root)?,
        Err(error) => return Err(error),
    }
    if !fs::symlink_metadata(root)?.file_type().is_dir() {
        return Err(std::io::Error::other("target root is not a directory"));
    }
    Ok(())
}

/// The marker in `root`, `None` when there is none; an error when it is unreadable or is
/// not a `vibe-tox-target-1` marker.
pub(super) fn read_marker(root: &Path) -> Result<Option<Marker>, BackupError> {
    let marker_path = root.join(MARKER_FILE);
    let metadata = match fs::symlink_metadata(&marker_path) {
        Ok(metadata) if metadata.file_type().is_file() => metadata,
        Ok(_) => return Err(BackupError::TargetFolderUnusable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let file = File::open(&marker_path)?;
    if !file.metadata()?.is_file()
        || !fs::symlink_metadata(&marker_path)?.file_type().is_file()
        || metadata.len() > MARKER_MAX_BYTES
    {
        return Err(BackupError::TargetFolderUnusable);
    }
    let mut bytes = Vec::new();
    file.take(MARKER_MAX_BYTES).read_to_end(&mut bytes)?;
    let marker: Marker =
        serde_json::from_slice(&bytes).map_err(|_| BackupError::TargetFolderUnusable)?;
    if marker.format != MARKER_FORMAT {
        return Err(BackupError::TargetFolderUnusable);
    }
    Ok(Some(marker))
}
