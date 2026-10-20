//! Linux probes (M4 design brief §6.1): the release from `/etc/os-release`, disk
//! encryption by walking the data folder's block device in sysfs for a device-mapper
//! `CRYPT-` target (LUKS directly, or LVM on LUKS), network file systems from the type
//! of the deepest `/proc/self/mountinfo` mount holding the folder, and NTP
//! synchronization from `timedatectl`. The parsers are pure, and anything unexpected is
//! `None` (unknown). Only the listed disk file systems count as local, including
//! `fuseblk` (a FUSE driver such as ntfs-3g on a local block device); other FUSE types
//! (`fuse`, `fuse.*`), overlay, tmpfs and any other type are unknown, and the app does
//! not start on them (D15).

use std::fs;
use std::path::Path;

#[cfg(unix)]
use super::DiskEncryption;
use super::{LinuxRelease, containing_mount, process};

const OS_RELEASE: &str = "/etc/os-release";
const MOUNTINFO: &str = "/proc/self/mountinfo";
#[cfg(unix)]
const SYS_DEV_BLOCK: &str = "/sys/dev/block";
const TIMEDATECTL: &str = "/usr/bin/timedatectl";

/// Disk file systems on locally attached storage.
const LOCAL_FS_TYPES: [&str; 14] = [
    "ext2", "ext3", "ext4", "xfs", "btrfs", "f2fs", "zfs", "jfs", "bcachefs", "vfat", "exfat",
    "ntfs", "ntfs3", "fuseblk",
];
/// Network and host-shared file systems, where SQLite WAL fails.
const NETWORK_FS_TYPES: [&str; 19] = [
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "sshfs",
    "fuse.sshfs",
    "9p",
    "virtiofs",
    "vboxsf",
    "ceph",
    "fuse.ceph",
    "glusterfs",
    "fuse.glusterfs",
    "davfs",
    "afs",
    "lustre",
    "fuse.rclone",
    "fuse.s3fs",
];
/// Device-mapper nesting deeper than this is not followed.
#[cfg(unix)]
const MAX_DEPTH: usize = 8;

/// The running release, or `None` when `/etc/os-release` cannot be read.
pub(super) fn probe_release() -> Option<LinuxRelease> {
    parse_os_release(&fs::read_to_string(OS_RELEASE).ok()?)
}

/// Whether the resolved data folder's file system sits on an encrypted device.
#[cfg(unix)]
pub(super) fn probe_encryption(data_folder: &Path) -> Option<DiskEncryption> {
    use std::os::unix::fs::MetadataExt;
    let (major, minor) = split_dev(fs::metadata(data_folder).ok()?.dev());
    device_encryption(Path::new(SYS_DEV_BLOCK), major, minor)
}

/// Whether the resolved data folder is on a network file system; `None` when unknown.
pub(super) fn probe_network_folder(data_folder: &Path) -> Option<bool> {
    is_network_mount(&fs::read_to_string(MOUNTINFO).ok()?, data_folder)
}

/// Whether systemd reports the clock synchronized.
pub(super) fn probe_time_sync() -> Option<bool> {
    parse_ntp_synchronized(&process::run(
        TIMEDATECTL,
        &["show", "-p", "NTPSynchronized", "--value"],
    )?)
}

/// `ID` and `VERSION_ID` (required) and `PRETTY_NAME` from os-release text.
pub(super) fn parse_os_release(text: &str) -> Option<LinuxRelease> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == name).then(|| unquote(value.trim()))
        })
    };
    Some(LinuxRelease {
        id: field("ID").filter(|id| !id.is_empty())?,
        version_id: field("VERSION_ID").filter(|version| !version.is_empty())?,
        pretty_name: field("PRETTY_NAME"),
    })
}

fn unquote(value: &str) -> String {
    let inner = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(value);
    inner.to_owned()
}

#[cfg(unix)]
/// The major and minor numbers of a Linux `dev_t` (glibc encoding).
pub(super) fn split_dev(dev: u64) -> (u64, u64) {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    (major, minor)
}

#[cfg(unix)]
/// Walks `<sys_dev_block>/<major>:<minor>` and its `slaves` for a `dm/uuid` starting
/// with `CRYPT-`. Major 0 (btrfs, overlay, tmpfs) and a missing device are unknown; a
/// real block device whose tree holds no crypt target is off.
pub(super) fn device_encryption(
    sys_dev_block: &Path,
    major: u64,
    minor: u64,
) -> Option<DiskEncryption> {
    if major == 0 {
        return None;
    }
    let device = sys_dev_block.join(format!("{major}:{minor}"));
    if !device.exists() {
        return None;
    }
    Some(if has_crypt_target(&device, 0) {
        DiskEncryption::On
    } else {
        DiskEncryption::Off
    })
}

