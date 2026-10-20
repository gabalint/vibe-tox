//! The recovery kit (REQ-OPS-007, NFR-30; M4 design brief §3.3, §3.9), and the canary
//! check of the backup area (NFR-32): clinical data never reaches a target file name,
//! `target.json`, the kit's non-key lines, an audit event or a log line, and key
//! material never leaves the kit's key line.

#![cfg(test)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use lis_auth::DataKey;
use lis_core::{Capability, CorrelationId, ErrorCode, Migration, ModuleSchema, Principal};
use lis_ops::backup::{self, BackupError};
use lis_ops::format::BackupKey;
use lis_ops::kit::{self, CONFIRM_RECOVERY_KIT, KitExport, KitInput, OutdatedReason};
use lis_ops::policy::DEFAULT_SETTINGS;
use lis_ops::targets::{self, NewTarget, Target, TargetKind, TargetPurpose};
use lis_store::StoreError;
use lis_store::testing::{TestDatabase, principal};
use proptest::prelude::*;
use tempfile::TempDir;

/// A synthetic patient name planted in clinical data.
const CANARY: &str = "Syntheticcanaryzq";

/// A test-only clinical table holding the canary.
const CANARY_SCHEMA: ModuleSchema = ModuleSchema {
    module: "canary",
    table_prefix: "canary_",
    migrations: &[Migration {
        version: 1,
        name: "canary",
        sql: "CREATE TABLE canary_patients (name TEXT NOT NULL) STRICT;",
    }],
};

struct Fixture {
    test: TestDatabase,
    key: DataKey,
    folders: TempDir,
}

fn system() -> Principal {
    Principal::system("test", [])
}

fn admin() -> Principal {
    principal(&[Capability::BackupRestore])
}

impl Fixture {
    fn new() -> Self {
        let test = TestDatabase::open(&[lis_ops::SCHEMA, CANARY_SCHEMA]).unwrap();
        test.db
            .write_blocking(
                system(),
                CorrelationId::generate(),
                lis_ops::ensure_installation,
            )
            .unwrap();
        Self {
            test,
            key: DataKey::from_bytes([9; 32]),
            folders: tempfile::tempdir().unwrap(),
        }
    }

