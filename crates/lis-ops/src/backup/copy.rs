//! Writing one verified set to one target (M4 design brief §3.1 step 6, §3.5).

use std::fs::{self, File};
use std::io::{self, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use lis_store::{ArtifactDigest, ArtifactStore, StoreError};
use sha2::{Digest, Sha256};

use super::CopyFailure;
use super::durable::{self, TEMP_SUFFIX};
use super::manifest::{
    ArtifactEntry, ArtifactProblem, DATABASE_FILE, MANIFEST_FILE, Manifest, SETS_FOLDER,
};
use crate::format::{self, BackupKey, Content, FormatError, Written};

/// Folder of a target's encrypted artifacts, inside its root; never pruned in M4.
const ARTIFACTS_FOLDER: &str = "artifacts";
const ARTIFACT_SUFFIX: &str = ".vtbk";

/// The encrypted file of the artifact with digest `sha256` in the target root `root`.
pub(crate) fn artifact_path(root: &Path, sha256: &str) -> PathBuf {
    root.join(ARTIFACTS_FOLDER)
        .join(format!("{sha256}{ARTIFACT_SUFFIX}"))
}

/// What every target's set is made of.
pub(super) struct SetSource<'a> {
    /// The intact plaintext snapshot.
    pub(super) snapshot: &'a Path,
    /// The artifact store, read with digest verification.
    pub(super) store: &'a ArtifactStore,
    /// The store's folder, for artifact lengths without reading them.
    pub(super) store_dir: &'a Path,
    /// Every artifact listed, in digest order.
    pub(super) digests: &'a [ArtifactDigest],
    /// The manifest without artifacts; each target adds what it holds.
    pub(super) manifest: &'a Manifest,
}

/// A verified set.
pub(super) struct Copied {
    /// Bytes written to the target by this run.
    pub(super) written_bytes: u64,
    /// Artifacts this target's set lacks.
    pub(super) problems: Vec<ArtifactProblem>,
}

/// Why a copy stopped.
pub(super) enum CopyError {
    /// The cancellation flag was raised; nothing of the set remains.
    Cancelled,
    /// The copy failed; nothing of the set remains.
    Failed {
        failure: CopyFailure,
        written_bytes: u64,
    },
}

/// Why a step stopped, before the written-byte count is attached.
enum Stop {
    Cancelled,
    Failed(CopyFailure),
}

impl From<io::Error> for Stop {
    fn from(_: io::Error) -> Self {
        Self::Failed(CopyFailure::WriteFailed)
    }
}

impl From<FormatError> for Stop {
    fn from(error: FormatError) -> Self {
        match error {
            FormatError::Cancelled => Self::Cancelled,
            FormatError::NotVtbk
            | FormatError::WrongContent
            | FormatError::KeyMismatch
            | FormatError::Corrupt
            | FormatError::TooLarge
            | FormatError::Random
            | FormatError::Io(_) => Self::Failed(CopyFailure::WriteFailed),
        }
    }
}

/// Writes the set `manifest.set_id` to the target at `root`, reads it back, and renames
/// it into place. On any failure or cancellation the partial set is removed.
pub(super) fn copy_set(
    root: &Path,
    source: &SetSource<'_>,
    key: &BackupKey,
    cancel: &AtomicBool,
) -> Result<Copied, CopyError> {
    let mut written_bytes = 0;
    let set_path = root.join(SETS_FOLDER).join(&source.manifest.set_id);
    let temp = durable::temp_path(&set_path).map_err(|_| CopyError::Failed {
        failure: CopyFailure::WriteFailed,
        written_bytes: 0,
    })?;
    match write_and_verify(root, &temp, source, key, cancel, &mut written_bytes) {
        Ok(problems) => Ok(Copied {
            written_bytes,
            problems,
        }),
        Err(stop) => {
            remove_set_folder(&temp);
            Err(match stop {
                Stop::Cancelled => CopyError::Cancelled,
                Stop::Failed(failure) => CopyError::Failed {
                    failure,
                    written_bytes,
                },
            })
        }
    }
}

/// Removes this run's set from the target at `root`, when an interrupted run had already
/// verified it.
pub(super) fn discard_set(root: &Path, set_id: &str) {
    remove_set_folder(&root.join(SETS_FOLDER).join(set_id));
}

fn remove_set_folder(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(error_kind = ?error.kind(), "partial backup set not removed"),
    }
}

