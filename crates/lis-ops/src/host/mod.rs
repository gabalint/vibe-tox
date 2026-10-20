//! Host probes (REQ-OPS-008, NFR-25; M4 design brief D8, D15, §6.1, E4): operating
//! system, disk encryption, time synchronization, free space and network data folders.
//! I/O is kept apart from evaluation: per-OS parsers are pure functions over captured
//! output, and each fact is typed or `None` (unknown), never "ok" by default. Subprocesses
//! run through one bounded runner. Probes block for up to 10 seconds per tool, so the
//! engine calls them off the database threads.
//!
//! Storage facts fail closed. They describe the volume that actually holds the data
//! folder: its canonical path (links resolved), or its nearest existing ancestor before
//! first start, then the mount whose point is the longest prefix of that path. A file
//! system type, drive type or volume outside the fixed lists of known local and known
//! network kinds is unknown, never local, and an unknown nested mount is not replaced by
//! the known mount above it. Disk encryption is reported only for a volume the probe can
//! name: the BitLocker drive on Windows, the device under the folder on Linux, and on
//! macOS only the startup volume group that FileVault covers.

mod linux;
mod macos;
mod os_support;
mod process;
mod windows;

use std::path::{Path, PathBuf};
use std::{fs, io};

pub use os_support::{OsSupport, SupportStatus, assess};

/// Whether the disk holding the data folder is encrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskEncryption {
    /// Encrypted (BitLocker protected, FileVault on, or a LUKS device underneath).
    On,
    /// Not encrypted.
    Off,
}

/// The running operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsIdentity {
    /// Windows, from the registry.
    Windows(WindowsVersion),
    /// macOS, from `sw_vers`.
    Macos(MacosVersion),
    /// Linux, from `/etc/os-release`.
    Linux(LinuxRelease),
}

/// Windows version values from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsVersion {
    /// `CurrentBuild`, such as 26100.
    pub build: u32,
    /// `DisplayVersion`, such as `24H2`; absent on old builds.
    pub display_version: Option<String>,
    /// `EditionID`, such as `Professional` or `EnterpriseS`.
    pub edition_id: String,
}

/// macOS version from `sw_vers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosVersion {
    /// `ProductVersion`, such as `26.1`.
    pub product_version: String,
    /// Its major number.
    pub major: u32,
}

/// Linux release fields from `/etc/os-release`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxRelease {
    /// `ID`, such as `ubuntu`.
    pub id: String,
    /// `VERSION_ID`, such as `24.04`.
    pub version_id: String,
    /// `PRETTY_NAME`, when present.
    pub pretty_name: Option<String>,
}

/// What the probes found; `None` is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFacts {
    /// The operating system.
    pub os: Option<OsIdentity>,
    /// Encryption of the data folder's disk.
    pub disk_encryption: Option<DiskEncryption>,
    /// Whether the clock is synchronized; probed on Linux only.
    pub time_sync: Option<bool>,
    /// Bytes available to this user on the data folder's disk.
    pub available_bytes: Option<u64>,
}

/// Probes this host for the facts health evaluates (daily, and on refresh).
#[must_use]
pub fn probe(data_folder: &Path) -> HostFacts {
    HostFacts {
        os: probe_os(),
        disk_encryption: probe_disk_encryption(data_folder),
        time_sync: if cfg!(target_os = "linux") {
            linux::probe_time_sync()
        } else {
            None
        },
        available_bytes: available_bytes(data_folder),
    }
}

/// Bytes available to this user on the disk holding `path` (`fs4`), or `None`.
#[must_use]
pub fn available_bytes(path: &Path) -> Option<u64> {
    fs4::available_space(path).ok()
}

/// Whether the data folder is on network storage (D15, startup refusal): a UNC path or
/// a drive the system reports as remote on Windows, a network file system type on Linux
/// and macOS. `None` when unknown, including an unlisted file system or drive type. The
/// engine starts only on `Some(false)`: the common local disks of each OS (APFS, HFS+,
/// NTFS and ReFS fixed drives, ext4, XFS, Btrfs, ZFS) are listed so normal installs start.
#[must_use]
pub fn data_folder_on_network(data_folder: &Path) -> Option<bool> {
    // An unreachable share cannot be resolved, but its path alone shows it is remote.
    if cfg!(windows) && data_folder.to_str().is_some_and(windows::is_unc_path) {
        return Some(true);
    }
    let folder = existing_volume_path(data_folder)?;
    if cfg!(windows) {
        return windows::probe_network_folder(folder.to_str()?);
    }
    if cfg!(target_os = "linux") {
        return linux::probe_network_folder(&folder);
    }
    if cfg!(target_os = "macos") {
        return macos::probe_network_folder(&folder);
    }
    None
}

