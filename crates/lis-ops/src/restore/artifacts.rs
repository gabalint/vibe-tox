//! Artifact re-verification and movement during restore apply (REQ-OPS-007). Expected
//! digests and lengths come from the authenticated backup manifest persisted in the
//! restore marker; both staged files and live files selected for reuse cross this trust
//! boundary again before a database moves.

use std::fs;
use std::io;
use std::path::Path;

use lis_store::{ARTIFACTS_DIR, ArtifactDigest};

use super::apply::{ApplyFailure, file_digest};
use crate::backup::ArtifactEntry;
use crate::backup::durable::rename;

/// Rechecks every expected artifact in staging, or in the live store after that staged
/// file has already moved during an `APPLYING` resume.
pub(super) fn recheck_artifacts(
    data_dir: &Path,
    staging: &Path,
    expected: &[ArtifactEntry],
) -> Result<Option<ApplyFailure>, crate::backup::BackupError> {
    for artifact in expected {
        let staged = staging.join(ARTIFACTS_DIR).join(&artifact.sha256);
        if staged.exists() {
            if !artifact_matches(&staged, artifact)? {
                return Ok(Some(ApplyFailure::StagedArtifactsChanged));
            }
            let live = data_dir.join(ARTIFACTS_DIR).join(&artifact.sha256);
            if live.exists() {
                // A matching live file is reused; a nonmatching one will be replaced.
                let _verified_for_reuse = artifact_matches(&live, artifact)?;
            }
        } else if !artifact_matches(
            &data_dir.join(ARTIFACTS_DIR).join(&artifact.sha256),
            artifact,
        )? {
            return Ok(Some(ApplyFailure::StagedArtifactsChanged));
        }
    }
    Ok(None)
}

fn artifact_matches(path: &Path, expected: &ArtifactEntry) -> io::Result<bool> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(metadata.len() == expected.bytes
        && file_digest(path)?.as_deref() == Some(expected.sha256.as_str()))
}

/// Moves each staged artifact into the store. An existing file whose digest verifies is
/// kept; one that does not is moved to `replaced/artifacts/` first.
pub(super) fn move_artifacts(staged: &Path, store: &Path, replaced: &Path) -> io::Result<()> {
    let entries = match fs::read_dir(staged) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    fs::create_dir_all(store)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.parse::<ArtifactDigest>().is_err() {
            continue;
        }
        let existing = store.join(&name);
        match file_digest(&existing)? {
            Some(digest) if digest == name => fs::remove_file(entry.path())?,
            Some(_) => {
                fs::create_dir_all(replaced.join(ARTIFACTS_DIR))?;
                rename(&existing, &replaced.join(ARTIFACTS_DIR).join(&name))?;
                rename(&entry.path(), &existing)?;
            }
            None => rename(&entry.path(), &existing)?,
        }
    }
    Ok(())
}
