//! macOS probes (M4 design brief §6.1): the version from `sw_vers`, the data folder's
//! volume from the deepest `mount` entry holding it, and FileVault from `fdesetup
//! status`. FileVault covers only the startup volume group (`/` and
//! `/System/Volumes/Data`), so on any other volume encryption is unknown. A volume is
//! local only with a listed disk type and the kernel's `local` flag; `smbfs`, `afpfs`,
//! `nfs`, `webdav` and `ftp` are network; anything else (macFUSE, disk images reported
//! with other types) is unknown. The parsers are pure, and anything unexpected is `None`.

use std::path::Path;

use super::{DiskEncryption, MacosVersion, containing_mount, process};

const SW_VERS: &str = "/usr/bin/sw_vers";
const FDESETUP: &str = "/usr/bin/fdesetup";
const MOUNT: &str = "/sbin/mount";

/// Mount points of the startup volume group, which FileVault encrypts.
const STARTUP_MOUNT_POINTS: [&str; 2] = ["/", "/System/Volumes/Data"];
/// Disk file systems on locally attached storage, when the mount is also flagged `local`.
/// `hfs` is HFS+; `ntfs` is the read-only system driver; `zfs` is OpenZFS.
const LOCAL_FS_TYPES: [&str; 6] = ["apfs", "hfs", "exfat", "msdos", "ntfs", "zfs"];
/// Network file systems, where SQLite WAL fails.
const NETWORK_FS_TYPES: [&str; 5] = ["smbfs", "afpfs", "nfs", "webdav", "ftp"];

/// The kind of volume that holds a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VolumeKind {
    /// The startup volume group, covered by FileVault.
    Startup,
    /// Another local disk, such as an external drive; FileVault does not cover it.
    OtherLocal,
    /// A network file system.
    Network,
}

/// The running macOS version, or `None` when it cannot be read.
pub(super) fn probe_version() -> Option<MacosVersion> {
    parse_sw_vers(&process::run(SW_VERS, &[])?)
}

/// FileVault state, when the resolved data folder is on the startup volume group;
/// unknown on any other volume.
pub(super) fn probe_encryption(data_folder: &Path) -> Option<DiskEncryption> {
    if probe_volume(data_folder)? != VolumeKind::Startup {
        return None;
    }
    parse_fdesetup_status(&process::run(FDESETUP, &["status"])?)
}

/// Whether the resolved data folder is on a network file system; `None` when unknown.
pub(super) fn probe_network_folder(data_folder: &Path) -> Option<bool> {
    Some(probe_volume(data_folder)? == VolumeKind::Network)
}

fn probe_volume(data_folder: &Path) -> Option<VolumeKind> {
    volume_kind(&process::run(MOUNT, &[])?, data_folder)
}

/// The kind of the deepest mount holding `path` in `mount` output; `None` when no mount
/// matches or that mount is not a listed kind.
pub(super) fn volume_kind(mount_output: &str, path: &Path) -> Option<VolumeKind> {
    let mounts = mount_output.lines().filter_map(|line| {
        let (point, fs_type, local) = mount_entry(line)?;
        let kind = classify_mount(&point, fs_type, local);
        Some((point, kind))
    });
    containing_mount(mounts, path).flatten()
}

/// The mount point, type and `local` flag of `<source> on <point> (<type>, <flags>)`.
/// Mount points may contain spaces; flags never contain parentheses.
fn mount_entry(line: &str) -> Option<(String, &str, bool)> {
    let (left, options) = line.strip_suffix(')')?.rsplit_once(" (")?;
    let (_, point) = left.split_once(" on /")?;
    let mut options = options.split(", ");
    let fs_type = options.next()?;
    Some((
        format!("/{point}"),
        fs_type,
        options.any(|flag| flag == "local"),
    ))
}

fn classify_mount(point: &str, fs_type: &str, local: bool) -> Option<VolumeKind> {
    if NETWORK_FS_TYPES.contains(&fs_type) {
        return Some(VolumeKind::Network);
    }
    if !local || !LOCAL_FS_TYPES.contains(&fs_type) {
        return None;
    }
    Some(if STARTUP_MOUNT_POINTS.contains(&point) {
        VolumeKind::Startup
    } else {
        VolumeKind::OtherLocal
    })
}

/// `ProductName` must be macOS; `ProductVersion` gives the major version.
pub(super) fn parse_sw_vers(output: &str) -> Option<MacosVersion> {
    let field = |name: &str| {
        output.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim())
        })
    };
    let name = field("ProductName")?;
    if name != "macOS" && name != "Mac OS X" {
        return None;
    }
    let product_version = field("ProductVersion")?;
    let major = product_version.split('.').next()?.parse().ok()?;
    Some(MacosVersion {
        product_version: product_version.to_owned(),
        major,
    })
}