#[cfg(unix)]
fn has_crypt_target(device: &Path, depth: usize) -> bool {
    let is_crypt = fs::read_to_string(device.join("dm").join("uuid"))
        .is_ok_and(|uuid| uuid.trim_start().starts_with("CRYPT-"));
    if is_crypt || depth >= MAX_DEPTH {
        return is_crypt;
    }
    let Ok(slaves) = fs::read_dir(device.join("slaves")) else {
        return false;
    };
    slaves
        .flatten()
        .any(|slave| has_crypt_target(&slave.path(), depth + 1))
}

/// Classifies the mount holding `path` (the longest mount point that contains it) by its
/// file system type; `None` when no mount matches or its type is not a listed one.
pub(super) fn is_network_mount(mountinfo: &str, path: &Path) -> Option<bool> {
    let mounts = mountinfo.lines().filter_map(|line| {
        let (mount_point, fs_type) = mount_fields(line)?;
        Some((unescape_octal(mount_point), network_fs_type(fs_type)))
    });
    containing_mount(mounts, path).flatten()
}

/// The mount point (field 5) and the file system type (after the ` - ` separator).
fn mount_fields(line: &str) -> Option<(&str, &str)> {
    let (left, right) = line.split_once(" - ")?;
    let mount_point = left.split(' ').nth(4)?;
    let fs_type = right.split(' ').next()?;
    Some((mount_point, fs_type))
}

/// `Some(true)` for a listed network type, `Some(false)` for a listed local disk type
/// and `None` (unknown) for everything else.
fn network_fs_type(fs_type: &str) -> Option<bool> {
    if NETWORK_FS_TYPES.contains(&fs_type) {
        Some(true)
    } else if LOCAL_FS_TYPES.contains(&fs_type) {
        Some(false)
    } else {
        None
    }
}

/// Decodes the `\040`-style octal escapes mountinfo uses for spaces and other bytes.
fn unescape_octal(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = bytes
            .get(index + 1..index + 4)
            .filter(|digits| bytes[index] == b'\\' && digits.iter().all(u8::is_ascii_digit))
            .and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 8).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                index += 4;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `yes` or `no` from `timedatectl show -p NTPSynchronized --value`.
