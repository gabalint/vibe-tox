//! Content-addressed artifact files: report PDFs, raw interface payloads.
//!
//! Each file is named by the SHA-256 of its bytes. `put` writes and syncs the
//! bytes before returning, so a database row may reference the digest only
//! after `put` succeeds (implementation plan, "Artifacts"). `read` verifies the
//! digest, so a replaced or damaged file is reported, never served.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::error::StoreError;

/// Name prefix of in-progress writes; never a valid digest.
const TEMP_PREFIX: &str = ".tmp-";

/// Largest artifact the store accepts, in bytes.
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

/// Lowercase hex SHA-256 of an artifact's bytes. Parsing it is what makes it
/// safe to use as a file name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactDigest(String);

impl ArtifactDigest {
    pub(crate) fn of_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    /// Digest and length of everything `reader` yields, read in constant memory.
    pub fn of_reader(mut reader: impl Read) -> io::Result<(Self, u64)> {
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        let mut size = 0_u64;
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok((Self(hex::encode(hasher.finalize())), size));
            }
            hasher.update(&buffer[..read]);
            size += read as u64;
        }
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ArtifactDigest {
    type Err = StoreError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let valid = text.len() == 64
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(StoreError::InvalidDigest)
        }
    }
}

/// A stored artifact's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    /// Digest of the bytes, which is also the file name.
    pub digest: ArtifactDigest,
    /// Length in bytes.
    pub size: u64,
}

/// What the artifact folder holds (REQ-OPS-007, REQ-OPS-010).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactListing {
    /// Every stored artifact, in ascending digest order.
    pub digests: Vec<ArtifactDigest>,
    /// Names of entries that are neither an artifact nor an in-progress
    /// write, such as a file copied in by hand or a folder; sorted. They are
    /// reported, never served or copied. The names are not chosen by the
    /// application, so they are not logged.
    pub unexpected: Vec<String>,
}

/// A directory of content-addressed files.
#[derive(Debug)]
pub struct ArtifactStore {
    dir: PathBuf,
    max_bytes: u64,
    next_temp: AtomicU64,
}

impl ArtifactStore {
    /// Opens (creating if needed) the store in `dir`, accepting up to `max_bytes` per artifact.
    ///
    /// Removes temporary files left by a `put` that a crash interrupted. Only
    /// one process may use `dir`: `Database` opens it under its instance lock.
    pub fn open(dir: impl Into<PathBuf>, max_bytes: u64) -> Result<Self, StoreError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        if let Some(parent) = dir.parent() {
            sync_dir(parent)?;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(Self {
            dir,
            max_bytes,
            next_temp: AtomicU64::new(0),
        })
    }

    /// Stores `bytes` durably and returns their digest and size.
    ///
    /// Writes a temporary file in the same directory, syncs it, renames it to
    /// the digest and syncs the directory. If the digest already exists, the
    /// existing file is verified and not rewritten.
    pub fn put(&self, bytes: &[u8]) -> Result<StoredArtifact, StoreError> {
        let size = u64::try_from(bytes.len()).map_err(|_| StoreError::ArtifactTooLarge)?;
        if size > self.max_bytes {
            return Err(StoreError::ArtifactTooLarge);
        }
        let stored = StoredArtifact {
            digest: ArtifactDigest::of_bytes(bytes),
            size,
        };
        let path = self.path(&stored.digest);
        if path.exists() {
            self.read(&stored.digest)?;
            // An earlier put may have crashed between its rename and directory sync.
            sync_dir(&self.dir)?;
            return Ok(stored);
        }
        let temp = self.dir.join(format!(
            "{TEMP_PREFIX}{}-{}",
            stored.digest,
            self.next_temp.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, &path)?;
        sync_dir(&self.dir)?;
        Ok(stored)
    }

    /// Reads the artifact and verifies its digest.
    pub fn read(&self, digest: &ArtifactDigest) -> Result<Vec<u8>, StoreError> {
        let file = match File::open(self.path(digest)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::ArtifactNotFound);
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        // Read one byte past the limit to detect an oversized (replaced) file.
        file.take(self.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if ArtifactDigest::of_bytes(&bytes) != *digest {
            return Err(StoreError::ArtifactCorrupt);
        }
        Ok(bytes)
    }

    /// Lists the folder: every artifact digest, and every other entry.
    ///
    /// Ignores temporary files of `put`s in progress. Nothing deletes
    /// artifacts, so the listing is every artifact ever stored, plus any put
    /// that completes while it runs. Digests are not verified here; `read`
    /// verifies each one.
    pub fn digests(&self) -> Result<ArtifactListing, StoreError> {
        let mut listing = ArtifactListing {
            digests: Vec::new(),
            unexpected: Vec::new(),
        };
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(TEMP_PREFIX) {
                continue;
            }
            match name.parse::<ArtifactDigest>() {
                Ok(digest) if entry.file_type()?.is_file() => listing.digests.push(digest),
                Ok(_) | Err(_) => listing.unexpected.push(name),
            }
        }
        listing.digests.sort_by(|a, b| a.0.cmp(&b.0));
        listing.unexpected.sort();
        Ok(listing)
    }

    fn path(&self, digest: &ArtifactDigest) -> PathBuf {
        self.dir.join(&digest.0)
    }
}

/// Makes a rename or file creation in `dir` durable on Unix.
///
/// Known limitation: Windows cannot open a directory with `std::fs`, so a
/// rename there may be lost on power failure even after this returns. Fixing
/// that needs `MoveFileExW` with `MOVEFILE_WRITE_THROUGH`, which the library
/// policy does not admit yet.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    if cfg!(unix) {
        File::open(dir)?.sync_all()?;
    }
    Ok(())
}