/// The canonical path of `path`, or of its nearest existing ancestor while it does not
/// exist yet, so probes ask about the volume that holds or will hold it. Links and
/// junctions are resolved; any error other than "not found" is unknown.
fn existing_volume_path(path: &Path) -> Option<PathBuf> {
    for candidate in path.ancestors() {
        match fs::canonicalize(candidate) {
            Ok(resolved) => return Some(resolved),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    None
}

/// The value of the mount whose point is the longest component-wise prefix of `path`;
/// at the same point a later mount covers an earlier one. `None` when no mount matches.
fn containing_mount<T>(mounts: impl IntoIterator<Item = (String, T)>, path: &Path) -> Option<T> {
    let mut best: Option<(usize, T)> = None;
    for (point, value) in mounts {
        let point = Path::new(&point);
        if !path.starts_with(point) {
            continue;
        }
        let depth = point.components().count();
        if best
            .as_ref()
            .is_none_or(|(best_depth, _)| depth >= *best_depth)
        {
            best = Some((depth, value));
        }
    }
    best.map(|(_, value)| value)
}

fn probe_os() -> Option<OsIdentity> {
    if cfg!(windows) {
        windows::probe_version().map(OsIdentity::Windows)
    } else if cfg!(target_os = "macos") {
        macos::probe_version().map(OsIdentity::Macos)
    } else if cfg!(target_os = "linux") {
        linux::probe_release().map(OsIdentity::Linux)
    } else {
        None
    }
}

fn probe_disk_encryption(data_folder: &Path) -> Option<DiskEncryption> {
    let folder = existing_volume_path(data_folder)?;
    if cfg!(windows) {
        return windows::probe_encryption(folder.to_str()?);
    }
    if cfg!(target_os = "macos") {
        return macos::probe_encryption(&folder);
    }
    #[cfg(unix)]
    if cfg!(target_os = "linux") {
        return linux::probe_encryption(&folder);
    }
    None
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn a_missing_data_folder_resolves_to_its_nearest_existing_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(root.path()).unwrap();
        let missing = root.path().join("not-yet").join("vibe-tox");
        assert_eq!(existing_volume_path(&missing), Some(canonical.clone()));
        fs::create_dir_all(&missing).unwrap();
        assert_eq!(
            existing_volume_path(&missing),
            Some(canonical.join("not-yet").join("vibe-tox"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_resolve_to_the_volume_they_point_at() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        let link = root.path().join("data-link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            existing_volume_path(&link.join("vibe-tox")),
            Some(fs::canonicalize(&target).unwrap())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_path_through_a_file_is_unknown_not_its_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, "synthetic").unwrap();
        assert_eq!(existing_volume_path(&file.join("vibe-tox")), None);
    }

    // D15: the engine refuses an unknown volume, so an ordinary disk must read as local.
    #[test]
    fn this_crates_folder_is_on_a_local_volume() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(data_folder_on_network(folder), Some(false));
        assert_eq!(data_folder_on_network(&folder.join("not-yet")), Some(false));
    }

    #[test]
    fn the_deepest_mount_wins_and_a_later_mount_covers_an_earlier_one() {
        let mounts = || {
            [
                ("/".to_owned(), "root"),
                ("/home".to_owned(), "home"),
                ("/home/lab".to_owned(), "first"),
                ("/home/lab".to_owned(), "over"),
                ("/home/lab2".to_owned(), "sibling"),
            ]
        };
        let pick = |path: &str| containing_mount(mounts(), Path::new(path));
        assert_eq!(pick("/home/lab/vibe-tox"), Some("over"));
        assert_eq!(pick("/home/lab"), Some("over"));
        assert_eq!(pick("/home/lab3"), Some("home"));
        assert_eq!(pick("/var/lib"), Some("root"));
        assert_eq!(
            containing_mount(mounts().into_iter().skip(1), Path::new("/var")),
            None
        );
    }

    proptest! {
        #[test]
        fn parsers_never_panic_on_arbitrary_output(text in any::<String>()) {
            let path = Path::new("/var/lib/vibe-tox");
            let _ = windows::parse_reg_query(&text);
            let _ = windows::parse_bitlocker_protection(&text);
            let _ = windows::parse_drive_type(&text);
            let _ = windows::is_unc_path(&text);
            let _ = macos::parse_sw_vers(&text);
            let _ = macos::parse_fdesetup_status(&text);
            let _ = macos::volume_kind(&text, path);
            let _ = linux::parse_os_release(&text);
            let _ = linux::parse_ntp_synchronized(&text);
            let _ = linux::is_network_mount(&text, path);
        }

        #[test]
        fn mountinfo_parsing_never_panics_on_escapes(
            point in "(/[a-z\\\\0-9]{0,6}){1,4}",
            fs_type in "[a-z0-9.]{1,10}",
        ) {
            let line = format!("36 35 98:0 / {point} rw - {fs_type} /dev/x rw\n");
            let _ = linux::is_network_mount(&line, Path::new("/a/b"));
        }
    }
}