fn write_and_verify(
    root: &Path,
    temp: &Path,
    source: &SetSource<'_>,
    key: &BackupKey,
    cancel: &AtomicBool,
    written_bytes: &mut u64,
) -> Result<Vec<ArtifactProblem>, Stop> {
    let sets = root.join(SETS_FOLDER);
    let artifacts = root.join(ARTIFACTS_FOLDER);
    fs::create_dir_all(&sets)?;
    fs::create_dir_all(&artifacts)?;
    remove_leftovers(&sets, &artifacts)?;
    fs::create_dir(temp)?;
    let database = encrypt_file(
        &temp.join(DATABASE_FILE),
        key,
        Content::Database,
        File::open(source.snapshot)?,
        cancel,
    )?;
    *written_bytes += database.ciphertext_bytes;
    let (entries, problems) = copy_artifacts(&artifacts, source, key, cancel, written_bytes)?;
    let mut manifest = source.manifest.clone();
    manifest.artifacts = entries;
    manifest.artifact_problems.extend(problems.iter().cloned());
    let manifest_bytes =
        serde_json::to_vec(&manifest).map_err(|_| Stop::Failed(CopyFailure::WriteFailed))?;
    let manifest_file = temp.join(MANIFEST_FILE);
    let written = encrypt_file(
        &manifest_file,
        key,
        Content::Manifest,
        manifest_bytes.as_slice(),
        cancel,
    )?;
    *written_bytes += written.ciphertext_bytes;
    read_back(temp, key, &database, &written, &manifest_bytes, cancel)?;
    durable::sync_parent(&manifest_file)?;
    durable::rename(temp, &sets.join(&manifest.set_id))?;
    Ok(problems)
}

/// Removes sets and artifact files that an interrupted run left half-written.
fn remove_leftovers(sets: &Path, artifacts: &Path) -> io::Result<()> {
    for entry in fs::read_dir(sets)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::remove_file(entry.path())?;
            }
        }
    }
    for entry in fs::read_dir(artifacts)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Copies every listed artifact the target lacks. An existing encrypted artifact is
/// reused only after authenticated decryption proves its plaintext length and digest;
/// length alone is never evidence that a copy is intact.
fn copy_artifacts(
    dir: &Path,
    source: &SetSource<'_>,
    key: &BackupKey,
    cancel: &AtomicBool,
    written_bytes: &mut u64,
) -> Result<(Vec<ArtifactEntry>, Vec<ArtifactProblem>), Stop> {
    let mut entries = Vec::with_capacity(source.digests.len());
    let mut problems = Vec::new();
    for digest in source.digests {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Cancelled);
        }
        let name = digest.to_string();
        let path = dir.join(format!("{name}{ARTIFACT_SUFFIX}"));
        let local = fs::metadata(source.store_dir.join(&name)).map(|meta| meta.len());
        let present = match &local {
            Ok(bytes) => encrypted_artifact_matches(&path, key, &name, *bytes, cancel)?,
            Err(_) => false,
        };
        if let (true, Ok(bytes)) = (present, local) {
            entries.push(ArtifactEntry {
                sha256: name,
                bytes,
            });
            continue;
        }
        let bytes = match source.store.read(digest) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(ArtifactProblem {
                    sha256: Some(name),
                    problem: problem_code(&error).to_owned(),
                });
                continue;
            }
        };
        let temp = durable::temp_path(&path)?;
        let written = encrypt_file(&temp, key, Content::Artifact, bytes.as_slice(), cancel)?;
        durable::rename(&temp, &path)?;
        *written_bytes += written.ciphertext_bytes;
        if !new_artifact_matches(&path, key, &name, &written, cancel)? {
            if let Err(error) = fs::remove_file(&path)
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!(error_kind = ?error.kind(), "failed artifact readback not removed");
            }
            return Err(Stop::Failed(CopyFailure::ReadbackMismatch));
        }
        entries.push(ArtifactEntry {
            sha256: name,
            bytes: written.plaintext_bytes,
        });
    }
    Ok((entries, problems))
}

/// Reopens a newly published artifact and checks both representations: the ciphertext
/// length and digest written by [`encrypt_file`], then the authenticated plaintext's
/// length and content-addressed digest.
fn new_artifact_matches(
    path: &Path,
    key: &BackupKey,
    expected_sha256: &str,
    written: &Written,
    cancel: &AtomicBool,
) -> Result<bool, Stop> {
    let metadata = fs::metadata(path)?;
    if metadata.len() != written.ciphertext_bytes || file_sha256(path)? != written.ciphertext_sha256
    {
        return Ok(false);
    }
    encrypted_artifact_matches(path, key, expected_sha256, written.plaintext_bytes, cancel)
}

