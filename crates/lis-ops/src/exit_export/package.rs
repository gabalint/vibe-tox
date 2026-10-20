//! Writing one exit package (M4 design brief §5 steps 1–5):
//!
//! ```text
//! vibe-tox-exit-<installationId>-<YYYYMMDDTHHMMSSZ>/
//!   README.txt  manifest.json  database/lis.sqlite3  artifacts/<sha256>
//!   audit/anchors.jsonl  schema/schema.sql  schema/data-dictionary.json|md
//!   schema/migrations/<module>/<NNNN>_<name>.sql  tools/verify_export.py
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use lis_core::{InstallationId, UtcTimestamp};
use lis_store::{ArtifactDigest, CommandContext};
use serde::{Deserialize, Serialize};

use super::contents::{VERIFIER, migration_sources, read_facts, readme};
use super::dictionary::ArtifactReference;
use super::{ExitExportId, ExitExportJob, ExportFailure, repo};
use crate::backup::durable::write_file;
use crate::backup::{ArtifactProblem, BackupError, HeadEntry, SchemaEntry};
use crate::installation_id;

/// The `format` value of every exit manifest.
pub const EXIT_FORMAT: &str = "vibe-tox-exit-1";
const PACKAGE_PREFIX: &str = "vibe-tox-exit-";

/// The package's `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExitManifest {
    /// Always [`EXIT_FORMAT`].
    pub format: String,
    /// The installation exported.
    pub installation_id: InstallationId,
    /// The export.
    pub export_id: ExitExportId,
    /// When the export started; the streams were sealed then.
    pub created_at: UtcTimestamp,
    /// The app version that wrote it.
    pub app_version: String,
    /// The database's migration ledger.
    pub schema: Vec<SchemaEntry>,
    /// Every other file of the package, with its SHA-256 and length.
    pub files: Vec<ManifestFile>,
    /// The database's audit chain heads.
    pub heads: Vec<HeadEntry>,
    /// Lines in `audit/anchors.jsonl`: every seal.
    pub seal_count: usize,
    /// The columns holding artifact digests.
    pub artifact_references: Vec<ArtifactReference>,
    /// Artifacts the package lacks, and unexpected entries of the artifact folder.
    pub artifact_problems: Vec<ArtifactProblem>,
}

/// One file of the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFile {
    /// Path inside the package, with `/` separators.
    pub path: String,
    /// SHA-256 of its bytes.
    pub sha256: String,
    /// Its length.
    pub bytes: u64,
}

/// A requested export moved to `running`.
pub(super) struct Started {
    pub(super) export_id: ExitExportId,
    pub(super) installation: InstallationId,
    pub(super) created_at: UtcTimestamp,
}

/// Starts the export and seals the audit streams (§5 step 1).
pub(super) fn start(
    ctx: &mut CommandContext<'_>,
    export_id: ExitExportId,
) -> Result<Started, BackupError> {
    let installation = installation_id(ctx.tx())?.ok_or(BackupError::NoInstallation)?;
    repo::start(ctx.tx(), export_id)?.ok_or(BackupError::NotFound("exit export"))?;
    lis_audit::seal(ctx.tx(), ctx.now())?;
    Ok(Started {
        export_id,
        installation,
        created_at: ctx.now(),
    })
}

/// What writing produced.
pub(super) enum Produced {
    Failed(ExportFailure),
    Interrupted,
    Written(Box<Written>),
}

/// Why writing stopped.
enum Halt {
    Failed(ExportFailure),
    Cancelled,
}

/// A package written and read back.
pub(super) struct Written {
    pub(super) package_name: String,
    pub(super) manifest_sha256: String,
    pub(super) verifier_sha256: String,
    pub(super) database_bytes: u64,
    pub(super) artifact_count: usize,
    pub(super) artifact_problems: usize,
}

impl From<io::Error> for Halt {
    fn from(_: io::Error) -> Self {
        Self::Failed(ExportFailure::WriteFailed)
    }
}

impl From<ExportFailure> for Halt {
    fn from(failure: ExportFailure) -> Self {
        Self::Failed(failure)
    }
}

/// The package being written.
pub(super) struct Package<'a> {
    job: &'a ExitExportJob<'a>,
    started: &'a Started,
    dir: PathBuf,
    files: Vec<ManifestFile>,
}