    fn target(&self, name: &str) -> Target {
        let folder = self.folders.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        let data_dir = self.test.data_dir().to_path_buf();
        let key_id = BackupKey::derive(&self.key).key_id();
        let input = NewTarget {
            label: format!("{name} drive"),
            kind: TargetKind::Removable,
            purpose: TargetPurpose::BackupAndAnchors,
            folder,
        };
        self.test
            .db
            .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
                targets::add_target(ctx, &data_dir, key_id, &input)
            })
            .unwrap()
    }

    fn kit_path(&self) -> PathBuf {
        self.folders
            .path()
            .join("vibe-tox-recovery-kit-2026-01-02.txt")
    }

    fn export_as(&self, who: Principal, reference: &str) -> Result<KitExport, BackupError> {
        let key = self.key.clone();
        let data_dir = self.test.data_dir().to_path_buf();
        let path = self.kit_path();
        let reference = reference.to_owned();
        self.test
            .db
            .write_blocking(who, CorrelationId::generate(), move |ctx| {
                let input = KitInput {
                    created_by_name: "synthetic.admin",
                    operating_system: "Synthetic OS 1.0",
                    data_dir: &data_dir,
                    disk_encryption_reference: &reference,
                };
                kit::export_kit(ctx, &key, &input, &path)
            })
    }

    fn export(&self) -> KitExport {
        self.export_as(admin(), "BitLocker recovery key in the director's safe")
            .unwrap()
    }

    fn confirm(&self, kit_no: i64, note: &str) -> Result<(), BackupError> {
        let note = note.to_owned();
        self.test
            .db
            .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
                kit::confirm_storage(ctx, kit_no, &note)
            })
    }

    fn status(&self, key: &DataKey) -> kit::KitStatus {
        let key_id = BackupKey::derive(key).key_id();
        self.test
            .db
            .read_blocking(move |conn| kit::status(conn, key_id))
            .unwrap()
    }

    /// `(action, details, reason)` of every audit event.
    fn audit_events(&self) -> Vec<(String, String, Option<String>)> {
        self.test
            .db
            .read_blocking(|conn| -> Result<_, StoreError> {
                let mut statement =
                    conn.prepare("SELECT action, details, reason FROM audit_events")?;
                let rows = statement
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .unwrap()
    }
}

#[test]
fn the_kit_holds_the_key_line_locations_and_steps_and_is_recorded_without_the_key() {
    let f = Fixture::new();
    let target = f.target("usb");
    let exported = f.export();
    assert_eq!(
        exported,
        KitExport {
            kit_no: 1,
            file_name: "vibe-tox-recovery-kit-2026-01-02.txt".to_owned(),
        }
    );
    let text = fs::read_to_string(f.kit_path()).unwrap();
    assert!(text.starts_with("Vibe Toxicology LIS recovery kit\nFormat: vtkit-1\n"));
    let key_line = text
        .lines()
        .find_map(|line| line.strip_prefix("Installation key: "))
        .unwrap();
    assert_eq!(kit::parse_key(key_line).unwrap().expose(), f.key.expose());
    let key_id = BackupKey::derive(&f.key).key_id().to_string();
    let fingerprint = text
        .lines()
        .find_map(|line| line.strip_prefix("Key fingerprint: "))
        .unwrap();
    assert_eq!(fingerprint.replace('-', ""), key_id);
    let location = format!(
        "- usb drive (removable, backup_and_anchors): {}",
        target.path.display()
    );
    for expected in [
        "Kit number: 1",
        "Created by: synthetic.admin",
        "Operating system: Synthetic OS 1.0",
        "Disk-encryption recovery key: BitLocker recovery key in the director's safe",
        location.as_str(),
        "Cloud re-enrollment: not applicable in this version",
        "Break-glass envelope: not applicable in this version",
    ] {
        assert!(text.lines().any(|line| line == expected), "{expected}");
    }
    let hex_key = hex::encode(f.key.expose());
    let export_events: Vec<_> = f
        .audit_events()
        .into_iter()
        .filter(|(action, _, _)| action == kit::EXPORT_RECOVERY_KIT)
        .collect();
    assert_eq!(export_events.len(), 1);
    assert_eq!(
        export_events[0].1,
        format!(r#"{{"key_id":"{key_id}","kit_no":1}}"#)
    );
    assert!(!export_events[0].1.contains(&hex_key[..16]));
}

#[test]
fn export_is_refused_without_capability_or_reference_and_writes_no_file() {
    let f = Fixture::new();
    let denied = f
        .export_as(principal(&[Capability::Export]), "Safe")
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    assert!(matches!(
        f.export_as(admin(), "  "),
        Err(BackupError::Invalid(field)) if field.field == "diskEncryptionReference"
    ));
    assert!(!f.kit_path().exists());
}

#[test]
fn the_file_is_removed_when_the_record_fails() {
    let f = Fixture::new();
    f.test
        .db
        .write_blocking(
            system(),
            CorrelationId::generate(),
            |ctx| -> Result<_, StoreError> {
                ctx.tx().execute_batch(
                    "CREATE TRIGGER canary_refuse_kits BEFORE INSERT ON ops_recovery_kits \
                 BEGIN SELECT RAISE(ABORT, 'refused for the test'); END;",
                )?;
                Ok(())
            },
        )
        .unwrap();
    assert!(f.export_as(admin(), "Safe").is_err());
    assert!(!f.kit_path().exists());
}

#[test]
fn the_kit_is_outdated_until_exported_and_again_when_key_or_locations_change() {
    let f = Fixture::new();
    assert_eq!(
        f.status(&f.key).outdated_reasons,
        [OutdatedReason::NeverExported]
    );
    f.target("usb");
    f.export();
    let status = f.status(&f.key);
    assert!(status.outdated_reasons.is_empty());
    assert_eq!(status.latest.unwrap().kit_no, 1);
    assert_eq!(
        f.status(&DataKey::from_bytes([1; 32])).outdated_reasons,
        [OutdatedReason::KeyChanged]
    );
    f.target("share");
    assert_eq!(
        f.status(&f.key).outdated_reasons,
        [OutdatedReason::TargetsChanged]
    );
    assert_eq!(f.export().kit_no, 2);
    assert!(f.status(&f.key).outdated_reasons.is_empty());
}

#[test]
fn only_the_latest_kit_is_confirmed_with_a_note() {
    let f = Fixture::new();
    assert!(matches!(
        f.confirm(1, "Safe"),
        Err(BackupError::NotFound(_))
    ));
    f.export();
    f.export();
    assert!(matches!(
        f.confirm(1, "Safe"),
        Err(BackupError::KitNotLatest)
    ));
    assert!(matches!(f.confirm(2, " "), Err(BackupError::Invalid(_))));
    assert!(f.status(&f.key).last_confirmation.is_none());
    f.confirm(2, "Director's safe and the bank box").unwrap();
    assert!(f.status(&f.key).last_confirmation.is_some());
    let reasons: Vec<Option<String>> = f
        .audit_events()
        .into_iter()
        .filter(|(action, _, _)| action == CONFIRM_RECOVERY_KIT)
        .map(|(_, _, reason)| reason)
        .collect();
    assert_eq!(
        reasons,
        [Some("Director's safe and the bank box".to_owned())]
    );
}

#[test]
fn a_mistyped_or_malformed_key_line_is_refused() {
    let key = DataKey::from_bytes([0xab; 32]);
    let text = kit::key_text(&key).to_string();
    let mut mistyped = text.clone();
    mistyped.replace_range(5..6, if &text[5..6] == "0" { "1" } else { "0" });
    assert!(matches!(
        kit::parse_key(&mistyped),
        Err(BackupError::KeyChecksum)
    ));
    let malformed = [
        String::new(),
        "vtk1-".to_owned(),
        text[..text.len() - 1].to_owned(),
        text.replacen("vtk1-", "vtk2-", 1),
        text.replacen('-', "", 2),
        format!("{text}-0000"),
        format!("{}g{}", &text[..5], &text[6..]),
        format!("Installation key: {text}"),
    ];
    for malformed in &malformed {
        assert!(
            matches!(kit::parse_key(malformed), Err(BackupError::KeyFormat)),
            "{malformed}"
        );
    }
}

#[test]
fn the_default_file_name_carries_the_date() {
    let now = lis_core::UtcTimestamp::parse("2026-10-13T08:00:00.000000000Z").unwrap();
    assert_eq!(
        kit::default_file_name(now),
        "vibe-tox-recovery-kit-2026-10-13.txt"
    );
}

proptest! {
    #[test]
    fn any_key_round_trips_through_its_key_line(bytes: [u8; 32]) {
        let key = DataKey::from_bytes(bytes);
        let text = kit::key_text(&key);
        let parsed = kit::parse_key(&text).unwrap();
        prop_assert_eq!(parsed.expose(), &bytes);
    }

    #[test]
    fn parsing_arbitrary_text_never_panics(text in ".{0,120}") {
        prop_assert!(kit::parse_key(&text).is_err());
    }
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Everything logged by this test binary since the first call.
fn captured() -> &'static Capture {
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).unwrap();
        capture
    })
}

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        found.push(path.clone());
        if path.is_dir() {
            files_under(&path, found);
        }
    }
}