fn encrypted_artifact_matches(
    path: &Path,
    key: &BackupKey,
    expected_sha256: &str,
    expected_bytes: u64,
    cancel: &AtomicBool,
) -> Result<bool, Stop> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Ok(false),
    };
    if metadata.len() != format::encrypted_len(expected_bytes) {
        return Ok(false);
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return Ok(false),
    };
    if !file.metadata().is_ok_and(|opened| opened.is_file())
        || !fs::symlink_metadata(path).is_ok_and(|named| named.file_type().is_file())
    {
        return Ok(false);
    }
    let mut plaintext = Vec::with_capacity(usize::try_from(expected_bytes).unwrap_or(0));
    match format::decrypt(key, Content::Artifact, file, &mut plaintext, cancel) {
        Err(FormatError::Cancelled) => Err(Stop::Cancelled),
        Err(_) => Ok(false),
        Ok(bytes) => {
            Ok(bytes == expected_bytes
                && hex::encode(Sha256::digest(&plaintext)) == expected_sha256)
        }
    }
}

fn problem_code(error: &StoreError) -> &'static str {
    match error {
        StoreError::ArtifactNotFound => "missing",
        StoreError::ArtifactCorrupt | StoreError::ArtifactTooLarge => "damaged",
        _ => "unreadable",
    }
}

/// Encrypts `reader` into a new file at `path` and syncs it.
fn encrypt_file(
    path: &Path,
    key: &BackupKey,
    content: Content,
    reader: impl Read,
    cancel: &AtomicBool,
) -> Result<Written, Stop> {
    let mut writer = BufWriter::new(durable::create(path)?);
    let written = format::encrypt(key, content, reader, &mut writer, cancel)?;
    let file = writer
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    file.sync_all()?;
    Ok(written)
}

/// Reads both files back: their ciphertext digests must equal what was written, and the
/// manifest must decrypt to the bytes in memory.
fn read_back(
    temp: &Path,
    key: &BackupKey,
    database: &Written,
    manifest: &Written,
    manifest_bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<(), Stop> {
    let mismatch = Stop::Failed(CopyFailure::ReadbackMismatch);
    if file_sha256(&temp.join(DATABASE_FILE))? != database.ciphertext_sha256
        || file_sha256(&temp.join(MANIFEST_FILE))? != manifest.ciphertext_sha256
    {
        return Err(mismatch);
    }
    let mut decrypted = Vec::with_capacity(manifest_bytes.len());
    let file = File::open(temp.join(MANIFEST_FILE))?;
    match format::decrypt(key, Content::Manifest, file, &mut decrypted, cancel) {
        Ok(_) if decrypted == manifest_bytes => Ok(()),
        Err(FormatError::Cancelled) => Err(Stop::Cancelled),
        Ok(_) | Err(_) => Err(mismatch),
    }
}

fn file_sha256(path: &Path) -> io::Result<String> {
    Ok(ArtifactDigest::of_reader(File::open(path)?)?.0.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicBool;

    use lis_auth::DataKey;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{Content, encrypt_file, encrypted_artifact_matches, new_artifact_matches};
    use crate::format::BackupKey;

    #[test]
    fn same_length_corrupt_target_artifact_is_not_reused() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("artifact.vtbk");
        let plaintext = b"synthetic artifact";
        let digest = hex::encode(Sha256::digest(plaintext));
        let key = BackupKey::derive(&DataKey::from_bytes([7; 32]));
        let cancel = AtomicBool::new(false);
        assert!(encrypt_file(&path, &key, Content::Artifact, &plaintext[..], &cancel).is_ok());
        let mut corrupted = fs::read(&path).unwrap();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        fs::write(&path, corrupted).unwrap();

        assert!(matches!(
            encrypted_artifact_matches(&path, &key, &digest, plaintext.len() as u64, &cancel),
            Ok(false)
        ));
    }

    #[test]
    fn a_new_artifact_corrupted_after_write_fails_readback() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("artifact.vtbk");
        let plaintext = b"synthetic newly written artifact";
        let digest = hex::encode(Sha256::digest(plaintext));
        let key = BackupKey::derive(&DataKey::from_bytes([7; 32]));
        let cancel = AtomicBool::new(false);
        let Ok(written) = encrypt_file(&path, &key, Content::Artifact, &plaintext[..], &cancel)
        else {
            panic!("the synthetic artifact encrypts");
        };
        let mut corrupted = fs::read(&path).unwrap();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        fs::write(&path, corrupted).unwrap();

        assert!(
            matches!(
                new_artifact_matches(&path, &key, &digest, &written, &cancel),
                Ok(false)
            ),
            "a changed artifact must never be counted as verified"
        );
    }
}
