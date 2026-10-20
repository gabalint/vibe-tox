//! The `vibe-tox-set-1` manifest of a backup set (M4 design brief §3.3): what the set
//! holds and the chain heads and seals it was taken at. It is stored encrypted as
//! `manifest.vtbk`, so it may name artifact digests and schema versions. This module is
//! its one home; restore reads sets through [`list_sets`] and [`read_manifest`].

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use jiff::Timestamp;
use lis_audit::{ChainLink, Seal, Stream};
use lis_core::{InstallationId, UtcTimestamp};
use lis_store::{AppliedMigration, ArtifactListing};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use super::durable::TEMP_SUFFIX;
use super::{BackupError, RunId, RunKind};
use crate::format::{self, BackupKey, Content};

/// The `format` value of every manifest.
pub const MANIFEST_FORMAT: &str = "vibe-tox-set-1";
/// Folder of a target's sets, inside its root.
pub(super) const SETS_FOLDER: &str = "sets";
/// The encrypted manifest inside a set folder.
pub(super) const MANIFEST_FILE: &str = "manifest.vtbk";
/// The encrypted database snapshot inside a set folder.
pub(super) const DATABASE_FILE: &str = "database.vtbk";
/// The only `integrity` value a manifest records: unintact snapshots are never copied.
const INTEGRITY_OK: &str = "ok";
/// A manifest lists one entry per artifact; this bounds what a reader accepts.
const MANIFEST_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// A backup set's manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    /// Always [`MANIFEST_FORMAT`].
    pub format: String,
    /// The installation the set belongs to; retention prunes only its own sets.
    pub installation_id: InstallationId,
    /// The set folder's name: `<YYYYMMDDTHHMMSSZ>-<last 8 hex digits of the run ID>`.
    pub set_id: String,
    /// The run that wrote it.
    pub run_id: RunId,
    /// Why the run started.
    pub kind: RunKind,
    /// When the run started; the snapshot was taken just after.
    pub created_at: UtcTimestamp,
    /// The app version that wrote it.
    pub app_version: String,
    /// The installation key's ID (32 hex digits).
    pub key_id: String,
    /// The snapshot's migration ledger.
    pub schema: Vec<SchemaEntry>,
    /// The snapshot file.
    pub database: DatabaseEntry,
    /// The snapshot's chain head per stream.
    pub heads: Vec<HeadEntry>,
    /// The snapshot's newest seal per sealed stream.
    pub seals: Vec<SealEntry>,
    /// Every artifact present on the target for this set, by digest.
    pub artifacts: Vec<ArtifactEntry>,
    /// Artifacts that could not be copied, and unexpected entries of the artifact folder.
    pub artifact_problems: Vec<ArtifactProblem>,
}

/// One applied migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaEntry {
    /// Module name.
    pub module: String,
    /// Migration version.
    pub version: u32,
    /// Migration name.
    pub name: String,
    /// The SQL checksum the store recorded.
    pub checksum: String,
}

/// The plaintext snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseEntry {
    /// SHA-256 of the snapshot file.
    pub sha256: String,
    /// Its length.
    pub bytes: u64,
    /// The integrity check result: always `ok`.
    pub integrity: String,
}

/// A chain head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadEntry {
    /// `clinical`, `security` or `access`.
    pub stream: String,
    /// The newest sequence number, 0 for an empty stream.
    pub seq: i64,
    /// Its stored hash.
    pub hash: String,
}

/// A stream's newest seal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SealEntry {
    /// The sealed stream.
    pub stream: String,
    /// The seal number.
    pub seal_no: i64,
    /// The last sequence number it covers.
    pub last_seq: i64,
    /// The stored hash of that event.
    pub head_hash: String,
}

/// An artifact in the set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEntry {
    /// Its digest, also its file name.
    pub sha256: String,
    /// Its plaintext length.
    pub bytes: u64,
}

/// An artifact that is not in the set.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactProblem {
    /// The digest, or `None` for an unexpected entry, whose name is not recorded.
    pub sha256: Option<String>,
    /// `missing`, `damaged`, `unreadable` or `unexpected_entry`.
    pub problem: String,
}

/// What a snapshot holds, read from the snapshot copy while the database moves on.
pub(super) struct SnapshotFacts {
    pub(super) heads: Vec<ChainLink>,
    pub(super) seals: Vec<SealEntry>,
    pub(super) schema: Vec<SchemaEntry>,
}

/// Reads the heads, the newest seal per stream and the migration ledger of a snapshot.
pub(super) fn read_snapshot(path: &Path) -> Result<SnapshotFacts, BackupError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let heads = lis_audit::heads(&conn)?;
    let mut seals = Vec::new();
    for stream in Stream::ALL {
        if let Some(seal) = lis_audit::last_seal(&conn, stream)? {
            seals.push(seal_entry(&seal));
        }
    }
    let schema = lis_store::applied_migrations(&conn)?
        .into_iter()
        .map(SchemaEntry::from)
        .collect();
    Ok(SnapshotFacts {
        heads,
        seals,
        schema,
    })
}

impl From<AppliedMigration> for SchemaEntry {
    fn from(applied: AppliedMigration) -> Self {
        Self {
            module: applied.module,
            version: applied.version,
            name: applied.name,
            checksum: applied.checksum,
        }
    }
}

impl From<&SchemaEntry> for AppliedMigration {
    fn from(entry: &SchemaEntry) -> Self {
        Self {
            module: entry.module.clone(),
            version: entry.version,
            name: entry.name.clone(),
            checksum: entry.checksum.clone(),
        }
    }
}

