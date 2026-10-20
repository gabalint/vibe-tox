//! The `vtbk1` encrypted file format of backup copies (REQ-OPS-007; M4 design brief §3.2,
//! D2): the one home of the format, specified with a reference decryptor and this
//! module's test vector in `docs/architecture/backup-and-export-formats.md` (`vtbk1`).
//!
//! ```text
//! header (88) = "VTBK" | 0x01 | content | 0x0000 | keyId (16) | wrapNonce (12)
//!             | wrappedFileKey (32 + 16 tag) | chunkBytes (u32 BE = 1 MiB)
//! wrappedFileKey = AEAD(KEK, wrapNonce, fileKey, aad = header[0..24] ‖ "file-key")
//! chunk i        = AEAD(fileKey, 7 zero bytes ‖ BE32(i) ‖ final flag, plaintext i, aad = header)
//! ```
//!
//! Invariants: each file has its own random `fileKey`; every chunk but the last holds
//! exactly [`CHUNK_BYTES`] and the last holds fewer (possibly none), so the final flag is
//! never ambiguous; the header authenticates every chunk, so an edited header, a foreign,
//! dropped or reordered chunk, a missing final chunk or another key all fail; [`decrypt`]
//! output is trusted only once it returns `Ok`. The KEK and key ID are derived from the
//! installation key with domain-separated SHA-256, never the TOTP-sealing key itself.

use std::fmt;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use chacha20poly1305::aead::{AeadInOut, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, Tag};
use lis_auth::DataKey;
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Length of the header in front of the chunks.
pub const HEADER_BYTES: usize = 88;
/// Plaintext bytes in every chunk but the last.
pub const CHUNK_BYTES: usize = 1 << 20;

const MAGIC: &[u8; 4] = b"VTBK";
const VERSION: u8 = 1;
const TAG_BYTES: usize = 16;
const KEY_ID_BYTES: usize = 16;
const NONCE_BYTES: usize = 12;
/// End of the header prefix that the wrapped file key authenticates.
const PREFIX_BYTES: usize = 24;
const KEK_DOMAIN: &[u8] = b"vibe-tox/backup-kek/v1";
const KEY_ID_DOMAIN: &[u8] = b"vibe-tox/key-id/v1";
const WRAP_LABEL: &[u8] = b"file-key";

/// What a file holds. The header names it, so one kind of file cannot stand in for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// The database snapshot.
    Database,
    /// One content-addressed artifact.
    Artifact,
    /// A backup set's manifest.
    Manifest,
}

impl Content {
    fn code(self) -> u8 {
        match self {
            Self::Database => 1,
            Self::Artifact => 2,
            Self::Manifest => 3,
        }
    }
}

/// The first 16 bytes of `SHA-256("vibe-tox/key-id/v1" ‖ installation key)`. It names the
/// key in headers, target markers, manifests and kit records without revealing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyId([u8; KEY_ID_BYTES]);

impl fmt::Display for KeyId {
    /// 32 lowercase hexadecimal digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// The backup key-encryption key derived from the installation key, with its key ID.
/// Holding it permits reading and writing backups only, not opening TOTP secrets.
pub struct BackupKey {
    kek: Zeroizing<[u8; 32]>,
    key_id: KeyId,
}

impl BackupKey {
    /// Derives the KEK and key ID from the installation key (M4 design brief D2).
    #[must_use]
    pub fn derive(installation_key: &DataKey) -> Self {
        let kek = Zeroizing::new(domain_hash(KEK_DOMAIN, installation_key.expose()));
        let id = domain_hash(KEY_ID_DOMAIN, installation_key.expose());
        let mut key_id = [0; KEY_ID_BYTES];
        key_id.copy_from_slice(&id[..KEY_ID_BYTES]);
        Self {
            kek,
            key_id: KeyId(key_id),
        }
    }

    /// The key ID that files and markers written with this key carry.
    #[must_use]
    pub fn key_id(&self) -> KeyId {
        self.key_id
    }
}

impl fmt::Debug for BackupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BackupKey({}, [redacted])", self.key_id)
    }
}

fn domain_hash(domain: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(key);
    hasher.finalize().into()
}

