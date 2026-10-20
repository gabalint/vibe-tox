//! Windows probes (M4 design brief §6.1): the version from the registry through
//! `reg.exe`, BitLocker protection of the data drive through the shell's
//! `System.Volume.BitLockerProtection` property (no administrator rights), and network
//! data folders: UNC paths, and drive letters (mapped drives included) whose .NET
//! `DriveInfo.DriveType` is `Network`. Only `Fixed` and `Removable` drives count as
//! local. The parsers are pure, and anything unexpected is `None` (unknown).

use super::{DiskEncryption, WindowsVersion, process};

const REG: &str = r"C:\Windows\System32\reg.exe";
const POWERSHELL: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
const CURRENT_VERSION_KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";

/// The running Windows version, or `None` when the registry cannot be read.
pub(super) fn probe_version() -> Option<WindowsVersion> {
    parse_reg_query(&process::run(REG, &["query", CURRENT_VERSION_KEY])?)
}

/// BitLocker protection of the drive holding the resolved data folder.
pub(super) fn probe_encryption(data_folder: &str) -> Option<DiskEncryption> {
    let drive = drive_letter(data_folder)?;
    parse_bitlocker_protection(&powershell(&format!(
        "(New-Object -ComObject Shell.Application).NameSpace('{drive}:').Self\
         .ExtendedProperty('System.Volume.BitLockerProtection')"
    ))?)
}

/// Whether the resolved data folder is on network storage: a UNC path, or a drive letter
/// (a mapped drive in this user's session included) whose type is `Network`.
pub(super) fn probe_network_folder(data_folder: &str) -> Option<bool> {
    if is_unc_path(data_folder) {
        return Some(true);
    }
    let drive = drive_letter(data_folder)?;
    parse_drive_type(&powershell(&format!(
        "[System.IO.DriveInfo]::new('{drive}:').DriveType"
    ))?)
}

/// Runs one fixed PowerShell command; only a drive letter is ever interpolated into it.
fn powershell(command: &str) -> Option<String> {
    process::run(
        POWERSHELL,
        &["-NoProfile", "-NonInteractive", "-Command", command],
    )
}

/// The drive letter of a `C:\…` or `\\?\C:\…` (canonical) path; `None` for UNC and
/// volume GUID paths. The letter is the only part of the path that reaches a command.
fn drive_letter(path: &str) -> Option<char> {
    let path = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\\.\"))
        .unwrap_or(path);
    let mut chars = path.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    (chars.next() == Some(':')).then_some(letter.to_ascii_uppercase())
}

/// The value of a `REG_SZ` named `name` in `reg.exe query` output.
fn reg_value<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        if fields.next()? != name || fields.next()? != "REG_SZ" {
            return None;
        }
        fields.next()
    })
}

/// `CurrentBuild`, `DisplayVersion` and `EditionID` from `reg.exe query` output.
pub(super) fn parse_reg_query(output: &str) -> Option<WindowsVersion> {
    Some(WindowsVersion {
        build: reg_value(output, "CurrentBuild")?.parse().ok()?,
        display_version: reg_value(output, "DisplayVersion").map(str::to_owned),
        edition_id: reg_value(output, "EditionID")?.to_owned(),
    })
}

/// The shell's BitLocker protection value: 1 is protected and 2 is not protected. Other
/// values (encrypting, decrypting, suspended, locked) and anything unexpected are
/// unknown; the mapping is verified on real hosts in M5.
pub(super) fn parse_bitlocker_protection(output: &str) -> Option<DiskEncryption> {
    match output.trim() {
        "1" => Some(DiskEncryption::On),
        "2" => Some(DiskEncryption::Off),
        _ => None,
    }
}

/// `DriveInfo.DriveType`: `Network` is network storage, `Fixed` and `Removable` are
/// local; `Unknown`, `NoRootDirectory`, `CDRom`, `Ram` and anything else are unknown.
pub(super) fn parse_drive_type(output: &str) -> Option<bool> {
    match output.trim() {
        "Network" => Some(true),
        "Fixed" | "Removable" => Some(false),
        _ => None,
    }
}

