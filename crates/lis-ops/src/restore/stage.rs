//! Decrypting a set into a scratch folder and checking it (M4 design brief §3.6): the
//! restore test and the real restore run the same steps, so a test proves exactly what a
//! restore would do. Nothing is stubbed: the database is decrypted, digest- and
//! integrity-checked and opened with this build's migrations; the chains are verified
//! and compared with the manifest and the anchors; every artifact is decrypted and
//! digest-checked.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use lis_core::{Clock, ModuleSchema, UtcTimestamp};
use lis_store::{AppliedMigration, ArtifactDigest, DATABASE_FILE, Database};
use rusqlite::{Connection, OpenFlags};

use super::apply::remove_folder;
use super::checks::{self, ChainCheck};
use super::findings::{FindingCode, Findings};
use crate::InstallationKind;
use crate::anchors::AnchorSummary;
use crate::backup::{BackupError, Manifest, database_path, read_manifest, set_dir, set_names};
use crate::format::{self, BackupKey, Content, FormatError};

/// What to stage and check.
pub struct StageInput<'a> {
    /// The target root (`…/VibeToxLIS`) holding the set.
    pub source_root: &'a Path,
    /// The set to check; `None` for the newest.
    pub set_id: Option<&'a str>,
    /// The key the set must be written under.
    pub key: &'a BackupKey,
    /// The folder, inside the data folder, that receives the decrypted copy; it is
    /// emptied first.
    pub scratch_dir: &'a Path,
    /// This build's schema modules in crate-map order (the engine's `MODULES`).
    pub modules: &'a [ModuleSchema],
    /// Every column holding artifact digests, as `(table, column)`; their owners publish them.
    pub artifact_columns: &'a [(&'a str, &'a str)],
    /// Other folders whose anchor files are compared too: other targets' roots, or a
    /// folder holding anchors only.
    pub anchor_roots: &'a [PathBuf],
    /// Keep the decrypted artifacts in `scratch_dir/artifacts/` (a restore), or only check
    /// them in memory (a restore test).
    pub keep_artifacts: bool,
    /// The clock the scratch database opens with.
    pub clock: Arc<dyn Clock>,
    /// Raised to stop between files and chunks.
    pub cancel: &'a AtomicBool,
}

/// What staging found.
#[derive(Debug, Clone)]
pub struct StagedRestore {
    /// The set checked.
    pub set_id: String,
    /// Its manifest, when it decrypted.
    pub manifest: Option<Manifest>,
    /// The decrypted database passed SQLite's integrity check.
    pub integrity_ok: bool,
    /// Per stream, when the database opened.
    pub chains: Vec<ChainCheck>,
    /// The anchors compared, when the database opened.
    pub anchors: Option<AnchorSummary>,
    /// The newest seal time of a lost-interval anchor: events until then are lost.
    pub lost_interval_until: Option<UtcTimestamp>,
    /// Every finding.
    pub findings: Findings,
    /// SHA-256 of the scratch database file after the checks, when it opened; a restore
    /// rechecks it before putting the file in place.
    pub staged_sha256: Option<String>,
    /// The kind the set's installation was created as, when its database opened; a restore
    /// is refused into an installation of the other kind (proposed REQ-OPS-016).
    pub installation_kind: Option<InstallationKind>,
}

impl StagedRestore {
    /// True when the set may be restored: nothing but the lost interval was found.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.manifest.is_some() && self.staged_sha256.is_some() && self.findings.passed()
    }

    /// Refuses a set made by an installation of another kind than `installation`: a demo
    /// backup never becomes a laboratory's data, nor the reverse (proposed REQ-OPS-016). A
    /// set whose database did not open has no kind and fails its checks anyway.
    pub fn ensure_kind(&self, installation: InstallationKind) -> Result<(), BackupError> {
        match self.installation_kind {
            Some(backup) if backup != installation => Err(BackupError::InstallationKindMismatch {
                backup: backup.as_str(),
                installation: installation.as_str(),
            }),
            Some(_) | None => Ok(()),
        }
    }
}

