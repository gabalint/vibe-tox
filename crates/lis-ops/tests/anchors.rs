//! Audit anchors on real temporary folders and SQLite files (NFR-18, ADR 0008,
//! REQ-OPS-008; M4 design brief §4, §10 exit scenario 3 without the engine). Nothing is
//! stubbed: seals, anchor lines, hashing, chain verification and anchor comparison are the
//! real ones; the expected findings come from the brief's verification table.

#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lis_audit::{AuditEvent, ChainLink, Outcome, Seal, Stream};
use lis_auth::DataKey;
use lis_core::{
    Capability, Clock, CorrelationId, ErrorCode, FixedClock, InstallationId, Principal,
};
use lis_ops::anchors::{
    self, AnchorError, AnchorVerification, EXPORT_ANCHORS, ExportReport, RestoreRecord,
};
use lis_ops::format::BackupKey;
use lis_ops::targets::{self, NewTarget, Target, TargetKind, TargetPurpose};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{DATABASE_FILE, Database, StoreError};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const MODULES: [lis_core::ModuleSchema; 2] = [lis_ops::SCHEMA, lis_ops::ANCHORS_SCHEMA];
/// The keys an anchor line may hold (backup-and-export-formats.md, Line format).
const ALLOWED_KEYS: [&str; 10] = [
    "format",
    "installation",
    "stream",
    "seal",
    "firstSeq",
    "lastSeq",
    "prevHash",
    "headHash",
    "sealedAt",
    "hashAlg",
];

struct Fixture {
    test: TestDatabase,
    folders: TempDir,
}

fn jobs() -> Principal {
    Principal::system("ops-jobs", [])
}

fn admin() -> Principal {
    principal(&[Capability::BackupRestore])
}

fn key() -> BackupKey {
    BackupKey::derive(&DataKey::from_bytes([7; 32]))
}

impl Fixture {
    fn new() -> Self {
        let test = TestDatabase::open(&MODULES).unwrap();
        test.db
            .write_blocking(
                jobs(),
                CorrelationId::generate(),
                lis_ops::ensure_installation,
            )
            .unwrap();
        Self {
            test,
            folders: tempfile::tempdir().unwrap(),
        }
    }

    fn db(&self) -> &Database {
        &self.test.db
    }

    fn target(&self, name: &str, kind: TargetKind, purpose: TargetPurpose) -> Target {
        add_target(
            self.db(),
            self.test.data_dir(),
            &self.folders.path().join(name),
            kind,
            purpose,
        )
    }

    fn change(&self, stream: Stream, reason: &str) {
        audit(self.db(), stream, reason);
    }

    fn export(&self) -> ExportReport {
        anchors::export_pending(self.db(), jobs()).unwrap()
    }

    fn advance(&self, minutes: u64) {
        let now = self.test.clock.now();
        self.test
            .clock
            .set(now.checked_add(Duration::from_secs(minutes * 60)).unwrap());
    }

    fn status(&self) -> anchors::AnchorStatusView {
        let now = self.test.clock.now();
        self.db()
            .read_blocking(move |conn| anchors::get_anchor_status(conn, now))
            .unwrap()
    }

    fn raw(&self) -> Connection {
        raw(self.test.data_dir())
    }
}

fn add_target(
    db: &Database,
    data_dir: &Path,
    folder: &Path,
    kind: TargetKind,
    purpose: TargetPurpose,
) -> Target {
    fs::create_dir_all(folder).unwrap();
    let data_dir = data_dir.to_path_buf();
    let input = NewTarget {
        label: "Synthetic target".to_owned(),
        kind,
        purpose,
        folder: folder.to_path_buf(),
    };
    db.write_blocking(admin(), CorrelationId::generate(), move |ctx| {
        targets::add_target(ctx, &data_dir, key().key_id(), &input)
    })
    .unwrap()
}