fn seal_entry(seal: &Seal) -> SealEntry {
    SealEntry {
        stream: seal.stream.as_str().to_owned(),
        seal_no: seal.seal_no,
        last_seq: seal.last_seq,
        head_hash: seal.head_hash.clone(),
    }
}

/// The manifest's entry for a head.
pub(super) fn head_entry(head: &ChainLink) -> HeadEntry {
    HeadEntry {
        stream: head.stream.as_str().to_owned(),
        seq: head.seq,
        hash: head.hash.clone(),
    }
}

impl DatabaseEntry {
    /// The entry of an intact snapshot.
    pub(super) fn intact(sha256: String, bytes: u64) -> Self {
        Self {
            sha256,
            bytes,
            integrity: INTEGRITY_OK.to_owned(),
        }
    }
}

/// One set folder of a target.
#[derive(Debug)]
pub struct SetListing {
    /// The folder name.
    pub set_id: String,
    /// Its decrypted manifest, or why it could not be read.
    pub manifest: Result<Manifest, BackupError>,
}

/// Every complete set of the target whose root is `root`, sorted by name (oldest first).
/// Sets still being written (`.tmp`) are never listed. A target without sets lists none.
pub fn list_sets(root: &Path, key: &BackupKey) -> Result<Vec<SetListing>, BackupError> {
    Ok(set_names(root)?
        .into_iter()
        .map(|set_id| {
            let manifest = read_manifest(&set_dir(root, &set_id), key);
            SetListing { set_id, manifest }
        })
        .collect())
}

/// The names of the complete sets in `root`, oldest first, without reading them.
pub(crate) fn set_names(root: &Path) -> Result<Vec<String>, BackupError> {
    let entries = match fs::read_dir(root.join(SETS_FOLDER)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(set_id) = entry.file_name().into_string() else {
            continue;
        };
        if !set_id.ends_with(TEMP_SUFFIX) && entry.file_type()?.is_dir() {
            names.push(set_id);
        }
    }
    names.sort();
    Ok(names)
}

/// The folder of the set `set_id` in `root`; `set_id` must be one of [`set_names`].
pub(crate) fn set_dir(root: &Path, set_id: &str) -> PathBuf {
    root.join(SETS_FOLDER).join(set_id)
}

/// The encrypted database snapshot of the set folder `set_dir`.
pub(crate) fn database_path(set_dir: &Path) -> PathBuf {
    set_dir.join(DATABASE_FILE)
}

/// Decrypts and parses the manifest of the set folder `set_dir`.
pub fn read_manifest(set_dir: &Path, key: &BackupKey) -> Result<Manifest, BackupError> {
    let folder_id = set_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| canonical_set_id(name))
        .ok_or(crate::format::FormatError::Corrupt)?;
    let file = File::open(set_dir.join(MANIFEST_FILE))?;
    let mut bytes = Vec::new();
    let not_cancellable = AtomicBool::new(false);
    format::decrypt(
        key,
        Content::Manifest,
        BufReader::new(std::io::Read::take(file, MANIFEST_MAX_BYTES)),
        &mut bytes,
        &not_cancellable,
    )?;
    let manifest = parse_manifest(&bytes)?;
    if manifest.set_id != folder_id {
        return Err(crate::format::FormatError::Corrupt.into());
    }
    Ok(manifest)
}

/// Whether `value` is the canonical set ID generated by [`set_id`], including a real
/// UTC calendar timestamp and an eight-character lowercase hexadecimal run suffix.
#[must_use]
pub(crate) fn canonical_set_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 25
        || !bytes[..8].iter().all(u8::is_ascii_digit)
        || bytes[8] != b'T'
        || !bytes[9..15].iter().all(u8::is_ascii_digit)
        || &bytes[15..17] != b"Z-"
        || !bytes[17..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return false;
    }
    let timestamp = format!(
        "{}-{}-{}T{}:{}:{}Z",
        &value[0..4],
        &value[4..6],
        &value[6..8],
        &value[9..11],
        &value[11..13],
        &value[13..15]
    );
    timestamp.parse::<Timestamp>().is_ok()
}

/// Parses manifest JSON, refusing another format.
pub(super) fn parse_manifest(bytes: &[u8]) -> Result<Manifest, BackupError> {
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| crate::format::FormatError::Corrupt)?;
    if manifest.format != MANIFEST_FORMAT {
        return Err(crate::format::FormatError::Corrupt.into());
    }
    Ok(manifest)
}

/// `<YYYYMMDDTHHMMSSZ>-<last 8 hex digits of the run ID>`: sorts by time, and the random
/// tail keeps two runs in the same second apart.
pub(super) fn set_id(started_at: UtcTimestamp, run_id: RunId) -> String {
    let time: String = started_at
        .to_string()
        .chars()
        .take(19)
        .filter(char::is_ascii_digit)
        .collect();
    let run = run_id.to_string();
    let (date, clock) = time.split_at(time.len().min(8));
    format!("{date}T{clock}Z-{}", &run[run.len().saturating_sub(8)..])
}

/// Unexpected entries of the artifact folder are counted, never named: their names were
/// not chosen by the application.
pub(super) fn unexpected_entries(listing: &ArtifactListing) -> Vec<ArtifactProblem> {
    listing
        .unexpected
        .iter()
        .map(|_| ArtifactProblem {
            sha256: None,
            problem: "unexpected_entry".to_owned(),
        })
        .collect()
}