pub(super) fn parse_ntp_synchronized(output: &str) -> Option<bool> {
    match output.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn os_release_gives_id_version_and_name() {
        let text = "PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nNAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\n\
                    VERSION=\"24.04.1 LTS (Noble Numbat)\"\nID=ubuntu\nID_LIKE=debian\n";
        let release = parse_os_release(text).unwrap();
        assert_eq!(
            (release.id.as_str(), release.version_id.as_str()),
            ("ubuntu", "24.04")
        );
        assert_eq!(release.pretty_name.as_deref(), Some("Ubuntu 24.04.1 LTS"));
        let rhel = "NAME='Red Hat Enterprise Linux'\nID=\"rhel\"\nVERSION_ID=\"9.4\"\n";
        assert_eq!(parse_os_release(rhel).unwrap().version_id, "9.4");
    }

    #[test]
    fn os_release_without_id_or_version_is_unknown() {
        assert_eq!(parse_os_release(""), None);
        assert_eq!(parse_os_release("ID=arch\nBUILD_ID=rolling\n"), None);
        assert_eq!(parse_os_release("VERSION_ID=\"12\"\n"), None);
        assert_eq!(parse_os_release("ID=\"\"\nVERSION_ID=\"12\"\n"), None);
    }

    #[cfg(unix)]
    #[test]
    fn dev_numbers_split_into_major_and_minor() {
        assert_eq!(split_dev(0x0803), (8, 3));
        assert_eq!(split_dev(0xfd01), (253, 1));
        assert_eq!(split_dev(0x0101_0305), (259, 0x1005));
    }

    #[cfg(unix)]
    fn device(root: &Path, name: &str, uuid: Option<&str>) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(path.join("slaves")).unwrap();
        if let Some(uuid) = uuid {
            fs::create_dir_all(path.join("dm")).unwrap();
            fs::write(path.join("dm").join("uuid"), format!("{uuid}\n")).unwrap();
        }
        path
    }

    #[cfg(unix)]
    fn link(parent: &Path, child: &Path) {
        let name = child.file_name().unwrap();
        std::os::unix::fs::symlink(child, parent.join("slaves").join(name)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn luks_directly_and_lvm_on_luks_are_encrypted() {
        let sys = tempfile::tempdir().unwrap();
        let disk = device(sys.path(), "8:2", None);
        let luks = device(sys.path(), "253:0", Some("CRYPT-LUKS2-0123abcd-synthetic"));
        link(&luks, &disk);
        let lvm = device(sys.path(), "253:1", Some("LVM-synthetic"));
        link(&lvm, &luks);
        assert_eq!(
            device_encryption(sys.path(), 253, 0),
            Some(DiskEncryption::On)
        );
        assert_eq!(
            device_encryption(sys.path(), 253, 1),
            Some(DiskEncryption::On)
        );
        assert_eq!(
            device_encryption(sys.path(), 8, 2),
            Some(DiskEncryption::Off)
        );
    }

    #[cfg(unix)]
    #[test]
    fn plain_lvm_is_off_and_virtual_or_missing_devices_are_unknown() {
        let sys = tempfile::tempdir().unwrap();
        let disk = device(sys.path(), "8:1", None);
        let lvm = device(sys.path(), "253:3", Some("LVM-synthetic"));
        link(&lvm, &disk);
        assert_eq!(
            device_encryption(sys.path(), 253, 3),
            Some(DiskEncryption::Off)
        );
        assert_eq!(device_encryption(sys.path(), 0, 42), None);
        assert_eq!(device_encryption(sys.path(), 9, 9), None);
    }

    const MOUNTINFO_TEXT: &str = "\
22 1 8:2 / / rw,relatime shared:1 - ext4 /dev/sda2 rw\n\
40 22 0:41 / /home/lab rw,relatime shared:30 - nfs4 server:/home/lab rw,vers=4.2\n\
41 40 8:3 / /home/lab/local\\040disk rw,relatime shared:31 - xfs /dev/sda3 rw\n\
42 22 0:50 / /mnt/share rw,relatime shared:32 - cifs //server/share rw\n\
43 22 0:51 / /mnt/remote rw,nosuid shared:33 - fuse.sshfs lab@server:/srv rw\n\
44 22 0:52 / /mnt/gvfs rw,nosuid shared:34 - fuse.gvfsd-fuse gvfsd-fuse rw\n\
45 22 0:53 / /mnt/odd rw shared:35 - futurefs /dev/sdb1 rw\n\
46 22 0:54 / /run/user rw shared:36 - tmpfs tmpfs rw\n\
47 22 8:17 / /media/usb rw shared:37 - fuseblk /dev/sdb1 rw\n\
48 44 8:4 / /mnt/gvfs/inner rw shared:38 - ext4 /dev/sda4 rw\n";

    fn classify(path: &str) -> Option<bool> {
        is_network_mount(MOUNTINFO_TEXT, Path::new(path))
    }

    #[test]
    fn local_disk_and_network_types_are_classified_by_the_deepest_mount() {
        assert_eq!(classify("/var/lib/vibe-tox"), Some(false));
        assert_eq!(classify("/home/lab/.local/share/vibe-tox"), Some(true));
        assert_eq!(classify("/mnt/share/data"), Some(true));
        assert_eq!(classify("/mnt/remote/vibe-tox"), Some(true));
        assert_eq!(classify("/mnt/gvfs/inner/vibe-tox"), Some(false));
    }

    #[test]
    fn octal_escaped_mount_points_match_their_decoded_path() {
        assert_eq!(classify("/home/lab/local disk/vibe-tox"), Some(false));
        assert_eq!(classify("/home/lab/local\\040disk/vibe-tox"), Some(true));
    }

    // NFR-31/NFR-32, D15: an unlisted type under a local root must not read as local.
    #[test]
    fn fuse_tmpfs_and_unlisted_types_are_unknown_not_local() {
        for path in [
            "/mnt/gvfs/vibe-tox",
            "/mnt/odd/vibe-tox",
            "/run/user/1000/vibe-tox",
        ] {
            assert_eq!(classify(path), None, "{path}");
        }
    }

    // D15: the app starts only on a local volume, so the usual Linux disks must read local.
    #[test]
    fn common_linux_disk_file_systems_are_local() {
        for fs_type in ["ext4", "xfs", "btrfs", "zfs", "ntfs3", "fuseblk"] {
            assert_eq!(network_fs_type(fs_type), Some(false), "{fs_type}");
        }
        assert_eq!(classify("/media/usb/vibe-tox"), Some(false));
    }

    #[test]
    fn missing_or_malformed_mount_tables_are_unknown() {
        assert_eq!(is_network_mount("", Path::new("/x")), None);
        assert_eq!(is_network_mount("garbage line", Path::new("/x")), None);
        let no_root = "40 22 0:41 / /home/lab rw - ext4 /dev/sda3 rw\n";
        assert_eq!(is_network_mount(no_root, Path::new("/var/lib")), None);
    }

    #[test]
    fn every_listed_type_has_one_classification() {
        for fs_type in NETWORK_FS_TYPES {
            assert_eq!(network_fs_type(fs_type), Some(true), "{fs_type}");
        }
        for fs_type in LOCAL_FS_TYPES {
            assert_eq!(network_fs_type(fs_type), Some(false), "{fs_type}");
        }
        for fs_type in ["fuse", "fuse.gvfsd-fuse", "overlay", "tmpfs", "nfsd", ""] {
            assert_eq!(network_fs_type(fs_type), None, "{fs_type}");
        }
    }

    #[test]
    fn ntp_synchronized_is_yes_no_or_unknown() {
        assert_eq!(parse_ntp_synchronized("yes\n"), Some(true));
        assert_eq!(parse_ntp_synchronized("no\n"), Some(false));
        assert_eq!(parse_ntp_synchronized(""), None);
        assert_eq!(parse_ntp_synchronized("Failed to connect to bus"), None);
    }
}