fn audit(db: &Database, stream: Stream, reason: &str) -> ChainLink {
    let reason = reason.to_owned();
    db.write_blocking(jobs(), CorrelationId::generate(), move |ctx| {
        ctx.audit(AuditEvent::new(stream, "test.change", Outcome::Succeeded).reason(reason))
    })
    .unwrap()
}

/// A second connection to the database file, for edits the application never makes.
fn raw(data_dir: &Path) -> Connection {
    let conn = Connection::open(data_dir.join(DATABASE_FILE)).unwrap();
    conn.busy_timeout(Duration::from_secs(5)).unwrap();
    conn
}

/// Every anchor file under the target, in name order, with its bytes.
fn anchor_files(target: &Target) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let anchors = targets::root(target).join("anchors");
    let Ok(days) = fs::read_dir(&anchors) else {
        return files;
    };
    for day in days {
        for file in fs::read_dir(day.unwrap().path()).unwrap() {
            let path = file.unwrap().path();
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}

fn seals_in(files: &BTreeMap<PathBuf, Vec<u8>>) -> Vec<Seal> {
    files
        .values()
        .flat_map(|bytes| {
            String::from_utf8(bytes.clone())
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .map(|line| lis_audit::parse_line(&line).unwrap().seal)
        .collect()
}

fn verify(
    db: &Database,
    folders: Vec<PathBuf>,
    restores: Vec<RestoreRecord>,
) -> AnchorVerification {
    let read = anchors::read_anchors(&folders).unwrap();
    db.read_blocking(move |conn| anchors::verify_anchors(conn, &read, &restores))
        .unwrap()
}

fn codes(verification: &AnchorVerification) -> BTreeSet<String> {
    verification
        .findings
        .iter()
        .map(|finding| finding.code.clone())
        .collect()
}

fn move_folder(from: &Path, to: &Path) {
    fs::rename(from, to).unwrap();
}

/// Every file of `before` is still in `after`, byte for byte.
fn assert_kept(before: &BTreeMap<PathBuf, Vec<u8>>, after: &BTreeMap<PathBuf, Vec<u8>>) {
    for (path, bytes) in before {
        assert_eq!(after.get(path), Some(bytes), "an anchor file was changed");
    }
}

fn by_position(mut seals: Vec<Seal>) -> Vec<Seal> {
    seals.sort_by_key(|seal| (seal.stream.as_str(), seal.seal_no));
    seals
}

/// The target's files hold every local seal exactly once.
fn assert_every_seal_once(f: &Fixture, target: &Target) {
    let local = f
        .db()
        .read_blocking(|conn| -> Result<Vec<Seal>, StoreError> {
            let mut all = Vec::new();
            for stream in Stream::ALL {
                all.extend(lis_audit::seals(conn, stream, 0)?);
            }
            Ok(all)
        })
        .unwrap();
    let written = by_position(seals_in(&anchor_files(target)));
    assert_eq!(
        written,
        by_position(local),
        "each seal reaches the target once"
    );
}

#[test]
fn export_drains_in_order_to_every_target_and_never_rewrites_a_file() {
    let f = Fixture::new();
    let network = f.target(
        "network",
        TargetKind::NetworkShare,
        TargetPurpose::BackupAndAnchors,
    );
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.change(Stream::Clinical, "first");
    let first = f.export();
    assert_eq!((first.targets_written, first.targets_unavailable), (2, 0));
    assert_eq!(first.seals_written, 2, "one clinical and one security seal");

    let usb_folder = f.folders.path().join("usb");
    let away = f.folders.path().join("usb-away");
    move_folder(&usb_folder, &away);
    for _ in 0..2 {
        let before = anchor_files(&network);
        f.advance(15);
        f.change(Stream::Clinical, "while the drive is away");
        let report = f.export();
        assert_eq!((report.targets_written, report.targets_unavailable), (1, 1));
        let after = anchor_files(&network);
        assert_kept(&before, &after);
        assert_eq!(after.len(), before.len() + 1);
    }

    move_folder(&away, &usb_folder);
    let usb_before = anchor_files(&usb);
    assert_eq!(usb_before.len(), 1);
    f.advance(15);
    let back = f.export();
    assert_eq!((back.targets_written, back.targets_unavailable), (2, 0));
    let usb_after = anchor_files(&usb);
    assert_eq!(usb_after.len(), 2);
    assert_kept(&usb_before, &usb_after);
    assert_every_seal_once(&f, &network);
    assert_every_seal_once(&f, &usb);

    let (last_path, last_bytes) = usb_after.last_key_value().unwrap();
    let drained: Vec<i64> = seals_in(&BTreeMap::from([(last_path.clone(), last_bytes.clone())]))
        .iter()
        .filter(|seal| seal.stream == Stream::Clinical)
        .map(|seal| seal.seal_no)
        .collect();
    assert_eq!(drained, [2, 3], "the queued clinical seals drain in order");
    let exports = f
        .db()
        .read_blocking(|conn| anchors::list_anchor_exports(conn, 50))
        .unwrap();
    assert_eq!(exports.len(), 6);
    let first_page = f
        .db()
        .read_blocking(|conn| anchors::list_anchor_exports(conn, 1))
        .unwrap();
    assert_eq!(
        first_page.len(),
        2,
        "the extra row proves more exports exist"
    );
    let newest_usb = exports
        .iter()
        .find(|export| last_path.ends_with(&export.file_name))
        .unwrap();
    let clinical = newest_usb
        .ranges
        .iter()
        .find(|range| range.stream == "clinical")
        .unwrap();
    assert_eq!((clinical.first_seal_no, clinical.last_seal_no), (2, 3));
}

#[test]
fn anchor_files_follow_the_documented_layout_and_hold_only_allowed_keys() {
    let f = Fixture::new();
    let canary = "CANARY-SYNTHETIC-PATIENT-ZZ9";
    let disk = f.target(
        "disk",
        TargetKind::LocalDisk,
        TargetPurpose::BackupAndAnchors,
    );
    f.change(Stream::Clinical, canary);
    f.change(Stream::Access, canary);
    f.export();
    let files = anchor_files(&disk);
    assert_eq!(files.len(), 1);
    let (path, bytes) = files.into_iter().next().unwrap();
    let day = path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(day, "2026-01-02");
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("20260102T030405Z-") && name.ends_with(".jsonl"),
        "{name}"
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.ends_with('\n'));
    assert!(!text.contains(canary));
    assert!(!text.contains("Synthetic target"));
    assert!(!text.contains(f.folders.path().to_str().unwrap()));
    for line in text.lines() {
        let object: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(line).unwrap();
        let keys: Vec<&str> = object.keys().map(String::as_str).collect();
        let mut allowed = ALLOWED_KEYS.to_vec();
        allowed.sort_unstable();
        let mut keys = keys;
        keys.sort_unstable();
        assert_eq!(keys, allowed);
        lis_audit::parse_line(line).unwrap();
    }
    assert_eq!(text.lines().count(), 3, "one seal per stream with events");
    let leftovers = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension().unwrap() != "jsonl")
        .count();
    assert_eq!(leftovers, 0, "no temporary file is left behind");
}

#[test]
fn a_pass_records_one_event_with_counts_only() {
    let f = Fixture::new();
    f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.target(
        "share",
        TargetKind::NetworkShare,
        TargetPurpose::BackupAndAnchors,
    );
    f.export();
    let events: Vec<(String, String, String)> = f
        .db()
        .read_blocking(|conn| -> Result<_, StoreError> {
            let mut statement = conn.prepare(
                "SELECT principal_id, outcome, details FROM audit_events WHERE action = ?1",
            )?;
            let rows = statement
                .query_map([EXPORT_ANCHORS], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .unwrap();
    assert_eq!(events.len(), 1);
    let (principal_id, outcome, details) = &events[0];
    assert_eq!(
        (principal_id.as_str(), outcome.as_str()),
        ("ops-jobs", "succeeded")
    );
    let details: serde_json::Value = serde_json::from_str(details).unwrap();
    assert_eq!(
        details,
        serde_json::json!({"seals_written": 1, "targets_failed": 0, "targets_unavailable": 0,
            "targets_written": 2})
    );
}

#[test]
fn a_scheduled_pass_with_nothing_written_records_nothing() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    fs::remove_dir_all(targets::root(&usb)).unwrap();
    let report = f.export();
    assert_eq!(
        report,
        ExportReport {
            targets_unavailable: 1,
            ..ExportReport::default()
        }
    );
    let count: i64 = f
        .raw()
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action = ?1",
            [EXPORT_ANCHORS],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn a_manual_export_needs_backup_restore_and_an_available_target() {
    let f = Fixture::new();
    let denied = anchors::export_pending(f.db(), principal(&[Capability::ViewAudit])).unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);

    let none = anchors::export_pending(f.db(), admin()).unwrap_err();
    assert!(matches!(none, AnchorError::NoTargetAvailable), "{none:?}");

    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    let user = admin();
    let user_id = user.actor().id_text();
    let report = anchors::export_pending(f.db(), user).unwrap();
    assert_eq!(report.targets_written, 1);
    let (kind, by): (String, String) = f
        .raw()
        .query_row(
            "SELECT kind, exported_by FROM ops_anchor_exports",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((kind.as_str(), by), ("manual", user_id.clone()));
    let principal_id: String = f
        .raw()
        .query_row(
            "SELECT principal_id FROM audit_events WHERE action = ?1",
            [EXPORT_ANCHORS],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(principal_id, user_id);

    fs::remove_dir_all(targets::root(&usb)).unwrap();
    let away = anchors::export_pending(f.db(), admin()).unwrap_err();
    assert!(matches!(away, AnchorError::NoTargetAvailable));
}

#[test]
fn a_seal_refused_as_a_line_fails_the_export_and_is_never_skipped() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.change(Stream::Clinical, "sealed");
    f.db()
        .write_blocking(
            jobs(),
            CorrelationId::generate(),
            |ctx| -> Result<_, StoreError> { Ok(lis_audit::seal(ctx.tx(), ctx.now())?) },
        )
        .unwrap();
    let raw = f.raw();
    raw.execute_batch(
        "DROP TRIGGER audit_seals_no_update;
         UPDATE audit_seals SET head_hash = replace(hex(zeroblob(32)), '0', 'z')
         WHERE stream = 'clinical' AND seal_no = 1;",
    )
    .unwrap();
    f.change(Stream::Clinical, "later");
    for _ in 0..2 {
        let report = f.export();
        assert_eq!((report.targets_written, report.targets_failed), (0, 1));
        assert!(anchor_files(&usb).is_empty());
    }
    let outcome: String = raw
        .query_row(
            "SELECT outcome FROM audit_events WHERE action = ?1 ORDER BY seq DESC LIMIT 1",
            [EXPORT_ANCHORS],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "failed");
}

#[test]
fn the_store_refuses_ranges_out_of_order_and_any_change() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.export();
    let raw = f.raw();
    let export_id: String = raw
        .query_row("SELECT id FROM ops_anchor_exports", [], |row| row.get(0))
        .unwrap();
    let skip = raw.execute_batch(&format!(
        "INSERT INTO ops_anchor_exports (id, target_id, file_name, exported_at, kind) \
         VALUES ('01900000-0000-7000-8000-0000000000ff', '{}', 'x.jsonl', '{}', 'scheduled'); \
         INSERT INTO ops_anchor_export_ranges VALUES \
         ('01900000-0000-7000-8000-0000000000ff', 'security', 3, 3);",
        usb.id,
        lis_store::testing::TEST_START,
    ));
    assert!(skip.unwrap_err().to_string().contains("continue"));
    for sql in [
        "UPDATE ops_anchor_exports SET file_name = 'y.jsonl'",
        "DELETE FROM ops_anchor_exports",
        "UPDATE ops_anchor_export_ranges SET last_seal_no = last_seal_no + 1",
        "DELETE FROM ops_anchor_export_ranges",
    ] {
        let error = raw.execute(sql, []).unwrap_err().to_string();
        assert!(error.contains("append-only"), "{sql}: {error}");
    }
    let still: i64 = raw
        .query_row(
            "SELECT count(*) FROM ops_anchor_export_ranges WHERE export_id = ?1",
            [&export_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(still, 1);
}

#[test]
fn lag_rises_while_anchoring_stops_and_falls_after_export() {
    let f = Fixture::new();
    f.target(
        "disk",
        TargetKind::LocalDisk,
        TargetPurpose::BackupAndAnchors,
    );
    f.change(Stream::Clinical, "before any off-host target");
    f.export();
    let local_only = f.status();
    assert_eq!(local_only.off_host_targets, 0);
    assert_eq!(local_only.health_facts().offhost_targets, 0);
    assert_eq!(
        local_only.streams[0].anchored_through_seq, 0,
        "a local disk is not off-host"
    );

    f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.export();
    let fresh = f.status();
    assert_eq!(fresh.off_host_targets, 1);
    assert_eq!(
        fresh.lag_minutes,
        Some(0),
        "only the pass's own event is unanchored"
    );
    assert_eq!(
        fresh.streams[0].anchored_through_seq,
        fresh.streams[0].head_seq
    );
    assert!(fresh.last_export.is_some());

    f.advance(45);
    f.change(Stream::Clinical, "while anchoring is stopped");
    f.advance(15);
    let stale = f.status();
    assert_eq!(stale.lag_minutes, Some(60));
    assert_eq!(stale.health_facts().lag, Some(Duration::from_secs(3600)));
    let clinical = &stale.streams[0];
    assert_eq!(clinical.stream, "clinical");
    assert!(clinical.head_seq > clinical.anchored_through_seq);
    assert_eq!(clinical.sealed_through_seq, clinical.anchored_through_seq);

    f.export();
    let caught_up = f.status();
    assert_eq!(caught_up.lag_minutes, Some(0));
    assert_eq!(caught_up.streams[0].oldest_unanchored_at, None);
}

#[test]
fn verification_reports_a_foreign_installation_and_invalid_lines_without_comparing_them() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.change(Stream::Clinical, "anchored");
    f.export();
    let own = seals_in(&anchor_files(&usb));
    let foreign_line = lis_audit::to_line(&own[0], InstallationId::generate()).unwrap();
    let other = f.folders.path().join("other");
    fs::create_dir_all(other.join("nested")).unwrap();
    fs::write(
        other.join("nested/foreign.jsonl"),
        format!("{foreign_line}\n"),
    )
    .unwrap();
    fs::write(
        other.join("broken.jsonl"),
        "{\"format\":\"vibe-tox-anchor-1\"}\n",
    )
    .unwrap();
    fs::write(other.join("ignored.txt"), "not an anchor file").unwrap();

    let result = verify(
        f.db(),
        vec![f.folders.path().join("usb"), other],
        Vec::new(),
    );
    assert_eq!(result.anchors_read, own.len() as u64 + 1);
    assert_eq!(result.summary.compared, own.len() as u64);
    assert_eq!(result.summary.matched, own.len() as u64);
    assert!(result.chains.iter().all(|chain| chain.intact));
    let foreign: Vec<_> = result
        .findings
        .iter()
        .filter(|finding| finding.code == "foreign_installation")
        .collect();
    assert_eq!(foreign.len(), 1);
    assert!(!foreign[0].problem);
    assert!(codes(&result).contains("invalid_anchor_line"));
    assert_eq!(
        result.problems(),
        1,
        "only the invalid line needs investigation"
    );

    let missing = anchors::read_anchors(&[f.folders.path().join("absent")]).unwrap_err();
    assert!(matches!(missing, AnchorError::FolderUnreadable));
}

#[test]
fn an_edited_event_gives_anchor_mismatch() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    for n in 0..3 {
        f.change(Stream::Clinical, &format!("change {n}"));
    }
    f.export();
    f.raw()
        .execute_batch(
            "DROP TRIGGER audit_events_no_update;
             UPDATE audit_events SET reason = 'edited' WHERE stream = 'clinical' AND seq = 2;",
        )
        .unwrap();
    let result = verify(f.db(), vec![targets::root(&usb)], Vec::new());
    assert_eq!(result.summary.mismatched, 1);
    assert!(codes(&result).contains("anchor_mismatch"));
    assert!(!result.chains[0].intact);
    assert!(result.problems() >= 2);
}

#[test]
fn a_recomputed_chain_passes_local_verification_but_not_the_anchors() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    for n in 0..3 {
        f.change(Stream::Clinical, &format!("change {n}"));
    }
    f.export();
    let raw = f.raw();
    raw.execute_batch(
        "DROP TRIGGER audit_events_no_update; DROP TRIGGER audit_seals_no_update;
         UPDATE audit_events SET reason = 'edited' WHERE stream = 'clinical' AND seq = 2;",
    )
    .unwrap();
    recompute_chain(&raw, "clinical");
    let intact = f
        .db()
        .read_blocking(|conn| -> Result<_, StoreError> {
            Ok(lis_audit::verify(conn, Stream::Clinical)?)
        })
        .unwrap();
    assert!(
        intact.is_intact(),
        "the rewrite fools local verification: {:?}",
        intact.findings
    );
    let result = verify(f.db(), vec![targets::root(&usb)], Vec::new());
    assert_eq!(result.summary.mismatched, 1);
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.code == "anchor_mismatch"
                && finding.stream.as_deref() == Some("clinical"))
    );
}

