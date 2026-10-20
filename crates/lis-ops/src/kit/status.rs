//! Recovery-kit status: the latest export, its latest storage confirmation and why it
//! no longer restores this installation as it is now (NFR-30).

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use super::KitId;
use crate::backup::BackupError;
use crate::format::KeyId;
use crate::targets::{self, Target};

lis_core::text_enum! {
    /// Why the latest kit no longer restores this installation as it is now.
    OutdatedReason {
        /// No kit was exported yet.
        NeverExported = "never_exported",
        /// The installation key changed since the latest export.
        KeyChanged = "key_changed",
        /// Backup locations were added or retired since the latest export.
        TargetsChanged = "targets_changed",
    }
}

/// The latest kit export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitRecord {
    /// The kit number.
    pub kit_no: i64,
    /// The file name written.
    pub file_name: String,
    /// Who exported it (user ID).
    pub exported_by: String,
    /// When.
    pub exported_at: UtcTimestamp,
}

/// The latest storage confirmation of the latest kit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfirmation {
    /// Who confirmed (user ID).
    pub confirmed_by: String,
    /// When.
    pub confirmed_at: UtcTimestamp,
}

/// The kit status the health check and the kit screen show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KitStatus {
    /// The latest export.
    pub latest: Option<KitRecord>,
    /// The latest confirmation of that export.
    pub last_confirmation: Option<StorageConfirmation>,
    /// Empty when the latest kit is current.
    pub outdated_reasons: Vec<OutdatedReason>,
}

/// The latest kit, its latest confirmation and why it is outdated, compared with the
/// current key ID and active targets.
pub fn status(conn: &Connection, key_id: KeyId) -> Result<KitStatus, BackupError> {
    status_for_key(conn, Some(key_id))
}

/// Kit status while the installation key is missing, mismatched or unavailable. An
/// existing kit is necessarily not current until the key is restored.
pub fn status_without_key(conn: &Connection) -> Result<KitStatus, BackupError> {
    status_for_key(conn, None)
}

/// The fingerprint printed by the newest recorded kit, for display while the local key
/// cannot be loaded. This never exposes key material.
pub fn latest_key_fingerprint(conn: &Connection) -> Result<Option<String>, BackupError> {
    Ok(conn
        .prepare_cached("SELECT key_id FROM ops_recovery_kits ORDER BY kit_no DESC LIMIT 1")?
        .query_row([], |row| row.get(0))
        .optional()?)
}

fn status_for_key(conn: &Connection, key_id: Option<KeyId>) -> Result<KitStatus, BackupError> {
    let latest: Option<(KitId, KitRecord, String, String)> = conn
        .prepare_cached(
            "SELECT id, kit_no, file_name, exported_by, exported_at, key_id, targets_digest \
             FROM ops_recovery_kits ORDER BY kit_no DESC LIMIT 1",
        )?
        .query_row([], |row| {
            let record = KitRecord {
                kit_no: row.get(1)?,
                file_name: row.get(2)?,
                exported_by: row.get(3)?,
                exported_at: row.get(4)?,
            };
            Ok((row.get(0)?, record, row.get(5)?, row.get(6)?))
        })
        .optional()?;
    let Some((kit_id, record, kit_key_id, kit_targets)) = latest else {
        return Ok(KitStatus {
            latest: None,
            last_confirmation: None,
            outdated_reasons: vec![OutdatedReason::NeverExported],
        });
    };
    let mut outdated_reasons = Vec::new();
    if key_id.is_none_or(|key_id| kit_key_id != key_id.to_string()) {
        outdated_reasons.push(OutdatedReason::KeyChanged);
    }
    if kit_targets != targets_digest(&targets::active(conn)?) {
        outdated_reasons.push(OutdatedReason::TargetsChanged);
    }
    let last_confirmation = conn
        .prepare_cached(
            "SELECT confirmed_by, confirmed_at FROM ops_kit_confirmations WHERE kit_id = ?1 \
             ORDER BY confirmed_at DESC LIMIT 1",
        )?
        .query_row([kit_id], |row| {
            Ok(StorageConfirmation {
                confirmed_by: row.get(0)?,
                confirmed_at: row.get(1)?,
            })
        })
        .optional()?;
    Ok(KitStatus {
        latest: Some(record),
        last_confirmation,
        outdated_reasons,
    })
}

/// SHA-256 over the active targets' IDs and paths, in ID order.
pub(super) fn targets_digest(locations: &[Target]) -> String {
    let mut lines: Vec<String> = locations
        .iter()
        .map(|target| format!("{}\t{}\n", target.id, target.path.display()))
        .collect();
    lines.sort();
    hex::encode(Sha256::digest(lines.concat()))
}