/// Why a file could not be written or read. Messages are PHI-free and name no path.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// The bytes do not start with a `vtbk1` header this build reads.
    #[error("not a vtbk1 file")]
    NotVtbk,
    /// The header names another kind of content.
    #[error("the file holds another kind of content")]
    WrongContent,
    /// The file was written under another installation key.
    #[error("the file was written under another key")]
    KeyMismatch,
    /// Authentication failed: the file is truncated, damaged or altered.
    #[error("the file is truncated, damaged or altered")]
    Corrupt,
    /// The plaintext would need more chunks than the 32-bit counter allows.
    #[error("the file is too large for the format")]
    TooLarge,
    /// The cancellation flag was raised between chunks.
    #[error("cancelled")]
    Cancelled,
    /// The OS random number generator failed.
    #[error("the random number generator failed")]
    Random,
    /// Reading or writing failed.
    #[error("file system error: {0}")]
    Io(#[from] io::Error),
}

/// What [`encrypt`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Lowercase hex SHA-256 of the whole file, header included, computed while writing.
    pub ciphertext_sha256: String,
    /// Length of the whole file.
    pub ciphertext_bytes: u64,
    /// Length of the plaintext.
    pub plaintext_bytes: u64,
}

/// Encrypts everything `reader` yields into `writer` under a fresh random file key, in
/// constant memory. Checks `cancel` before every chunk. Does not sync or flush `writer`.
pub fn encrypt(
    key: &BackupKey,
    content: Content,
    reader: impl Read,
    writer: impl Write,
    cancel: &AtomicBool,
) -> Result<Written, FormatError> {
    let file_key = Zeroizing::new(random()?);
    let wrap_nonce = random()?;
    let file = FileKeys {
        file_key: &file_key,
        wrap_nonce,
    };
    encrypt_with(key, content, &file, reader, writer, cancel)
}

/// Decrypts a file written by [`encrypt`] with the same key and content kind into
/// `writer`, in constant memory, and returns the plaintext length. Plaintext is written
/// as each chunk authenticates: trust the output only when this returns `Ok`.
pub fn decrypt(
    key: &BackupKey,
    content: Content,
    mut reader: impl Read,
    mut writer: impl Write,
    cancel: &AtomicBool,
) -> Result<u64, FormatError> {
    let mut header = [0; HEADER_BYTES];
    if read_full(&mut reader, &mut header)? < HEADER_BYTES {
        return Err(FormatError::NotVtbk);
    }
    let file_key = open_header(key, content, &header)?;
    let cipher = ChaCha20Poly1305::new(&Key::from(*file_key));
    let mut buffer = vec![0; CHUNK_BYTES + TAG_BYTES];
    let mut plaintext_bytes = 0_u64;
    for index in 0..=u32::MAX {
        if cancel.load(Ordering::Relaxed) {
            return Err(FormatError::Cancelled);
        }
        let read = read_full(&mut reader, &mut buffer)?;
        let Some(message_bytes) = read.checked_sub(TAG_BYTES) else {
            return Err(FormatError::Corrupt);
        };
        let last = read < buffer.len();
        let (message, tag) = buffer[..read].split_at_mut(message_bytes);
        let tag = Tag::try_from(&*tag).map_err(|_| FormatError::Corrupt)?;
        cipher
            .decrypt_inout_detached(&chunk_nonce(index, last), &header, message.into(), &tag)
            .map_err(|_| FormatError::Corrupt)?;
        writer.write_all(message)?;
        plaintext_bytes += message_bytes as u64;
        if last {
            return Ok(plaintext_bytes);
        }
    }
    Err(FormatError::TooLarge)
}

/// Length of the file [`encrypt`] writes for `plaintext_bytes` of plaintext. A copy whose
/// length differs is incomplete or not this plaintext's.
#[must_use]
pub fn encrypted_len(plaintext_bytes: u64) -> u64 {
    let chunks = plaintext_bytes / CHUNK_BYTES as u64 + 1;
    HEADER_BYTES as u64 + plaintext_bytes + chunks * TAG_BYTES as u64
}

/// The per-file secrets; random in [`encrypt`], fixed in the test vector.
struct FileKeys<'a> {
    file_key: &'a [u8; 32],
    wrap_nonce: [u8; NONCE_BYTES],
}

