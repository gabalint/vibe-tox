//! Content-addressed artifacts and `VACUUM INTO` snapshots.

use std::fs;

use lis_core::{Capability, CorrelationId};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{ARTIFACTS_DIR, ArtifactDigest, ArtifactStore, StoreError};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::DEMO;

const REPORT: &[u8] = b"%PDF-1.7 synthetic report bytes";

#[test]
fn put_then_read_round_trips_and_repeated_put_reuses_the_file() {
    let dir = TempDir::new().unwrap();
    let store = ArtifactStore::open(dir.path(), 1024).unwrap();
    let stored = store.put(REPORT).unwrap();
    assert_eq!(
        stored.digest.to_string(),
        hex::encode(Sha256::digest(REPORT))
    );
    assert_eq!(stored.size, REPORT.len() as u64);
    assert_eq!(store.read(&stored.digest).unwrap(), REPORT);
    assert_eq!(store.put(REPORT).unwrap(), stored);
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        names,
        [stored.digest.to_string()],
        "no temporary files remain"
    );
}

#[test]
fn tampered_artifact_is_reported_and_never_served_or_replaced() {
    let dir = TempDir::new().unwrap();
    let store = ArtifactStore::open(dir.path(), 1024).unwrap();
    let stored = store.put(REPORT).unwrap();
    let path = dir.path().join(stored.digest.to_string());
    fs::write(&path, b"%PDF-1.7 replaced bytes").unwrap();
    assert!(matches!(
        store.read(&stored.digest),
        Err(StoreError::ArtifactCorrupt)
    ));
    assert!(matches!(
        store.put(REPORT),
        Err(StoreError::ArtifactCorrupt)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"%PDF-1.7 replaced bytes");
}

#[test]
fn reopening_removes_temporary_files_left_by_an_interrupted_put() {
    let dir = TempDir::new().unwrap();
    let digest = hex::encode(Sha256::digest(REPORT));
    // What a crash between create and rename leaves behind.
    fs::write(dir.path().join(format!(".tmp-{digest}-0")), &REPORT[..5]).unwrap();
    let store = ArtifactStore::open(dir.path(), 1024).unwrap();
    let stored = store.put(REPORT).unwrap();
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, [stored.digest.to_string()]);
}

#[test]
fn oversized_artifacts_are_refused() {
    let dir = TempDir::new().unwrap();
    let store = ArtifactStore::open(dir.path(), 8).unwrap();
    assert!(store.put(b"12345678").is_ok());
    assert!(matches!(
        store.put(b"123456789"),
        Err(StoreError::ArtifactTooLarge)
    ));
}

#[test]
fn unknown_digest_is_not_found() {
    let dir = TempDir::new().unwrap();
    let store = ArtifactStore::open(dir.path(), 1024).unwrap();
    let digest: ArtifactDigest = hex::encode(Sha256::digest(b"absent")).parse().unwrap();
    assert!(matches!(
        store.read(&digest),
        Err(StoreError::ArtifactNotFound)
    ));
}

#[test]
fn digest_parsing_rejects_unsafe_or_malformed_names() {
    let upper = hex::encode(Sha256::digest(REPORT)).to_uppercase();
    let short = "a".repeat(63);
    for text in ["", "../../etc/passwd", upper.as_str(), short.as_str()] {
        assert!(
            matches!(
                text.parse::<ArtifactDigest>(),
                Err(StoreError::InvalidDigest)
            ),
            "{text}"
        );
    }
}

#[test]
fn database_keeps_artifacts_in_the_data_directory() {
    let t = TestDatabase::open(&[]).unwrap();
    let stored = t.db.artifacts().put(REPORT).unwrap();
    let path = t
        .data_dir()
        .join(ARTIFACTS_DIR)
        .join(stored.digest.to_string());
    assert_eq!(fs::read(path).unwrap(), REPORT);
}

#[test]
fn snapshot_is_a_verified_consistent_copy() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    t.db.write_blocking(
        principal(&[Capability::Accession]),
        CorrelationId::generate(),
        |ctx| {
            ctx.require(Capability::Accession)?;
            for n in 0..10 {
                ctx.tx().execute(
                    "INSERT INTO demo_items (label) VALUES (?1)",
                    [format!("item-{n}")],
                )?;
            }
            Ok::<_, StoreError>(())
        },
    )
    .unwrap();
    let out = TempDir::new().unwrap();
    let path = out.path().join("snapshot.sqlite3");
    let snapshot = t.db.snapshot(&path).unwrap();
    assert!(snapshot.is_intact(), "{:?}", snapshot.integrity_check);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        snapshot.digest.to_string(),
        hex::encode(Sha256::digest(&bytes))
    );
    assert_eq!(snapshot.size, bytes.len() as u64);
    let names: Vec<_> = fs::read_dir(out.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        names,
        ["snapshot.sqlite3"],
        "no WAL or journal files beside the snapshot"
    );
    let copy = Connection::open(&path).unwrap();
    let rows: i64 = copy
        .query_row("SELECT count(*) FROM demo_items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 10);
}

#[test]
fn snapshot_refuses_an_existing_destination() {
    let t = TestDatabase::open(&[]).unwrap();
    let out = TempDir::new().unwrap();
    let path = out.path().join("taken.sqlite3");
    fs::write(&path, b"not empty").unwrap();
    assert!(t.db.snapshot(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"not empty");
}

#[test]
fn digests_lists_artifacts_in_order_and_reports_every_other_entry() {
    let dir = TempDir::new().unwrap();
    let store = ArtifactStore::open(dir.path(), 1024).unwrap();
    assert_eq!(store.digests().unwrap().digests, []);
    let mut expected: Vec<ArtifactDigest> = [b"first".as_slice(), b"second", b"third"]
        .iter()
        .map(|bytes| store.put(bytes).unwrap().digest)
        .collect();
    expected.sort_by_key(ToString::to_string);
    // Not a digest: too short. (Case variants are not used: on a
    // case-insensitive file system they name an existing artifact.)
    let short = expected[0].to_string()[..63].to_owned();
    let folder = hex::encode(Sha256::digest(b"a folder"));
    fs::write(dir.path().join(".tmp-in-progress"), b"partial").unwrap();
    fs::write(dir.path().join("notes.txt"), b"copied in by hand").unwrap();
    fs::write(dir.path().join(&short), b"truncated name").unwrap();
    fs::create_dir(dir.path().join(&folder)).unwrap();

    let listing = store.digests().unwrap();
    assert_eq!(listing.digests, expected);
    let mut unexpected = vec![folder, short, "notes.txt".to_owned()];
    unexpected.sort();
    assert_eq!(listing.unexpected, unexpected);
}