/// True for a UNC path (`\\server\share`, `\\?\UNC\…`), which is a network location.
/// Local device paths such as `\\?\C:\…` are not.
pub(crate) fn is_unc_path(path: &str) -> bool {
    let upper = path.to_ascii_uppercase();
    if let Some(rest) = upper.strip_prefix(r"\\?\").or(upper.strip_prefix(r"\\.\")) {
        return rest.starts_with(r"UNC\");
    }
    upper.starts_with(r"\\") || upper.starts_with("//")
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG_OUTPUT: &str = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\r\n    \
        SystemRoot    REG_SZ    C:\\WINDOWS\r\n    CurrentBuild    REG_SZ    26100\r\n    \
        CurrentBuildNumber    REG_SZ    26100\r\n    DisplayVersion    REG_SZ    24H2\r\n    \
        EditionID    REG_SZ    Professional\r\n    ProductName    REG_SZ    Windows 10 Pro\r\n    \
        UBR    REG_DWORD    0x1a2b\r\n";

    #[test]
    fn the_registry_output_gives_build_display_version_and_edition() {
        let version = parse_reg_query(REG_OUTPUT).unwrap();
        assert_eq!(version.build, 26100);
        assert_eq!(version.display_version.as_deref(), Some("24H2"));
        assert_eq!(version.edition_id, "Professional");
    }

    #[test]
    fn incomplete_or_malformed_registry_output_is_unknown() {
        assert_eq!(parse_reg_query(""), None);
        assert_eq!(
            parse_reg_query("    CurrentBuild    REG_SZ    abc\n    EditionID    REG_SZ    Core"),
            None
        );
        let no_edition = "    CurrentBuild    REG_SZ    19045\n";
        assert_eq!(parse_reg_query(no_edition), None);
        let old = "    CurrentBuild    REG_SZ    17763\n    EditionID    REG_SZ    EnterpriseS\n";
        assert_eq!(parse_reg_query(old).unwrap().display_version, None);
    }

    #[test]
    fn bitlocker_values_map_only_protected_and_unprotected() {
        assert_eq!(
            parse_bitlocker_protection("1\r\n"),
            Some(DiskEncryption::On)
        );
        assert_eq!(parse_bitlocker_protection("2"), Some(DiskEncryption::Off));
        for other in ["", "0", "3", "5", "6", "On", "Access denied"] {
            assert_eq!(parse_bitlocker_protection(other), None, "{other}");
        }
    }

    #[test]
    fn drive_letters_come_from_plain_and_canonical_drive_paths() {
        assert_eq!(drive_letter(r"c:\Users\Lab\AppData\Local"), Some('C'));
        assert_eq!(drive_letter(r"\\?\d:\VibeTox"), Some('D'));
        assert_eq!(drive_letter(r"\\.\Z:\VibeTox"), Some('Z'));
        for other in [
            r"\\server\share",
            r"\\?\UNC\server\share",
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\VibeTox",
            "1:\\x",
            "C",
        ] {
            assert_eq!(drive_letter(other), None, "{other}");
        }
    }

    #[test]
    fn mapped_network_drives_are_network_and_fixed_or_removable_are_local() {
        assert_eq!(parse_drive_type("Network\r\n"), Some(true));
        assert_eq!(parse_drive_type("Fixed\r\n"), Some(false));
        assert_eq!(parse_drive_type("Removable"), Some(false));
    }

    // D15: a drive type the probe cannot name must not read as local.
    #[test]
    fn unknown_and_other_drive_types_are_unknown_not_local() {
        for other in [
            "",
            "Unknown",
            "NoRootDirectory",
            "CDRom",
            "Ram",
            "4",
            "Access denied",
        ] {
            assert_eq!(parse_drive_type(other), None, "{other}");
        }
    }

    #[test]
    fn unc_paths_are_network_locations() {
        for network in [
            r"\\server\share\VibeTox",
            r"\\?\UNC\server\share",
            "//server/share",
        ] {
            assert!(is_unc_path(network), "{network}");
        }
        for local in [
            r"C:\Users\Lab",
            r"\\?\C:\Users\Lab",
            r"\\.\C:\x",
            "relative",
        ] {
            assert!(!is_unc_path(local), "{local}");
        }
    }
}
