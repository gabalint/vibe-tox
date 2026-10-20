//! Restore tests (REQ-OPS-007, NFR-30; M4 design brief §3.6, exit scenario 2): a fresh
//! backup passes with its lost interval counted, and each finding code appears for its
//! injected fault. The expected codes come from §3.6, not from the implementation.

use std::fs;
use std::path::Path;

use lis_auth::DataKey;
use lis_ops::backup::{Manifest, read_manifest};
use lis_ops::format::{BackupKey, Content};
use lis_ops::restore::{FindingCode, RestoreTestReport, TestState, list_restore_tests};
use lis_ops::targets::{self, Target, TargetKind};
use lis_store::ArtifactDigest;
use rusqlite::Connection;

use crate::support::{DEMO, Fixture, MODULES, backed_up, decrypted, encrypted, set_dir};

fn codes(report: &RestoreTestReport) -> Vec<FindingCode> {
    report
        .findings
        .list()
        .iter()
        .map(|finding| finding.code)
        .collect()
}

fn assert_found(report: &RestoreTestReport, code: FindingCode) {
    assert_eq!(report.state, TestState::Failed, "{:?}", codes(report));
    assert!(
        report.findings.count(code) >= 1,
        "{code:?} not in {:?}",
        codes(report)
    );
}

#[test]
fn a_fresh_backup_passes_with_its_lost_interval_counted() {
    let (f, target, set_id) = backed_up();
    let report = f.restore_test(&target);
    assert_eq!(report.state, TestState::Passed, "{:?}", codes(&report));
    assert_eq!(codes(&report), [FindingCode::LostInterval]);

    let records = f
        .test
        .db
        .read_blocking(|conn| list_restore_tests(conn, 10))
        .unwrap();
    let record = &records[0];
    assert!(record.passed());
    assert_eq!(record.set_id.as_deref(), Some(set_id.as_str()));
    assert!(record.set_created_at.is_some() && record.elapsed_ms.is_some());
    let anchors = record.anchors.unwrap();
    assert!(anchors.matched >= 1, "{anchors:?}");
    assert!(anchors.lost_interval >= 1, "{anchors:?}");
    assert_eq!(anchors.mismatched + anchors.local_events_missing, 0);
    let scratch = f.test.data_dir().join("ops").join("restore-tests");
    assert_eq!(fs::read_dir(scratch).unwrap().count(), 0, "scratch removed");
    let completed = f.count(
        "SELECT count(*) FROM audit_events WHERE action = 'ops.restore_test' \
         AND principal_id = 'ops-jobs' AND outcome = 'succeeded'",
    );
    assert_eq!(completed, 1);
}

#[test]
fn a_truncated_database_is_a_digest_mismatch() {
    let (f, target, set_id) = backed_up();
    let file = set_dir(&target, &set_id).join("database.vtbk");
    let bytes = fs::read(&file).unwrap();
    fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
    assert_found(
        &f.restore_test(&target),
        FindingCode::DatabaseDigestMismatch,
    );
}

#[test]
fn a_flipped_artifact_bit_is_corrupt_and_a_deleted_artifact_missing() {
    let (f, target, _) = backed_up();
    let artifacts = targets::root(&target).join("artifacts");
    let mut files: Vec<_> = fs::read_dir(&artifacts)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    let mut flipped = fs::read(&files[0]).unwrap();
    flipped[100] ^= 0x01;
    fs::write(&files[0], flipped).unwrap();
    fs::remove_file(&files[1]).unwrap();
    let report = f.restore_test(&target);
    assert_found(&report, FindingCode::ArtifactCorrupt);
    assert_found(&report, FindingCode::ArtifactMissing);
}

#[test]
fn a_swapped_manifest_with_another_set_id_is_unreadable() {
    let (f, target, first) = backed_up();
    f.change();
    f.advance(20);
    let second = f.backup(&target);
    fs::copy(
        set_dir(&target, &first).join("manifest.vtbk"),
        set_dir(&target, &second).join("manifest.vtbk"),
    )
    .unwrap();
    assert_found(&f.restore_test(&target), FindingCode::ManifestUnreadable);
}

#[test]
fn another_key_is_a_key_mismatch_and_nothing_else() {
    let (f, target, _) = backed_up();
    let other = BackupKey::derive(&DataKey::from_bytes([9; 32]));
    let report = f.restore_test_with(&target, &other, &MODULES);
    assert_found(&report, FindingCode::KeyMismatch);
    assert_eq!(codes(&report), [FindingCode::KeyMismatch]);
}

#[test]
fn an_edited_anchor_is_a_mismatch() {
    let (f, target, _) = backed_up();
    let anchors = targets::root(&target).join("anchors");
    let day = fs::read_dir(&anchors)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut files: Vec<_> = fs::read_dir(day)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    // The first file holds seals the set covers.
    let text = fs::read_to_string(&files[0]).unwrap();
    let mut line: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    line["headHash"] = serde_json::Value::String("ab".repeat(32));
    let edited = format!("{}\n{text}", serde_json::to_string(&line).unwrap());
    fs::write(&files[0], edited).unwrap();
    assert_found(&f.restore_test(&target), FindingCode::AnchorMismatch);
}