#[test]
fn a_truncated_tail_gives_local_events_missing() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.change(Stream::Clinical, "kept");
    f.export();
    f.change(Stream::Clinical, "deleted later");
    f.export();
    f.raw()
        .execute_batch(
            "DROP TRIGGER audit_events_no_delete; DROP TRIGGER audit_seals_no_delete;
             DELETE FROM audit_events WHERE stream = 'clinical' AND seq = 2;
             DELETE FROM audit_seals WHERE stream = 'clinical' AND seal_no = 2;",
        )
        .unwrap();
    let result = verify(f.db(), vec![targets::root(&usb)], Vec::new());
    assert_eq!(result.summary.local_events_missing, 1);
    assert!(
        result.chains[0].intact,
        "the shortened chain is intact on its own"
    );
    assert!(result.problems() >= 1);
}

/// Exit scenario 3 (VAL-40 subset): an older set restored while newer anchors exist. The
/// newer anchors are a lost interval once the restore is anchored, an unverified boundary
/// before, and plain findings without the restore record; later anchors match.
#[test]
fn a_restore_of_an_older_set_gives_lost_interval_and_later_anchors_match() {
    let f = Fixture::new();
    let usb = f.target("usb", TargetKind::Removable, TargetPurpose::AnchorsOnly);
    f.change(Stream::Clinical, "in the set");
    f.export();
    let restored_heads = f
        .db()
        .read_blocking(|conn| -> Result<_, StoreError> { Ok(lis_audit::heads(conn)?) })
        .unwrap();
    let set = f.folders.path().join("set.sqlite3");
    assert!(f.db().snapshot(&set).unwrap().is_intact());

    f.advance(15);
    f.change(Stream::Clinical, "lost with the restore");
    f.change(Stream::Clinical, "also lost");
    f.export();

    f.advance(15);
    let replacement = f.folders.path().join("replacement");
    fs::create_dir_all(&replacement).unwrap();
    fs::copy(&set, replacement.join(DATABASE_FILE)).unwrap();
    let clock = Arc::new(FixedClock::new(f.test.clock.now()));
    let restored = Database::open(&replacement, &MODULES, clock.clone()).unwrap();
    let applied_event = restored
        .write_blocking(
            Principal::system("restore", []),
            CorrelationId::generate(),
            |ctx| {
                ctx.audit(AuditEvent::new(
                    Stream::Security,
                    "ops.restore_applied",
                    Outcome::Succeeded,
                ))
            },
        )
        .unwrap();
    let record = RestoreRecord {
        applied_at: clock.now(),
        restored_heads,
        applied_event,
    };
    let folders = vec![targets::root(&usb)];

    let unexplained = verify(&restored, folders.clone(), Vec::new());
    assert_eq!(unexplained.summary.local_events_missing, 1);
    assert!(unexplained.problems() >= 1);

    let boundary = verify(&restored, folders.clone(), vec![record.clone()]);
    assert_eq!(boundary.summary.unverified_restore_boundary, 1);
    assert_eq!(boundary.summary.lost_interval, 0);
    assert!(
        boundary
            .findings
            .iter()
            .any(|finding| finding.code == "unverified_restore_boundary" && finding.problem)
    );

    let report = anchors::export_pending(&restored, jobs()).unwrap();
    assert_eq!(
        report.targets_written, 1,
        "the target is the same folder after the restore"
    );
    audit(&restored, Stream::Clinical, "after the restore");
    clock.set(clock.now().checked_add(Duration::from_secs(900)).unwrap());
    anchors::export_pending(&restored, jobs()).unwrap();

    let explained = verify(&restored, folders, vec![record]);
    assert_eq!(explained.summary.lost_interval, 1);
    assert_eq!(
        explained.summary.conflicts, 0,
        "re-sealed numbers are explained"
    );
    assert_eq!(explained.problems(), 0, "{:?}", explained.findings);
    let later_clinical = explained.summary.matched;
    assert!(
        later_clinical >= 4,
        "anchors after the restore match: {later_clinical}"
    );
    assert!(explained.chains.iter().all(|chain| chain.intact));
}

