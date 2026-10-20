//! Backup targets (REQ-OPS-007, NFR-20; M4 design brief §3.3): the marker comes before
//! the row, availability follows the marker, and unsafe folders are refused.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

use lis_auth::DataKey;
use lis_core::{Capability, CorrelationId, ErrorCode, Principal};
use lis_ops::backup::BackupError;
use lis_ops::format::BackupKey;
use lis_ops::targets::{
    self, ADD_TARGET, NewTarget, RETIRE_TARGET, Target, TargetId, TargetKind, TargetPurpose,
};
use lis_store::StoreError;
use lis_store::testing::{TestDatabase, principal};
use tempfile::TempDir;

struct Fixture {
    test: TestDatabase,
    folders: TempDir,
}

fn fixture() -> Fixture {
    let test = TestDatabase::open(&[lis_ops::SCHEMA]).unwrap();
    test.db
        .write_blocking(
            Principal::system("test", []),
            CorrelationId::generate(),
            lis_ops::ensure_installation,
        )
        .unwrap();
    Fixture {
        test,
        folders: tempfile::tempdir().unwrap(),
    }
}

fn admin() -> Principal {
    principal(&[Capability::BackupRestore])
}

fn key(byte: u8) -> DataKey {
    DataKey::from_bytes([byte; 32])
}

impl Fixture {
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.folders.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn add_as(&self, who: Principal, folder: &Path, key: &DataKey) -> Result<Target, BackupError> {
        let data_dir = self.test.data_dir().to_path_buf();
        let key_id = BackupKey::derive(key).key_id();
        let input = NewTarget {
            label: "Second disk".to_owned(),
            kind: TargetKind::Removable,
            purpose: TargetPurpose::BackupAndAnchors,
            folder: folder.to_path_buf(),
        };
        self.test
            .db
            .write_blocking(who, CorrelationId::generate(), move |ctx| {
                targets::add_target(ctx, &data_dir, key_id, &input)
            })
    }

    fn add(&self, folder: &Path) -> Result<Target, BackupError> {
        self.add_as(admin(), folder, &key(1))
    }

    fn retire(&self, id: TargetId, revision: i64, reason: &str) -> Result<(), BackupError> {
        let reason = reason.to_owned();
        self.test
            .db
            .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
                targets::retire_target(ctx, id, revision, &reason)
            })
    }

    fn active(&self) -> Vec<Target> {
        self.test.db.read_blocking(targets::active).unwrap()
    }

    /// `(details, reason)` of each event with `action`, oldest first.
    fn events(&self, action: &'static str) -> Vec<(String, Option<String>)> {
        self.test
            .db
            .read_blocking(move |conn| -> Result<_, StoreError> {
                let mut statement = conn.prepare(
                    "SELECT details, reason FROM audit_events WHERE action = ?1 ORDER BY seq",
                )?;
                let rows = statement
                    .query_map([action], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .unwrap()
    }
}

fn marker(folder: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(folder.join("VibeToxLIS/target.json")).unwrap()).unwrap()
}

#[test]
fn adding_a_target_writes_an_identifier_only_marker_and_an_audited_row() {
    let f = fixture();
    let folder = f.folder("drive");
    let target = f.add(&folder).unwrap();
    assert_eq!(f.active(), std::slice::from_ref(&target));
    assert!(targets::is_available(&target));
    assert_eq!(
        targets::root(&target),
        folder.canonicalize().unwrap().join("VibeToxLIS")
    );
    let marker = marker(&folder);
    let keys: Vec<&str> = marker
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 5);
    for key_name in ["format", "installationId", "targetId", "keyId", "createdAt"] {
        assert!(keys.contains(&key_name), "{key_name}");
    }
    assert_eq!(marker["format"], "vibe-tox-target-1");
    assert_eq!(marker["targetId"], target.id.to_string());
    assert_eq!(
        marker["keyId"],
        BackupKey::derive(&key(1)).key_id().to_string()
    );
    let events = f.events(ADD_TARGET);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].0,
        r#"{"kind":"removable","purpose":"backup_and_anchors"}"#
    );
}

#[test]
fn adding_requires_backup_restore_and_writes_nothing_without_it() {
    let f = fixture();
    let folder = f.folder("drive");
    let error = f
        .add_as(
            principal(&[Capability::ManageConfiguration]),
            &folder,
            &key(1),
        )
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert!(f.active().is_empty());
    assert!(!folder.join("VibeToxLIS").exists());
}

#[test]
fn a_folder_inside_the_data_folder_is_refused() {
    let f = fixture();
    let inside = f.test.data_dir().join("copies");
    fs::create_dir_all(&inside).unwrap();
    assert!(matches!(
        f.add(&inside),
        Err(BackupError::TargetInsideDataFolder)
    ));
    assert!(matches!(
        f.add(f.test.data_dir()),
        Err(BackupError::TargetInsideDataFolder)
    ));
    assert!(!inside.join("VibeToxLIS").exists());
}

