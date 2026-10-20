//! Startup reconciliation of published backup sets (REQ-OPS-007). A power loss after
//! the final directory rename but before the copy row commits can otherwise make an
//! unrecorded set look usable. Authenticated sets without an exact successful copy row
//! are moved aside before listing or retention runs.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::{BackupError, list_sets, repo};
use crate::backup::durable;
use crate::format::BackupKey;
use crate::{installation_id, targets};

const QUARANTINE_FOLDER: &str = "quarantine";

/// The result of reconciling every available active backup target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PublishedSetReconciliation {
    /// Authenticated final sets moved aside because no successful copy row records them.
    pub quarantined: usize,
    /// Final set folders whose name or authenticated manifest was invalid. They remain
    /// visible as unreadable findings, but are never listed as usable or pruned.
    pub unreadable: usize,
}

/// Moves every authenticated published set without a matching unpruned successful copy
/// row to `<target>/quarantine/`. Call this at startup before listing sets or retention.
pub fn reconcile_published_sets(
    conn: &Connection,
    key: &BackupKey,
) -> Result<PublishedSetReconciliation, BackupError> {
    let installation = installation_id(conn)?.ok_or(BackupError::NoInstallation)?;
    let mut result = PublishedSetReconciliation::default();
    for target in targets::active(conn)? {
        if !target.purpose.takes_backups() || !targets::is_available(&target) {
            continue;
        }
        let root = targets::root(&target);
        for listing in list_sets(&root, key)? {
            let manifest = match listing.manifest {
                Ok(manifest) => manifest,
                Err(_) => {
                    result.unreadable += 1;
                    continue;
                }
            };
            let recorded = manifest.installation_id == installation
                && repo::successful_copy_exists(
                    conn,
                    target.id,
                    manifest.run_id,
                    &manifest.set_id,
                )?;
            if recorded {
                continue;
            }
            quarantine(&root, &listing.set_id, &manifest.run_id.to_string())?;
            result.quarantined += 1;
        }
    }
    Ok(result)
}

fn quarantine(root: &Path, set_id: &str, run_id: &str) -> Result<(), BackupError> {
    let folder = root.join(QUARANTINE_FOLDER);
    fs::create_dir_all(&folder)?;
    let base = format!("unrecorded-{set_id}-{run_id}");
    let destination = unused_destination(&folder, &base);
    durable::rename(&root.join("sets").join(set_id), &destination)?;
    Ok(())
}

fn unused_destination(folder: &Path, base: &str) -> PathBuf {
    let initial = folder.join(base);
    if !initial.exists() {
        return initial;
    }
    for suffix in 2_u32.. {
        let candidate = folder.join(format!("{base}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("the finite filesystem cannot contain every numeric suffix")
}