#[test]
fn a_missing_seal_inside_an_anchor_lineage_fails_restore_verification() {
    let (f, target, _) = backed_up();
    let anchors = targets::root(&target).join("anchors");
    let mut files = Vec::new();
    for day in fs::read_dir(anchors).unwrap() {
        for entry in fs::read_dir(day.unwrap().path()).unwrap() {
            files.push(entry.unwrap().path());
        }
    }
    files.sort();
    assert!(files.len() >= 2, "the fixture exports two anchor passes");
    fs::remove_file(&files[0]).unwrap();

    assert_found(&f.restore_test(&target), FindingCode::AnchorGap);
}

#[test]
fn a_set_newer_than_the_app_is_refused_before_it_opens() {
    let (f, target, _) = backed_up();
    let older_build = [lis_ops::SCHEMA, DEMO];
    let report = f.restore_test_with(&target, &f.key, &older_build);
    assert_found(&report, FindingCode::SchemaNewerThanApp);
    assert_eq!(report.findings.count(FindingCode::OpenFailed), 0);
}

#[test]
fn a_reference_to_an_artifact_the_set_lacks_is_reported() {
    let f = Fixture::new();
    let target = f.target("usb", TargetKind::Removable);
    f.report(b"synthetic stored report");
    f.reference(&"ab".repeat(32));
    f.backup(&target);
    let report = f.restore_test(&target);
    assert_found(&report, FindingCode::ReferencedArtifactMissing);
    assert_eq!(
        report
            .findings
            .count(FindingCode::ReferencedArtifactMissing),
        1
    );
}

/// Replaces the set's database with an edited copy whose digest the manifest records,
/// so only the edit itself can be found.
fn craft(
    f: &Fixture,
    target: &Target,
    set_id: &str,
    edit_database: impl FnOnce(&Path),
    edit_manifest: impl FnOnce(&mut Manifest),
) {
    let dir = set_dir(target, set_id);
    let mut manifest = read_manifest(&dir, &f.key).unwrap();
    let work = tempfile::tempdir().unwrap();
    let plain = work.path().join("copy.sqlite3");
    fs::write(
        &plain,
        decrypted(&f.key, Content::Database, &dir.join("database.vtbk")),
    )
    .unwrap();
    edit_database(&plain);
    let bytes = fs::read(&plain).unwrap();
    let (digest, length) = ArtifactDigest::of_reader(bytes.as_slice()).unwrap();
    manifest.database.sha256 = digest.to_string();
    manifest.database.bytes = length;
    edit_manifest(&mut manifest);
    encrypted(
        &f.key,
        Content::Database,
        &dir.join("database.vtbk"),
        &bytes,
    );
    let manifest = serde_json::to_vec(&manifest).unwrap();
    encrypted(
        &f.key,
        Content::Manifest,
        &dir.join("manifest.vtbk"),
        &manifest,
    );
}

fn sql(path: &Path, statements: &str) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(statements).unwrap();
    conn.execute_batch("PRAGMA journal_mode = DELETE").unwrap();
}

fn crafted(
    edit_database: impl FnOnce(&Path),
    edit_manifest: impl FnOnce(&mut Manifest),
) -> RestoreTestReport {
    let (f, target, set_id) = backed_up();
    craft(&f, &target, &set_id, edit_database, edit_manifest);
    f.restore_test(&target)
}

#[test]
fn a_damaged_database_fails_its_integrity_check() {
    let report = crafted(
        |path| {
            let mut bytes = fs::read(path).unwrap();
            let page = 4096 * 3;
            bytes[page..page + 4096].fill(0xff);
            fs::write(path, bytes).unwrap();
        },
        |_| {},
    );
    assert_found(&report, FindingCode::IntegrityFailed);
}

#[test]
fn a_database_the_migrations_refuse_does_not_open() {
    let report = crafted(
        |path| {
            sql(
                path,
                "UPDATE store_migrations SET checksum = '00' WHERE module = 'demo'",
            );
        },
        |_| {},
    );
    assert_found(&report, FindingCode::OpenFailed);
}

#[test]
fn an_edited_event_breaks_the_chain() {
    let report = crafted(
        |path| {
            sql(
                path,
                "DROP TRIGGER audit_events_no_update; \
                 UPDATE audit_events SET reason = 'edited' WHERE stream = 'clinical' AND seq = 1;",
            );
        },
        |_| {},
    );
    assert_found(&report, FindingCode::ChainBroken);
}

#[test]
fn heads_and_seals_must_equal_the_manifest() {
    let report = crafted(
        |_| {},
        |manifest| {
            manifest.heads[0].seq += 1;
            manifest.seals[0].last_seq += 1;
        },
    );
    assert_found(&report, FindingCode::HeadMismatch);
    assert_found(&report, FindingCode::SealInconsistent);
}

#[test]
fn a_set_lacking_anchored_events_it_should_hold_is_reported() {
    let report = crafted(
        |path| {
            sql(
                path,
                "DROP TRIGGER audit_events_no_delete; \
                 DELETE FROM audit_events WHERE stream = 'clinical' \
                 AND seq = (SELECT max(seq) FROM audit_events WHERE stream = 'clinical');",
            );
        },
        |_| {},
    );
    assert_found(&report, FindingCode::LocalEventsMissing);
}

#[test]
fn an_unavailable_target_fails_the_test_without_findings() {
    let (f, target, _) = backed_up();
    fs::remove_file(targets::root(&target).join("target.json")).unwrap();
    let report = f.restore_test(&target);
    assert_eq!(report.state, TestState::Failed);
    assert_eq!(
        report.failure,
        Some(lis_ops::restore::TestFailure::TargetUnavailable)
    );
    assert!(report.findings.list().is_empty());
}