/// Recomputes a stream's `sha256-v1` chain and its seals from the documented format
/// (backup-and-export-formats.md), as someone with file access could.
fn recompute_chain(conn: &Connection, stream: &str) {
    let columns = "hash_alg, stream, seq, prev_hash, occurred_at, recorded_at, principal_kind, \
        principal_id, session_id, workstation_id, action, outcome, record_type, record_id, \
        old_revision, new_revision, reason, authority, correlation_id, details";
    let rows: Vec<Vec<Option<String>>> = conn
        .prepare(&format!(
            "SELECT {columns} FROM audit_events WHERE stream = ?1 ORDER BY seq"
        ))
        .unwrap()
        .query_map([stream], |row| {
            (0..20)
                .map(|index| match index {
                    2 | 14 | 15 => Ok(row.get::<_, Option<i64>>(index)?.map(|v| v.to_string())),
                    _ => row.get::<_, Option<String>>(index),
                })
                .collect()
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let mut prev = "0".repeat(64);
    for mut row in rows {
        row[3] = Some(prev.clone());
        let mut bytes = Vec::new();
        for field in &row {
            match field {
                None => bytes.push(0),
                Some(text) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&u32::try_from(text.len()).unwrap().to_be_bytes());
                    bytes.extend_from_slice(text.as_bytes());
                }
            }
        }
        let hash = hex::encode(Sha256::digest(&bytes));
        conn.execute(
            "UPDATE audit_events SET prev_hash = ?1, hash = ?2 WHERE stream = ?3 AND seq = ?4",
            params![
                prev,
                hash,
                stream,
                row[2].as_deref().unwrap().parse::<i64>().unwrap()
            ],
        )
        .unwrap();
        prev = hash;
    }
    conn.execute_batch(&format!(
        "UPDATE audit_seals SET head_hash = (SELECT hash FROM audit_events e \
         WHERE e.stream = audit_seals.stream AND e.seq = audit_seals.last_seq) WHERE stream = '{stream}';
         UPDATE audit_seals SET prev_hash = coalesce((SELECT p.head_hash FROM audit_seals p \
         WHERE p.stream = audit_seals.stream AND p.seal_no = audit_seals.seal_no - 1), prev_hash) \
         WHERE stream = '{stream}';"
    ))
    .unwrap();
}