/// Stages one set into `scratch_dir` and checks it (§3.6 steps 1–7). Findings describe
/// the set; `Err` means the check could not run: the set does not exist, the target or
/// the scratch folder failed, or `cancel` was raised.
pub fn stage(input: &StageInput<'_>) -> Result<StagedRestore, BackupError> {
    let set_id = choose_set(input.source_root, input.set_id)?;
    let set_dir = set_dir(input.source_root, &set_id);
    let mut staged = StagedRestore {
        set_id,
        manifest: None,
        integrity_ok: false,
        chains: Vec::new(),
        anchors: None,
        lost_interval_until: None,
        findings: Findings::default(),
        staged_sha256: None,
        installation_kind: None,
    };
    let Some(manifest) = open_manifest(&set_dir, input.key, &mut staged.findings)? else {
        return Ok(staged);
    };
    remove_folder(input.scratch_dir)?;
    fs::create_dir_all(input.scratch_dir)?;
    let referenced = check_database(input, &set_dir, &manifest, &mut staged)?;
    checks::check_artifacts(input, &manifest, &mut staged.findings)?;
    if let Some(referenced) = referenced {
        let listed: BTreeSet<&str> = manifest
            .artifacts
            .iter()
            .map(|entry| entry.sha256.as_str())
            .collect();
        let missing = referenced
            .iter()
            .filter(|digest| !listed.contains(digest.as_str()))
            .count();
        staged
            .findings
            .add(FindingCode::ReferencedArtifactMissing, missing as u64);
    }
    staged.manifest = Some(manifest);
    Ok(staged)
}

/// The requested set, or the newest; a name not listed is not found, so it can never
/// name a path outside the target.
fn choose_set(root: &Path, wanted: Option<&str>) -> Result<String, BackupError> {
    let names = set_names(root)?;
    let chosen = match wanted {
        Some(wanted) => names.into_iter().find(|name| name == wanted),
        None => names.into_iter().next_back(),
    };
    chosen.ok_or(BackupError::NotFound("backup set"))
}

/// Step 1: the manifest, or the finding that it cannot be used.
fn open_manifest(
    set_dir: &Path,
    key: &BackupKey,
    findings: &mut Findings,
) -> Result<Option<Manifest>, BackupError> {
    let code = match read_manifest(set_dir, key) {
        Ok(manifest) if manifest.key_id == key.key_id().to_string() => return Ok(Some(manifest)),
        Ok(_) | Err(BackupError::Format(FormatError::KeyMismatch)) => FindingCode::KeyMismatch,
        Err(BackupError::Format(FormatError::Cancelled)) => {
            return Err(FormatError::Cancelled.into());
        }
        Err(BackupError::Io(error)) if error.kind() != io::ErrorKind::NotFound => {
            return Err(error.into());
        }
        Err(_) => FindingCode::ManifestUnreadable,
    };
    findings.add(code, 1);
    Ok(None)
}

/// Steps 1 (schema), 2–5 and the first half of 7: returns the digests the restored
/// database references, or `None` when it could not be opened.
fn check_database(
    input: &StageInput<'_>,
    set_dir: &Path,
    manifest: &Manifest,
    staged: &mut StagedRestore,
) -> Result<Option<BTreeSet<String>>, BackupError> {
    let ledger: Vec<AppliedMigration> = manifest.schema.iter().map(Into::into).collect();
    if lis_store::check_applied(input.modules, &ledger).is_err() {
        staged.findings.add(FindingCode::SchemaNewerThanApp, 1);
        return Ok(None);
    }
    let file = input.scratch_dir.join(DATABASE_FILE);
    if !decrypt_database(input, set_dir, manifest, &file, &mut staged.findings)? {
        return Ok(None);
    }
    if !integrity_ok(&file) {
        staged.findings.add(FindingCode::IntegrityFailed, 1);
        return Ok(None);
    }
    staged.integrity_ok = true;
    let db = match Database::open(input.scratch_dir, input.modules, Arc::clone(&input.clock)) {
        Ok(db) => db,
        Err(error) => {
            tracing::warn!(error = %error, "a staged backup database did not open");
            staged.findings.add(FindingCode::OpenFailed, 1);
            return Ok(None);
        }
    };
    let referenced = checks::check_opened(&db, input, manifest, staged)?;
    staged.installation_kind = db.read_blocking(crate::installation_kind)?;
    drop(db);
    settle(&file)?;
    let (digest, _) = ArtifactDigest::of_reader(File::open(&file)?)?;
    staged.staged_sha256 = Some(digest.to_string());
    Ok(Some(referenced))
}