impl<'a> Package<'a> {
    pub(super) fn new(job: &'a ExitExportJob<'a>, started: &'a Started) -> Self {
        let stamp: String = started
            .created_at
            .to_string()
            .chars()
            .take(19)
            .filter(char::is_ascii_digit)
            .collect();
        let (date, clock) = stamp.split_at(stamp.len().min(8));
        let name = format!("{PACKAGE_PREFIX}{}-{date}T{clock}Z", started.installation);
        Self {
            job,
            started,
            dir: job.folder.join(name),
            files: Vec::new(),
        }
    }

    /// Writes the package; on failure removes the folder this export created.
    pub(super) fn write(mut self) -> Produced {
        if fs::create_dir(&self.dir).is_err() {
            return Produced::Failed(ExportFailure::FolderUnusable);
        }
        match self.write_contents() {
            Ok(written) => Produced::Written(Box::new(written)),
            Err(halt) => {
                if let Err(error) = fs::remove_dir_all(&self.dir) {
                    tracing::warn!(export_id = %self.started.export_id, error_kind = ?error.kind(), "failed exit package not removed");
                }
                match halt {
                    Halt::Failed(failure) => Produced::Failed(failure),
                    Halt::Cancelled => Produced::Interrupted,
                }
            }
        }
    }

    fn write_contents(&mut self) -> Result<Written, Halt> {
        let database = self.dir.join("database").join(lis_store::DATABASE_FILE);
        fs::create_dir_all(database.parent().unwrap_or(&self.dir))?;
        let snapshot = self.job.db.snapshot(&database).map_err(|error| {
            tracing::error!(export_id = %self.started.export_id, error = %error, "exit snapshot failed");
            ExportFailure::SnapshotFailed
        })?;
        if !snapshot.is_intact() {
            return Err(ExportFailure::SnapshotNotIntact.into());
        }
        self.files.push(ManifestFile {
            path: "database/lis.sqlite3".to_owned(),
            sha256: snapshot.digest.to_string(),
            bytes: snapshot.size,
        });
        let facts = read_facts(&database, self.started.installation, self.job)
            .map_err(|_| ExportFailure::SnapshotFailed)?;
        let artifact_count = self.copy_artifacts(&facts.required_artifacts)?;
        self.put("audit/anchors.jsonl", facts.anchor_lines.as_bytes())?;
        self.put("schema/schema.sql", facts.schema_sql.as_bytes())?;
        self.put("schema/data-dictionary.json", &facts.dictionary_json)?;
        self.put(
            "schema/data-dictionary.md",
            facts.dictionary_markdown.as_bytes(),
        )?;
        for (path, sql) in migration_sources(self.job.modules) {
            self.put(&path, sql.as_bytes())?;
        }
        let verifier_sha256 = ArtifactDigest::of_reader(VERIFIER.as_bytes())?
            .0
            .to_string();
        self.put("tools/verify_export.py", VERIFIER.as_bytes())?;
        self.put("README.txt", readme(&verifier_sha256).as_bytes())?;
        let manifest = ExitManifest {
            format: EXIT_FORMAT.to_owned(),
            installation_id: self.started.installation,
            export_id: self.started.export_id,
            created_at: self.started.created_at,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            schema: facts.schema,
            files: self.files.clone(),
            heads: facts.heads,
            seal_count: facts.seal_count,
            artifact_references: facts.references,
            artifact_problems: Vec::new(),
        };
        let (manifest_sha256, manifest_bytes) = self.write_manifest(&manifest)?;
        self.read_back(&manifest, &manifest_sha256, manifest_bytes)?;
        lis_store::sync_dir(&self.dir)?;
        Ok(Written {
            package_name: self.package_name(),
            manifest_sha256,
            verifier_sha256,
            database_bytes: snapshot.size,
            artifact_count,
            artifact_problems: manifest.artifact_problems.len(),
        })
    }