#[test]
fn a_missing_folder_or_a_blank_label_is_refused() {
    let f = fixture();
    let missing = f.folders.path().join("unplugged");
    assert!(matches!(
        f.add(&missing),
        Err(BackupError::TargetFolderUnusable)
    ));
    let data_dir = f.test.data_dir().to_path_buf();
    let folder = f.folder("drive");
    let input = NewTarget {
        label: "  ".to_owned(),
        kind: TargetKind::LocalDisk,
        purpose: TargetPurpose::AnchorsOnly,
        folder: folder.clone(),
    };
    let key_id = BackupKey::derive(&key(1)).key_id();
    let error = f
        .test
        .db
        .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
            targets::add_target(ctx, &data_dir, key_id, &input)
        })
        .unwrap_err();
    assert!(matches!(error, BackupError::Invalid(field) if field.field == "label"));
    assert!(!folder.join("VibeToxLIS").exists());
}

#[cfg(unix)]
#[test]
fn an_unwritable_folder_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let folder = f.folder("read-only");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(folder.join("probe"), b"x").is_ok() {
        // Running as a user that ignores permissions (root); nothing to test.
        return;
    }
    let result = f.add(&folder);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(BackupError::TargetNotWritable)));
    assert!(f.active().is_empty());
}

#[test]
fn a_folder_of_another_installation_is_refused() {
    let ours = fixture();
    let theirs = fixture();
    let folder = ours.folder("shared");
    theirs.add(&folder).unwrap();
    assert!(matches!(
        ours.add(&folder),
        Err(BackupError::TargetOfAnotherInstallation)
    ));
    assert!(ours.active().is_empty());
}

#[test]
fn a_folder_of_another_key_is_refused_and_an_active_folder_is_not_added_twice() {
    let f = fixture();
    let folder = f.folder("drive");
    let target = f.add(&folder).unwrap();
    assert!(matches!(
        f.add(&folder),
        Err(BackupError::TargetAlreadyAdded)
    ));
    f.retire(target.id, 1, "Drive replaced").unwrap();
    assert!(matches!(
        f.add_as(admin(), &folder, &key(2)),
        Err(BackupError::TargetKeyMismatch)
    ));
    let readded = f.add(&folder).unwrap();
    assert_ne!(readded.id, target.id);
    assert_eq!(marker(&folder)["targetId"], readded.id.to_string());
}

#[test]
fn availability_follows_the_marker_like_an_unplugged_drive() {
    let f = fixture();
    let folder = f.folder("drive");
    let target = f.add(&folder).unwrap();
    let away = f.folders.path().join("elsewhere");
    fs::rename(&folder, &away).unwrap();
    assert!(!targets::is_available(&target));
    fs::rename(&away, &folder).unwrap();
    assert!(targets::is_available(&target));
    fs::write(
        folder.join("VibeToxLIS/target.json"),
        br#"{"format":"something-else"}"#,
    )
    .unwrap();
    assert!(!targets::is_available(&target));
}

#[test]
fn a_target_is_retired_once_with_its_revision_and_a_reason() {
    let f = fixture();
    let target = f.add(&f.folder("drive")).unwrap();
    assert!(matches!(
        f.retire(target.id, 2, "Stale"),
        Err(BackupError::RevisionConflict(_))
    ));
    assert!(matches!(
        f.retire(target.id, 1, " "),
        Err(BackupError::Invalid(_))
    ));
    f.retire(target.id, 1, "Drive replaced").unwrap();
    assert!(f.active().is_empty());
    assert!(matches!(
        f.retire(target.id, 2, "Again"),
        Err(BackupError::TargetRetired)
    ));
    assert!(matches!(
        f.retire(TargetId::generate(), 1, "Unknown"),
        Err(BackupError::NotFound(_))
    ));
    let events = f.events(RETIRE_TARGET);
    assert_eq!(
        events,
        [("{}".to_owned(), Some("Drive replaced".to_owned()))]
    );
}

#[test]
fn target_rows_cannot_be_edited_reactivated_or_deleted() {
    let f = fixture();
    let target = f.add(&f.folder("drive")).unwrap();
    f.retire(target.id, 1, "Drive replaced").unwrap();
    let id = target.id;
    let refused: Vec<bool> = f
        .test
        .db
        .write_blocking(
            admin(),
            CorrelationId::generate(),
            move |ctx| -> Result<_, StoreError> {
                Ok([
                    "UPDATE ops_targets SET label = 'Other' WHERE id = ?1",
                    "UPDATE ops_targets SET retired_at = NULL, retired_by = NULL, \
                 retire_reason = NULL, revision = revision + 1 WHERE id = ?1",
                    "DELETE FROM ops_targets WHERE id = ?1",
                ]
                .iter()
                .map(|sql| ctx.tx().execute(sql, [id]).is_err())
                .collect())
            },
        )
        .unwrap();
    assert_eq!(refused, [true, true, true]);
}

#[test]
fn only_removable_and_network_targets_are_off_host() {
    assert!(!TargetKind::LocalDisk.is_off_host());
    assert!(TargetKind::Removable.is_off_host());
    assert!(TargetKind::NetworkShare.is_off_host());
    assert!(TargetPurpose::BackupAndAnchors.takes_backups());
    assert!(!TargetPurpose::AnchorsOnly.takes_backups());
}