/// Step 2: decrypts the database into `file` and compares it with the manifest.
fn decrypt_database(
    input: &StageInput<'_>,
    set_dir: &Path,
    manifest: &Manifest,
    file: &Path,
    findings: &mut Findings,
) -> Result<bool, BackupError> {
    let source = match File::open(database_path(set_dir)) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            findings.add(FindingCode::DatabaseDigestMismatch, 1);
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    let mut writer = BufWriter::new(File::create(file)?);
    let decrypted = format::decrypt(
        input.key,
        Content::Database,
        BufReader::new(source),
        &mut writer,
        input.cancel,
    );
    let code = match decrypted {
        Ok(_) => None,
        Err(FormatError::Cancelled) => return Err(FormatError::Cancelled.into()),
        Err(FormatError::Io(error)) => return Err(error.into()),
        Err(FormatError::KeyMismatch) => Some(FindingCode::KeyMismatch),
        Err(_) => Some(FindingCode::DatabaseDigestMismatch),
    };
    if let Some(code) = code {
        findings.add(code, 1);
        return Ok(false);
    }
    writer.flush()?;
    writer
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()?;
    let (digest, bytes) = ArtifactDigest::of_reader(File::open(file)?)?;
    let matches =
        digest.to_string() == manifest.database.sha256 && bytes == manifest.database.bytes;
    if !matches {
        findings.add(FindingCode::DatabaseDigestMismatch, 1);
    }
    Ok(matches)
}

/// SQLite's `PRAGMA integrity_check` returns the single row `ok`; a file that is not a
/// database fails too.
fn integrity_ok(file: &Path) -> bool {
    let check = || -> rusqlite::Result<Vec<String>> {
        let conn = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut statement = conn.prepare("PRAGMA integrity_check")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect()
    };
    check().is_ok_and(|rows| rows == ["ok"])
}

/// Folds the write-ahead log into the database file, so the file alone is the database.
fn settle(file: &Path) -> Result<(), BackupError> {
    let conn = Connection::open(file)?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    conn.close().map_err(|(_, error)| error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(kind: Option<InstallationKind>) -> StagedRestore {
        StagedRestore {
            set_id: "synthetic-set".to_owned(),
            manifest: None,
            integrity_ok: true,
            chains: Vec::new(),
            anchors: None,
            lost_interval_until: None,
            findings: Findings::default(),
            staged_sha256: None,
            installation_kind: kind,
        }
    }

    // Proposed REQ-OPS-016: a demo backup is never restored into a laboratory, nor the reverse.
    #[test]
    fn a_set_of_the_other_kind_is_refused() {
        use InstallationKind::{Demo, Laboratory};
        for (backup, installation) in [(Demo, Laboratory), (Laboratory, Demo)] {
            let refused = staged(Some(backup)).ensure_kind(installation).unwrap_err();
            assert!(
                matches!(refused, BackupError::InstallationKindMismatch { .. }),
                "{refused}"
            );
            assert!(staged(Some(backup)).ensure_kind(backup).is_ok());
        }
        assert!(staged(None).ensure_kind(Laboratory).is_ok());
    }
}