fn encrypt_with(
    key: &BackupKey,
    content: Content,
    file: &FileKeys<'_>,
    mut reader: impl Read,
    writer: impl Write,
    cancel: &AtomicBool,
) -> Result<Written, FormatError> {
    let header = seal_header(key, content, file)?;
    let mut writer = HashingWriter::new(writer);
    writer.write_all(&header)?;
    let cipher = ChaCha20Poly1305::new(&Key::from(*file.file_key));
    let mut buffer = vec![0; CHUNK_BYTES];
    let mut plaintext_bytes = 0_u64;
    for index in 0..=u32::MAX {
        if cancel.load(Ordering::Relaxed) {
            return Err(FormatError::Cancelled);
        }
        let read = read_full(&mut reader, &mut buffer)?;
        let last = read < CHUNK_BYTES;
        let message = &mut buffer[..read];
        let tag = cipher
            .encrypt_inout_detached(&chunk_nonce(index, last), &header, message.into())
            .map_err(|_| FormatError::TooLarge)?;
        writer.write_all(message)?;
        writer.write_all(&tag)?;
        plaintext_bytes += read as u64;
        if last {
            return Ok(writer.finish(plaintext_bytes));
        }
    }
    Err(FormatError::TooLarge)
}

/// `7 zero bytes ‖ BE32(index) ‖ final flag`: the STREAM nonce layout of RustCrypto's
/// `StreamBE32`.
fn chunk_nonce(index: u32, last: bool) -> Nonce {
    let mut nonce = [0; NONCE_BYTES];
    nonce[7..11].copy_from_slice(&index.to_be_bytes());
    nonce[11] = u8::from(last);
    Nonce::from(nonce)
}

/// The header with the file key wrapped under the KEK.
fn seal_header(
    key: &BackupKey,
    content: Content,
    file: &FileKeys<'_>,
) -> Result<[u8; HEADER_BYTES], FormatError> {
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(MAGIC);
    header[4] = VERSION;
    header[5] = content.code();
    header[8..PREFIX_BYTES].copy_from_slice(&key.key_id.0);
    header[24..36].copy_from_slice(&file.wrap_nonce);
    let mut wrapped = *file.file_key;
    let tag = ChaCha20Poly1305::new(&Key::from(*key.kek))
        .encrypt_inout_detached(
            &Nonce::from(file.wrap_nonce),
            &wrap_aad(&header),
            wrapped.as_mut_slice().into(),
        )
        .map_err(|_| FormatError::TooLarge)?;
    header[36..68].copy_from_slice(&wrapped);
    header[68..84].copy_from_slice(&tag);
    header[84..].copy_from_slice(&(CHUNK_BYTES as u32).to_be_bytes());
    Ok(header)
}

/// Checks the header and unwraps the file key.
fn open_header(
    key: &BackupKey,
    content: Content,
    header: &[u8; HEADER_BYTES],
) -> Result<Zeroizing<[u8; 32]>, FormatError> {
    let chunk_bytes = (CHUNK_BYTES as u32).to_be_bytes();
    if &header[..4] != MAGIC || header[4] != VERSION || header[6..8] != [0, 0] {
        return Err(FormatError::NotVtbk);
    }
    if header[84..] != chunk_bytes {
        return Err(FormatError::NotVtbk);
    }
    if header[5] != content.code() {
        return Err(FormatError::WrongContent);
    }
    if header[8..PREFIX_BYTES] != key.key_id.0 {
        return Err(FormatError::KeyMismatch);
    }
    let mut nonce = [0; NONCE_BYTES];
    nonce.copy_from_slice(&header[24..36]);
    let mut file_key = Zeroizing::new([0; 32]);
    file_key.copy_from_slice(&header[36..68]);
    let tag = Tag::try_from(&header[68..84]).map_err(|_| FormatError::Corrupt)?;
    ChaCha20Poly1305::new(&Key::from(*key.kek))
        .decrypt_inout_detached(
            &Nonce::from(nonce),
            &wrap_aad(header),
            file_key.as_mut_slice().into(),
            &tag,
        )
        .map_err(|_| FormatError::Corrupt)?;
    Ok(file_key)
}