    fn package_name(&self) -> String {
        self.dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Copies every listed artifact, verifying its digest as it is read. One that cannot
    /// be read is a manifest problem, never silently left out.
    fn copy_artifacts(&mut self, required: &BTreeSet<String>) -> Result<usize, Halt> {
        let store = self.job.db.artifacts();
        let listing = store
            .digests()
            .map_err(|_| ExportFailure::ArtifactListingFailed)?;
        if !listing.unexpected.is_empty() {
            return Err(ExportFailure::ArtifactListingFailed.into());
        }
        let mut copied = BTreeSet::new();
        for digest in &listing.digests {
            if self.job.cancel.load(Ordering::Relaxed) {
                return Err(Halt::Cancelled);
            }
            match store.read(digest) {
                Ok(bytes) => {
                    self.put(&format!("artifacts/{digest}"), &bytes)?;
                    copied.insert(digest.to_string());
                }
                Err(_) => return Err(ExportFailure::ArtifactListingFailed.into()),
            }
        }
        if !required.is_subset(&copied) {
            return Err(ExportFailure::ArtifactListingFailed.into());
        }
        Ok(copied.len())
    }

    /// Writes `bytes` durably at `relative` and lists it.
    fn put(&mut self, relative: &str, bytes: &[u8]) -> Result<(), Halt> {
        let path = self.path_of(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_file(&path, bytes)?;
        let (digest, length) = ArtifactDigest::of_reader(bytes)?;
        self.files.push(ManifestFile {
            path: relative.to_owned(),
            sha256: digest.to_string(),
            bytes: length,
        });
        Ok(())
    }

    fn path_of(&self, relative: &str) -> PathBuf {
        relative
            .split('/')
            .fold(self.dir.clone(), |path, part| path.join(part))
    }

    /// Writes `manifest.json` last and returns its SHA-256.
    fn write_manifest(&self, manifest: &ExitManifest) -> Result<(String, u64), Halt> {
        let mut bytes =
            serde_json::to_vec_pretty(manifest).map_err(|_| ExportFailure::WriteFailed)?;
        bytes.push(b'\n');
        write_file(&self.dir.join("manifest.json"), &bytes)?;
        let (digest, length) = ArtifactDigest::of_reader(bytes.as_slice())?;
        Ok((digest.to_string(), length))
    }

    /// Reads every listed file and `manifest.json` back and compares the manifest's
    /// digest, length and parsed value (§5 step 5).
    fn read_back(
        &self,
        manifest: &ExitManifest,
        manifest_sha256: &str,
        manifest_bytes: u64,
    ) -> Result<(), Halt> {
        for file in &self.files {
            let (digest, length) =
                ArtifactDigest::of_reader(fs::File::open(self.path_of(&file.path))?)?;
            if digest.to_string() != file.sha256 || length != file.bytes {
                return Err(ExportFailure::ReadbackMismatch.into());
            }
        }
        read_manifest_back(
            &self.dir.join("manifest.json"),
            manifest,
            manifest_sha256,
            manifest_bytes,
        )?;
        Ok(())
    }
}

fn read_manifest_back(
    path: &std::path::Path,
    expected: &ExitManifest,
    expected_sha256: &str,
    expected_bytes: u64,
) -> Result<(), Halt> {
    let bytes = fs::read(path)?;
    let (digest, length) = ArtifactDigest::of_reader(bytes.as_slice())?;
    let parsed: ExitManifest =
        serde_json::from_slice(&bytes).map_err(|_| ExportFailure::ReadbackMismatch)?;
    if digest.to_string() != expected_sha256 || length != expected_bytes || parsed != *expected {
        return Err(ExportFailure::ReadbackMismatch.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lis_core::{InstallationId, UtcTimestamp};
    use lis_store::ArtifactDigest;
    use tempfile::tempdir;

    use super::{EXIT_FORMAT, ExitManifest, read_manifest_back};
    use crate::exit_export::ExitExportId;

    #[test]
    fn a_manifest_changed_after_write_fails_readback() {
        let manifest = ExitManifest {
            format: EXIT_FORMAT.to_owned(),
            installation_id: InstallationId::generate(),
            export_id: ExitExportId::generate(),
            created_at: UtcTimestamp::parse("2026-10-13T12:00:00.000000000Z").unwrap(),
            app_version: "0.1.0".to_owned(),
            schema: Vec::new(),
            files: Vec::new(),
            heads: Vec::new(),
            seal_count: 0,
            artifact_references: Vec::new(),
            artifact_problems: Vec::new(),
        };
        let dir = tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let (digest, length) = ArtifactDigest::of_reader(bytes.as_slice()).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::write(&path, b"{}").unwrap();

        assert!(read_manifest_back(&path, &manifest, &digest.to_string(), length).is_err());
    }
}