/// "FileVault is On." (also while finishing after a restart) or "FileVault is Off.";
/// encryption or decryption in progress and anything else are unknown.
pub(super) fn parse_fdesetup_status(output: &str) -> Option<DiskEncryption> {
    let first = output.lines().next()?.trim();
    if first.contains("in progress") || output.contains("in progress") {
        return None;
    }
    if first.starts_with("FileVault is On") {
        Some(DiskEncryption::On)
    } else if first.starts_with("FileVault is Off") {
        Some(DiskEncryption::Off)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sw_vers_gives_the_major_version() {
        let output = "ProductName:\t\tmacOS\nProductVersion:\t\t26.1\nBuildVersion:\t\t25B78\n";
        let version = parse_sw_vers(output).unwrap();
        assert_eq!(
            (version.major, version.product_version.as_str()),
            (26, "26.1")
        );
        let old = "ProductName:\tMac OS X\nProductVersion:\t10.15.7\nBuildVersion:\t19H15\n";
        assert_eq!(parse_sw_vers(old).unwrap().major, 10);
    }

    #[test]
    fn other_products_and_malformed_versions_are_unknown() {
        assert_eq!(parse_sw_vers(""), None);
        assert_eq!(
            parse_sw_vers("ProductName:\tiPadOS\nProductVersion:\t26.0\n"),
            None
        );
        assert_eq!(
            parse_sw_vers("ProductName:\tmacOS\nProductVersion:\tbeta\n"),
            None
        );
        assert_eq!(parse_sw_vers("ProductName:\tmacOS\n"), None);
    }

    const MOUNT_OUTPUT: &str = "\
/dev/disk3s1s1 on / (apfs, sealed, local, read-only, journaled)
devfs on /dev (devfs, local, nobrowse)
/dev/disk3s5 on /System/Volumes/Data (apfs, local, journaled, nobrowse, protect, root data)
map auto_home on /System/Volumes/Data/home (autofs, automounted, nobrowse)
/dev/disk7s1 on /Volumes/Lab Backup (apfs, local, nodev, nosuid, journaled, noowners)
/dev/disk8s1 on /Volumes/USB (msdos, local, nodev, nosuid, noowners)
//lab@nas.example/share on /Volumes/share (smbfs, nodev, nosuid, mounted by lab)
afp_synthetic@nas.example on /Volumes/afp (afpfs, nodev, nosuid, mounted by lab)
nas.example:/export on /Volumes/export (nfs, asynchronous, nodev, nosuid)
https://dav.example/ on /Volumes/dav.example (webdav, nodev, noexec, nosuid)
macfuse@synthetic on /Volumes/fuse (macfuse, nodev, nosuid, synchronous)
/dev/disk9s1 on /Volumes/NoFlag (apfs, nodev, nosuid, journaled)
/dev/disk7s2 on /Volumes/Lab Backup/Inner (exfat, local, nodev, nosuid)
";

    fn kind(path: &str) -> Option<VolumeKind> {
        volume_kind(MOUNT_OUTPUT, Path::new(path))
    }

    #[test]
    fn the_startup_volume_group_is_the_one_filevault_covers() {
        let home = "/Users/lab/Library/Application Support/vibe-tox";
        assert_eq!(kind(home), Some(VolumeKind::Startup));
        assert_eq!(
            kind("/System/Volumes/Data/Users/lab/vibe-tox"),
            Some(VolumeKind::Startup)
        );
    }

    // NFR-31: FileVault on the startup disk says nothing about an external volume.
    #[test]
    fn external_local_volumes_are_not_the_startup_volume() {
        assert_eq!(
            kind("/Volumes/Lab Backup/vibe-tox"),
            Some(VolumeKind::OtherLocal)
        );
        assert_eq!(kind("/Volumes/USB/vibe-tox"), Some(VolumeKind::OtherLocal));
        assert_eq!(
            kind("/Volumes/Lab Backup/Inner/vibe-tox"),
            Some(VolumeKind::OtherLocal)
        );
    }

    // D15: the app starts only on a local volume, so the usual Mac disks must read local.
    #[test]
    fn apfs_hfs_plus_and_other_local_disks_are_local() {
        let mounts = "\
/dev/disk3s1s1 on / (apfs, sealed, local, read-only, journaled)
/dev/disk4s2 on /Volumes/Old Mac (hfs, local, journaled)
/dev/disk5s1 on /Volumes/Pool (zfs, local, noatime)
/dev/disk6s1 on /Volumes/Windows (ntfs, local, read-only, noowners)
";
        for path in [
            "/Users/lab/vibe-tox",
            "/Volumes/Old Mac/vibe-tox",
            "/Volumes/Pool/vibe-tox",
            "/Volumes/Windows/vibe-tox",
        ] {
            assert!(
                volume_kind(mounts, Path::new(path))
                    .is_some_and(|kind| kind != VolumeKind::Network),
                "{path}"
            );
        }
    }

    #[test]
    fn smb_afp_nfs_and_webdav_mounts_are_network() {
        for path in [
            "/Volumes/share/vibe-tox",
            "/Volumes/afp/vibe-tox",
            "/Volumes/export/vibe-tox",
            "/Volumes/dav.example/vibe-tox",
        ] {
            assert_eq!(kind(path), Some(VolumeKind::Network), "{path}");
        }
    }

    // D15: an unlisted type or a disk without the `local` flag must not read as local.
    #[test]
    fn unlisted_types_and_unflagged_disks_are_unknown_not_local() {
        for path in [
            "/Volumes/fuse/vibe-tox",
            "/Volumes/NoFlag/vibe-tox",
            "/System/Volumes/Data/home/lab",
        ] {
            assert_eq!(kind(path), None, "{path}");
        }
        assert_eq!(volume_kind("", Path::new("/Users/lab")), None);
        assert_eq!(volume_kind("not a mount line", Path::new("/")), None);
    }

    #[test]
    fn filevault_status_maps_on_off_and_unknown() {
        assert_eq!(
            parse_fdesetup_status("FileVault is On.\n"),
            Some(DiskEncryption::On)
        );
        assert_eq!(
            parse_fdesetup_status("FileVault is On, but needs to be restarted to finish.\n"),
            Some(DiskEncryption::On)
        );
        assert_eq!(
            parse_fdesetup_status("FileVault is Off.\n"),
            Some(DiskEncryption::Off)
        );
        for other in [
            "",
            "FileVault is Off.\nEncryption in progress: Percent completed = 12.5\n",
            "FileVault is On.\nDecryption in progress: Percent completed = 40.0\n",
            "Error: unexpected",
        ] {
            assert_eq!(parse_fdesetup_status(other), None, "{other}");
        }
    }
}
