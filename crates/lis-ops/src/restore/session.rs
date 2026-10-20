//! The restore wizard's in-memory session and the sets a chosen folder offers (M4 design
//! brief §3.7 steps 1–2). The installation key typed from the kit lives only in this
//! session, for [`RESTORE_SESSION_MINUTES`] minutes, and is wiped when it is dropped.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lis_auth::DataKey;
use lis_core::{Id, InstallationId, ModuleSchema, UtcTimestamp};
use lis_store::AppliedMigration;

use super::findings::FindingCode;
use crate::backup::{BackupError, Manifest, RunKind, SetListing, list_sets};
use crate::format::{BackupKey, KeyId};
use crate::kit::parse_key;
use crate::policy::RESTORE_SESSION_MINUTES;
use crate::targets;

/// Marker type of restore session IDs.
pub enum RestoreWizard {}
/// Identifies a restore session.
pub type RestoreSessionId = Id<RestoreWizard>;

/// A restore in progress on the setup screen: the entered installation key and when the
/// session ends.
pub struct RestoreSession {
    id: RestoreSessionId,
    key: DataKey,
    backup_key: BackupKey,
    expires_at: UtcTimestamp,
}

impl RestoreSession {
    /// Starts a session with a key line typed or pasted from a recovery kit
    /// (`begin_restore`): a wrong structure or checksum is refused.
    pub fn begin(key_line: &str, now: UtcTimestamp) -> Result<Self, BackupError> {
        let key = parse_key(key_line)?;
        let lifetime = Duration::from_secs(u64::from(RESTORE_SESSION_MINUTES) * 60);
        Ok(Self {
            id: RestoreSessionId::generate(),
            backup_key: BackupKey::derive(&key),
            key,
            expires_at: now.checked_add(lifetime).unwrap_or(now),
        })
    }

    /// The session's ID.
    #[must_use]
    pub fn id(&self) -> RestoreSessionId {
        self.id
    }

    /// The entered key's ID, which the kit prints as its fingerprint.
    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.backup_key.key_id()
    }

    /// The backup key, while the session lasts.
    pub fn backup_key(&self, now: UtcTimestamp) -> Result<&BackupKey, BackupError> {
        self.check(now)?;
        Ok(&self.backup_key)
    }

    /// The installation key, while the session lasts; the engine writes it to the OS key
    /// store when the restore is confirmed.
    pub fn installation_key(&self, now: UtcTimestamp) -> Result<&DataKey, BackupError> {
        self.check(now)?;
        Ok(&self.key)
    }

    fn check(&self, now: UtcTimestamp) -> Result<(), BackupError> {
        if now < self.expires_at {
            Ok(())
        } else {
            Err(BackupError::RestoreSessionExpired)
        }
    }
}

impl fmt::Debug for RestoreSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RestoreSession({}, {}, [redacted])",
            self.id,
            self.key_id()
        )
    }
}

/// One backup set, as the restore wizard and the backups screen list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSummary {
    /// The set's name.
    pub set_id: String,
    /// When it was taken.
    pub created_at: UtcTimestamp,
    /// Why its run started.
    pub kind: RunKind,
    /// The app version that wrote it.
    pub app_version: String,
    /// Length of its database snapshot.
    pub database_bytes: u64,
    /// Artifacts it holds.
    pub artifact_count: usize,
    /// `None` when this build can restore it; otherwise why not.
    pub incompatible: Option<FindingCode>,
}

/// The sets of `listings` whose manifest decrypts, and how many did not.
#[must_use]
pub fn summarize_sets(
    listings: Vec<SetListing>,
    modules: &[ModuleSchema],
) -> (Vec<SetSummary>, usize) {
    let mut unreadable = 0;
    let mut sets = Vec::new();
    for listing in listings {
        match listing.manifest {
            Ok(manifest) => sets.push(summary(listing.set_id, &manifest, modules)),
            Err(_) => unreadable += 1,
        }
    }
    (sets, unreadable)
}

fn summary(set_id: String, manifest: &Manifest, modules: &[ModuleSchema]) -> SetSummary {
    let ledger: Vec<AppliedMigration> = manifest.schema.iter().map(Into::into).collect();
    let compatible = lis_store::check_applied(modules, &ledger).is_ok();
    SetSummary {
        set_id,
        created_at: manifest.created_at,
        kind: manifest.kind,
        app_version: manifest.app_version.clone(),
        database_bytes: manifest.database.bytes,
        artifact_count: manifest.artifacts.len(),
        incompatible: (!compatible).then_some(FindingCode::SchemaNewerThanApp),
    }
}

/// A folder chosen as the restore source (`choose_restore_source`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreSource {
    /// The target root the sets are read from.
    pub root: PathBuf,
    /// The installation whose backups it holds.
    pub installation_id: InstallationId,
    /// Its sets whose manifest decrypts, oldest first.
    pub sets: Vec<SetSummary>,
    /// Sets whose manifest did not decrypt.
    pub unreadable_sets: usize,
}

/// Reads the target marker in `folder` (or in its `VibeToxLIS` subfolder), checks that its
/// key ID is the session key's, and lists its sets.
pub fn read_source(
    folder: &Path,
    key: &BackupKey,
    modules: &[ModuleSchema],
) -> Result<RestoreSource, BackupError> {
    let root = targets::resolve_root(folder);
    let owner = targets::marker_owner(&root).map_err(|_| BackupError::NoBackupsInFolder)?;
    let (installation_id, key_id) = owner.ok_or(BackupError::NoBackupsInFolder)?;
    if key_id != key.key_id().to_string() {
        return Err(BackupError::TargetKeyMismatch);
    }
    let (sets, unreadable_sets) = summarize_sets(list_sets(&root, key)?, modules);
    Ok(RestoreSource {
        root,
        installation_id,
        sets,
        unreadable_sets,
    })
}
