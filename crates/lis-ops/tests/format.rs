//! `vtbk1` properties (M4 design brief §3.2): round trip, and refusal of truncation,
//! reordered chunks, a flipped bit, another key, an edited header and another content
//! kind. The known-answer vector and the missing final flag are unit tests in `format.rs`.

#![cfg(test)]

use std::sync::atomic::AtomicBool;

use lis_auth::DataKey;
use lis_ops::format::{
    BackupKey, CHUNK_BYTES, Content, FormatError, HEADER_BYTES, decrypt, encrypt, encrypted_len,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

const TAG_BYTES: usize = 16;

fn key(byte: u8) -> BackupKey {
    BackupKey::derive(&DataKey::from_bytes([byte; 32]))
}

/// `len` synthetic bytes from a cheap generator, so large inputs stay fast to build.
fn data(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

fn sealed(plaintext: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let written = encrypt(
        &key(1),
        Content::Database,
        plaintext,
        &mut out,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(written.ciphertext_bytes, out.len() as u64);
    assert_eq!(written.plaintext_bytes, plaintext.len() as u64);
    assert_eq!(written.ciphertext_sha256, hex::encode(Sha256::digest(&out)));
    assert_eq!(encrypted_len(plaintext.len() as u64), out.len() as u64);
    out
}

fn opened(key: &BackupKey, content: Content, file: &[u8]) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::new();
    decrypt(key, content, file, &mut out, &AtomicBool::new(false))?;
    Ok(out)
}

#[test]
fn round_trips_empty_and_chunk_boundary_inputs() {
    for len in [
        0,
        1,
        CHUNK_BYTES - 1,
        CHUNK_BYTES,
        CHUNK_BYTES + 1,
        2 * CHUNK_BYTES,
    ] {
        let plaintext = data(len, len as u64);
        let file = sealed(&plaintext);
        assert_eq!(
            opened(&key(1), Content::Database, &file).unwrap(),
            plaintext
        );
    }
}

#[test]
fn every_file_gets_its_own_file_key() {
    assert_ne!(sealed(b"same plaintext"), sealed(b"same plaintext"));
}

#[test]
fn reordered_chunks_are_refused() {
    let file = sealed(&data(2 * CHUNK_BYTES + 10, 7));
    let chunk = CHUNK_BYTES + TAG_BYTES;
    let mut swapped = file[..HEADER_BYTES].to_vec();
    swapped.extend_from_slice(&file[HEADER_BYTES + chunk..HEADER_BYTES + 2 * chunk]);
    swapped.extend_from_slice(&file[HEADER_BYTES..HEADER_BYTES + chunk]);
    swapped.extend_from_slice(&file[HEADER_BYTES + 2 * chunk..]);
    assert!(matches!(
        opened(&key(1), Content::Database, &swapped),
        Err(FormatError::Corrupt)
    ));
}

#[test]
fn a_dropped_final_chunk_is_refused() {
    let file = sealed(&data(CHUNK_BYTES + 10, 3));
    let without_final = &file[..HEADER_BYTES + CHUNK_BYTES + TAG_BYTES];
    assert!(matches!(
        opened(&key(1), Content::Database, without_final),
        Err(FormatError::Corrupt)
    ));
}

#[test]
fn a_chunk_from_another_file_is_refused() {
    let first = sealed(&data(100, 1));
    let second = sealed(&data(100, 1));
    let mut spliced = first[..HEADER_BYTES].to_vec();
    spliced.extend_from_slice(&second[HEADER_BYTES..]);
    assert!(matches!(
        opened(&key(1), Content::Database, &spliced),
        Err(FormatError::Corrupt)
    ));
}

#[test]
fn another_key_or_content_kind_is_refused_before_decryption() {
    let file = sealed(b"synthetic snapshot bytes");
    assert!(matches!(
        opened(&key(2), Content::Database, &file),
        Err(FormatError::KeyMismatch)
    ));
    assert!(matches!(
        opened(&key(1), Content::Manifest, &file),
        Err(FormatError::WrongContent)
    ));
    assert!(matches!(
        opened(&key(1), Content::Database, b"not a backup"),
        Err(FormatError::NotVtbk)
    ));
}

/// The variant name of a refusal.
fn refusal(result: Result<Vec<u8>, FormatError>) -> &'static str {
    match result {
        Ok(_) => "accepted",
        Err(FormatError::NotVtbk) => "not_vtbk",
        Err(FormatError::WrongContent) => "wrong_content",
        Err(FormatError::KeyMismatch) => "key_mismatch",
        Err(FormatError::Corrupt) => "corrupt",
        Err(_) => "other",
    }
}

#[test]
fn edited_header_fields_are_refused() {
    let file = sealed(b"synthetic snapshot bytes");
    for (offset, field, expected) in [
        (0, "magic", "not_vtbk"),
        (4, "version", "not_vtbk"),
        (5, "content", "wrong_content"),
        (6, "reserved", "not_vtbk"),
        (8, "key ID", "key_mismatch"),
        (24, "wrap nonce", "corrupt"),
        (40, "wrapped key", "corrupt"),
        (87, "chunk size", "not_vtbk"),
    ] {
        let mut edited = file.clone();
        edited[offset] ^= 0x01;
        let result = opened(&key(1), Content::Database, &edited);
        assert_eq!(refusal(result), expected, "{field}");
    }
}

#[test]
fn a_raised_cancellation_flag_stops_both_directions() {
    let cancelled = AtomicBool::new(true);
    let result = encrypt(
        &key(1),
        Content::Database,
        &b"x"[..],
        Vec::new(),
        &cancelled,
    );
    assert!(matches!(result, Err(FormatError::Cancelled)));
    let file = sealed(b"x");
    let result = decrypt(
        &key(1),
        Content::Database,
        &file[..],
        Vec::new(),
        &cancelled,
    );
    assert!(matches!(result, Err(FormatError::Cancelled)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn round_trips_any_input(len in 0..(2 * CHUNK_BYTES + 64), seed: u64) {
        let plaintext = data(len, seed);
        let file = sealed(&plaintext);
        prop_assert_eq!(opened(&key(1), Content::Database, &file).unwrap(), plaintext);
    }

    #[test]
    fn any_truncation_is_refused(len in 0..(CHUNK_BYTES + 64), seed: u64, cut: prop::sample::Index) {
        let file = sealed(&data(len, seed));
        let kept = cut.index(file.len());
        prop_assert!(opened(&key(1), Content::Database, &file[..kept]).is_err());
    }

    #[test]
    fn any_flipped_bit_is_refused(
        len in 0..(CHUNK_BYTES + 64),
        seed: u64,
        at: prop::sample::Index,
        bit in 0_u8..8,
    ) {
        let mut file = sealed(&data(len, seed));
        let index = at.index(file.len());
        file[index] ^= 1 << bit;
        prop_assert!(opened(&key(1), Content::Database, &file).is_err());
    }

    #[test]
    fn appended_bytes_are_refused(len in 0..4096_usize, extra in 1..64_usize, seed: u64) {
        let mut file = sealed(&data(len, seed));
        file.extend(data(extra, seed ^ 1));
        prop_assert!(opened(&key(1), Content::Database, &file).is_err());
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        prop_assert!(opened(&key(1), Content::Database, &bytes).is_err());
    }
}