fn wrap_aad(header: &[u8; HEADER_BYTES]) -> [u8; PREFIX_BYTES + WRAP_LABEL.len()] {
    let mut aad = [0; PREFIX_BYTES + WRAP_LABEL.len()];
    aad[..PREFIX_BYTES].copy_from_slice(&header[..PREFIX_BYTES]);
    aad[PREFIX_BYTES..].copy_from_slice(WRAP_LABEL);
    aad
}

fn random<const N: usize>() -> Result<[u8; N], FormatError> {
    let mut bytes = [0; N];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| FormatError::Random)?;
    Ok(bytes)
}

/// Reads until `buffer` is full or the input ends; returns the bytes read.
fn read_full(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Passes bytes through while hashing and counting them.
struct HashingWriter<W> {
    inner: W,
    hasher: Sha256,
    bytes: u64,
}

impl<W: Write> HashingWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.inner.write_all(bytes)?;
        self.hasher.update(bytes);
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    fn finish(self, plaintext_bytes: u64) -> Written {
        Written {
            ciphertext_sha256: hex::encode(self.hasher.finalize()),
            ciphertext_bytes: self.bytes,
            plaintext_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static NOT_CANCELLED: AtomicBool = AtomicBool::new(false);

    fn installation_key() -> DataKey {
        DataKey::from_bytes(std::array::from_fn(|i| i as u8))
    }

    /// The test vector printed in `backup-and-export-formats.md` (`vtbk1`).
    fn vector() -> Vec<u8> {
        let file_key: [u8; 32] = std::array::from_fn(|i| 0x20 + i as u8);
        let file = FileKeys {
            file_key: &file_key,
            wrap_nonce: std::array::from_fn(|i| 0x40 + i as u8),
        };
        let key = BackupKey::derive(&installation_key());
        let mut out = Vec::new();
        let plaintext = b"vibe-tox synthetic test vector\n".as_slice();
        encrypt_with(
            &key,
            Content::Manifest,
            &file,
            plaintext,
            &mut out,
            &NOT_CANCELLED,
        )
        .unwrap();
        out
    }

    #[test]
    fn known_answer_vector_matches_the_format_document() {
        let key = BackupKey::derive(&installation_key());
        assert_eq!(key.key_id().to_string(), "548b627d52b0809d7630bae666610381");
        assert_eq!(
            hex::encode(vector()),
            "5654424b01030000548b627d52b0809d7630bae666610381404142434445464748494a4b\
             372e52e284492139d13a9aabb6e832146fd64210da5976231150b4ac7dfb43c42913c1b8\
             c8b55132e4ac611a38d12b9700100000253eb8d33b71a8769a738db058cf6aecb94da1be\
             9dcd12efa9c9c2abc27203be5f21795acff9c35c23595d98f36398"
        );
    }

    #[test]
    fn a_final_chunk_without_the_final_flag_is_refused() {
        let key = BackupKey::derive(&installation_key());
        let mut file = vector();
        let header: [u8; HEADER_BYTES] = file[..HEADER_BYTES].try_into().unwrap();
        let file_key = open_header(&key, Content::Manifest, &header).unwrap();
        let body = &mut file[HEADER_BYTES..];
        let message_bytes = body.len() - TAG_BYTES;
        let cipher = ChaCha20Poly1305::new(&Key::from(*file_key));
        let (message, tag) = body.split_at_mut(message_bytes);
        let tag_value = Tag::try_from(&*tag).unwrap();
        cipher
            .decrypt_inout_detached(&chunk_nonce(0, true), &header, message.into(), &tag_value)
            .unwrap();
        let flagless = cipher
            .encrypt_inout_detached(&chunk_nonce(0, false), &header, message.into())
            .unwrap();
        tag.copy_from_slice(&flagless);
        let result = decrypt(
            &key,
            Content::Manifest,
            file.as_slice(),
            Vec::new(),
            &NOT_CANCELLED,
        );
        assert!(matches!(result, Err(FormatError::Corrupt)), "{result:?}");
    }

    #[test]
    fn the_backup_key_is_redacted_and_separate_from_the_installation_key() {
        let key = BackupKey::derive(&installation_key());
        assert!(!format!("{key:?}").contains(&hex::encode(*key.kek)));
        assert_ne!(*key.kek, *installation_key().expose());
        assert_eq!(encrypted_len(0), 88 + 16);
        assert_eq!(encrypted_len(CHUNK_BYTES as u64), 88 + (1 << 20) + 32);
    }
}
