//! The demo's folders are always siblings of the laboratory folder on every OS's layout, and
//! resetting the demo can delete nothing else and never leaves a half-deleted demo
//! (proposed REQ-OPS-016). Synthetic folders only.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

/// A local data folder with a laboratory and a demo installation, each holding a file.
fn installed() -> (tempfile::TempDir, Folders) {
    let root = tempfile::tempdir().unwrap();
    let folders = Folders::new(
        root.path().join("org.vibetox.lis"),
        root.path().join("org.vibetox.lis").join("logs"),
    )
    .unwrap();
    for kind in [InstallationKind::Laboratory, InstallationKind::Demo] {
        let dir = folders.data_dir(kind);
        fs::create_dir_all(dir.join("artifacts")).unwrap();
        fs::write(dir.join("lis.sqlite3"), b"synthetic").unwrap();
    }
    (root, folders)
}

fn lab_intact(folders: &Folders) {
    let lab = folders.data_dir(InstallationKind::Laboratory);
    assert_eq!(fs::read(lab.join("lis.sqlite3")).unwrap(), b"synthetic");
    assert!(lab.join("artifacts").is_dir());
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Tauri's layouts: macOS keeps logs in `~/Library/Logs/<id>`; Windows and Linux in
/// `<local data>/<id>/logs`, inside the laboratory's data folder.
#[test]
fn the_demo_folders_are_siblings_outside_the_laboratorys_on_every_os() {
    for (data, logs) in [
        (
            "/Users/synthetic/Library/Application Support/org.vibetox.lis",
            "/Users/synthetic/Library/Logs/org.vibetox.lis",
        ),
        (
            "C:/Users/synthetic/AppData/Local/org.vibetox.lis",
            "C:/Users/synthetic/AppData/Local/org.vibetox.lis/logs",
        ),
        (
            "/home/synthetic/.local/share/org.vibetox.lis",
            "/home/synthetic/.local/share/org.vibetox.lis/logs",
        ),
    ] {
        let folders = Folders::new(PathBuf::from(data), PathBuf::from(logs)).unwrap();
        let lab = folders.data_dir(InstallationKind::Laboratory);
        let parent = lab.parent().unwrap();
        assert_eq!(
            folders.log_dir(InstallationKind::Laboratory),
            Path::new(logs)
        );
        let demo_paths = [
            folders.data_dir(InstallationKind::Demo),
            folders.log_dir(InstallationKind::Demo),
            folders.tombstone.as_path(),
            folders.choice.as_path(),
        ];
        assert_eq!(
            demo_paths.map(|path| path.strip_prefix(parent).unwrap().to_path_buf()),
            [
                "org.vibetox.lis.demo",
                "org.vibetox.lis.demo-logs",
                "org.vibetox.lis.demo-deleted",
                "org.vibetox.lis.installation",
            ]
            .map(PathBuf::from)
        );
        for path in demo_paths {
            assert!(
                !path.starts_with(lab),
                "{} is in the laboratory",
                path.display()
            );
            assert!(
                !lab.starts_with(path),
                "{} holds the laboratory",
                path.display()
            );
        }
    }
    assert!(Folders::new(PathBuf::from("/"), PathBuf::from("/logs")).is_none());
}

#[test]
fn reset_deletes_the_demo_folder_and_nothing_else() {
    let (_root, folders) = installed();
    folders.choose(Choice::DemoReset).unwrap();
    assert!(folders.reset_demo(Duration::ZERO).unwrap());
    assert!(!exists(folders.data_dir(InstallationKind::Demo)));
    assert!(!exists(&folders.tombstone));
    lab_intact(&folders);
    assert_eq!(
        folders.chosen().unwrap(),
        Choice::DemoReset,
        "the choice file is kept"
    );
    assert!(
        folders.reset_demo(Duration::ZERO).unwrap(),
        "nothing left to reset"
    );
}

#[test]
fn reset_refuses_any_folder_but_the_laboratory_folders_own_siblings() {
    let (root, folders) = installed();
    let lab = folders.data_dir(InstallationKind::Laboratory).to_path_buf();
    for wrong in [
        lab.clone(),
        lab.join("demo"),
        root.path().to_path_buf(),
        root.path().join("other.demo"),
    ] {
        let wrong_demo = Folders {
            demo: wrong.clone(),
            ..folders.clone()
        };
        let wrong_tombstone = Folders {
            tombstone: wrong,
            ..folders.clone()
        };
        assert!(wrong_demo.reset_demo(Duration::ZERO).is_err());
        assert!(wrong_tombstone.reset_demo(Duration::ZERO).is_err());
        assert!(wrong_tombstone.remove_tombstone().is_err());
        lab_intact(&folders);
        assert!(exists(folders.data_dir(InstallationKind::Demo)));
    }
}

// An interrupted reset leaves a tombstone: the next reset or start deletes it, and the demo
// folder it left is either whole or gone.
#[test]
fn a_leftover_tombstone_is_deleted_without_touching_the_demo() {
    let (_root, folders) = installed();
    fs::create_dir_all(folders.tombstone.join("artifacts")).unwrap();
    fs::write(folders.tombstone.join("lis.sqlite3-wal"), b"synthetic").unwrap();
    folders.remove_tombstone().unwrap();
    assert!(!exists(&folders.tombstone));
    let demo = folders.data_dir(InstallationKind::Demo);
    assert_eq!(fs::read(demo.join("lis.sqlite3")).unwrap(), b"synthetic");

    fs::create_dir_all(&folders.tombstone).unwrap();
    assert!(folders.reset_demo(Duration::ZERO).unwrap());
    assert!(!exists(demo) && !exists(&folders.tombstone));
    lab_intact(&folders);
}

#[cfg(unix)]
#[test]
fn a_reset_whose_delete_fails_leaves_no_demo_and_finishes_later() {
    use std::os::unix::fs::PermissionsExt;
    let (_root, folders) = installed();
    let locked = folders.data_dir(InstallationKind::Demo).join("artifacts");
    fs::write(locked.join("artifact"), b"synthetic").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();
    let deleted = folders.reset_demo(Duration::ZERO).unwrap();
    let held = folders.tombstone.join("artifacts");
    fs::set_permissions(&held, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!deleted, "the tombstone could not be deleted");
    assert!(
        !exists(folders.data_dir(InstallationKind::Demo)),
        "never half a demo"
    );
    folders.remove_tombstone().unwrap();
    assert!(!exists(&folders.tombstone));
    lab_intact(&folders);
}

#[cfg(unix)]
#[test]
fn reset_moves_and_removes_links_without_following_them() {
    let (_root, folders) = installed();
    let demo = folders.data_dir(InstallationKind::Demo);
    fs::remove_dir_all(demo).unwrap();
    let lab = folders.data_dir(InstallationKind::Laboratory);
    std::os::unix::fs::symlink(lab, demo).unwrap();
    assert!(folders.reset_demo(Duration::ZERO).unwrap());
    assert!(!exists(demo) && !exists(&folders.tombstone));
    std::os::unix::fs::symlink(lab, &folders.tombstone).unwrap();
    folders.remove_tombstone().unwrap();
    assert!(!exists(&folders.tombstone));
    lab_intact(&folders);
}

#[test]
fn the_choice_defaults_to_the_laboratory_and_round_trips() {
    let (_root, folders) = installed();
    assert_eq!(folders.chosen().unwrap(), Choice::Laboratory);
    for choice in [Choice::Demo, Choice::DemoReset, Choice::Laboratory] {
        folders.choose(choice).unwrap();
        assert_eq!(folders.chosen().unwrap(), choice);
    }
    fs::write(&folders.choice, "production\n").unwrap();
    assert!(folders.chosen().is_err());
    assert_eq!(Choice::DemoReset.kind(), InstallationKind::Demo);
}

#[test]
fn the_demo_has_its_own_key_store_entry() {
    assert_eq!(
        key_identifier("org.vibetox.lis", InstallationKind::Laboratory),
        "org.vibetox.lis"
    );
    assert_eq!(
        key_identifier("org.vibetox.lis", InstallationKind::Demo),
        "org.vibetox.lis.demo"
    );
}