#[test]
fn clinical_data_and_key_material_stay_out_of_targets_kit_lines_audit_and_logs() {
    let capture = captured();
    let f = Fixture::new();
    f.test
        .db
        .write_blocking(
            system(),
            CorrelationId::generate(),
            |ctx| -> Result<_, StoreError> {
                ctx.tx()
                    .execute("INSERT INTO canary_patients (name) VALUES (?1)", [CANARY])?;
                Ok(())
            },
        )
        .unwrap();
    f.test
        .db
        .artifacts()
        .put(format!("synthetic report for {CANARY}").as_bytes())
        .unwrap();
    let target = f.target("usb");
    let backup_key = BackupKey::derive(&f.key);
    let report = backup::run(
        &f.test.db,
        f.test.data_dir(),
        Some(&backup_key),
        &DEFAULT_SETTINGS,
        f.test.clock.as_ref(),
        &AtomicBool::new(false),
    )
    .unwrap()
    .unwrap();
    assert_eq!(report.copies_verified, 1);
    f.export();

    let secrets = [
        kit::key_text(&f.key).to_string(),
        hex::encode(f.key.expose()),
    ];
    let path = target.path.display().to_string();
    assert_target_files_clean(&targets::root(&target), &secrets[1]);
    let kit_text = fs::read_to_string(f.kit_path()).unwrap();
    for line in kit_text.lines() {
        assert!(!line.contains(CANARY), "{line}");
        if !line.starts_with("Installation key: ") {
            assert!(
                !secrets.iter().any(|secret| line.contains(secret)),
                "{line}"
            );
        }
    }
    for (action, details, reason) in f.audit_events() {
        let event = format!("{action} {details} {reason:?}");
        assert!(!event.contains(CANARY) && !event.contains(&path), "{event}");
        assert!(
            !secrets.iter().any(|secret| event.contains(secret)),
            "{event}"
        );
    }
    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("backup run finished"),
        "the capture sees the run"
    );
    assert!(!logs.contains(CANARY) && !logs.contains(&path));
    assert!(!secrets.iter().any(|secret| logs.contains(secret)));
}

/// No file name holds the canary, and no file holds it or the key in plaintext.
fn assert_target_files_clean(root: &Path, key_hex: &str) {
    let mut files = Vec::new();
    files_under(root, &mut files);
    assert!(files.len() >= 5, "{files:?}");
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy();
        assert!(!name.contains(CANARY), "{name}");
        if file.is_dir() {
            continue;
        }
        let bytes = fs::read(file).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(CANARY),
            "{name} holds the canary in plaintext"
        );
        assert!(!text.contains(key_hex), "{name} holds the key");
    }
}
