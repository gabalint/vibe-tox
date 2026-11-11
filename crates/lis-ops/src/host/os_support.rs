//! Operating-system support table (NFR-25, M4 design brief §6.1, E14): the last day of
//! security updates per version and edition, from the vendors' lifecycle pages recorded
//! in `docs/research/sources-engineering.md` ("Operating-system support sources",
//! ENG-149 to ENG-158, checked 2026-10-13). A version outside the table is unknown,
//! never supported by default. Review the table every release.
//!
//! macOS has no published end dates: "current or two previous major versions" is a
//! heuristic that matches the versions Apple still patched on the checked date.

use jiff::civil::{Date, date};

use super::{LinuxRelease, MacosVersion, OsIdentity, WindowsVersion};

/// What the table says about one operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsSupport {
    /// The product as shown on the health screen, such as "Windows 11 Pro 24H2 (build 26100)".
    pub product: String,
    /// The support rule.
    pub status: SupportStatus,
}

/// The support rule for one operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportStatus {
    /// Security updates until this day, inclusive.
    Until(Date),
    /// A Windows Home edition: not supported for laboratory use.
    HomeEdition,
    /// Windows 10 after its end of support: only under Extended Security Updates, which
    /// the laboratory attests, and never after the last ESU year ends on `last`.
    ExtendedSecurityUpdates {
        /// The last day of the last ESU year.
        last: Date,
    },
    /// A current or one of the two previous macOS major versions (heuristic).
    MacosRecent,
    /// An older macOS major version.
    MacosOutdated,
    /// Not in the table.
    Unknown,
}

/// The support rule for `os`.
#[must_use]
pub fn assess(os: &OsIdentity) -> OsSupport {
    match os {
        OsIdentity::Windows(version) => windows(version),
        OsIdentity::Macos(version) => macos(version),
        OsIdentity::Linux(release) => linux(release),
    }
}

/// The operating system in parts, as "This computer" shows it; the support check's product
/// text joins the same parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsName {
    /// The product, such as "Windows 11", "macOS" or "Ubuntu".
    pub name: String,
    /// The release, such as "24H2 (build 26100)", "26.1" or "24.04.1 LTS".
    pub version: String,
    /// The Windows `EditionID`, such as "Professional".
    pub edition: Option<String>,
}

/// `os` split into product name, release and Windows edition.
#[must_use]
pub fn describe(os: &OsIdentity) -> OsName {
    match os {
        OsIdentity::Windows(version) => windows_name(version),
        OsIdentity::Macos(version) => OsName {
            name: "macOS".to_owned(),
            version: version.product_version.clone(),
            edition: None,
        },
        OsIdentity::Linux(release) => linux_name(release),
    }
}

/// "Windows 10" or "Windows 11" by build, the release with its build, and the edition.
fn windows_name(version: &WindowsVersion) -> OsName {
    let family = if version.build >= 22000 { 11 } else { 10 };
    OsName {
        name: format!("Windows {family}"),
        version: match &version.display_version {
            Some(display) => format!("{display} (build {})", version.build),
            None => format!("(build {})", version.build),
        },
        edition: Some(version.edition_id.clone()),
    }
}

/// `NAME` and the rest of `PRETTY_NAME` ("Ubuntu", "24.04.1 LTS"), else `ID` and `VERSION_ID`.
fn linux_name(release: &LinuxRelease) -> OsName {
    let name = release.name.clone().unwrap_or_else(|| release.id.clone());
    let version = release
        .pretty_name
        .as_deref()
        .and_then(|pretty| pretty.strip_prefix(name.as_str()))
        .map(str::trim)
        .filter(|rest| !rest.is_empty())
        .map_or_else(|| release.version_id.clone(), str::to_owned);
    OsName {
        name,
        version,
        edition: None,
    }
}

/// Windows edition classes by `EditionID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edition {
    Home,
    Pro,
    Enterprise,
    Ltsc,
    IotLtsc,
}

fn edition(edition_id: &str) -> Option<Edition> {
    let base = edition_id.strip_suffix('N').unwrap_or(edition_id);
    Some(match base {
        "Core" | "CoreSingleLanguage" | "CoreCountrySpecific" => Edition::Home,
        "Professional" | "ProfessionalWorkstation" | "ProfessionalEducation" => Edition::Pro,
        "Enterprise" | "Education" => Edition::Enterprise,
        "EnterpriseS" => Edition::Ltsc,
        "IoTEnterpriseS" => Edition::IotLtsc,
        _ => return None,
    })
}

/// Windows 11 general-availability builds: version, Home and Pro end, Enterprise and
/// Education end (Microsoft Windows release health and lifecycle pages).
const WINDOWS_11: [(u32, &str, Date, Date); 6] = [
    (22621, "22H2", date(2024, 10, 8), date(2025, 10, 14)),
    (22631, "23H2", date(2025, 11, 11), date(2026, 11, 10)),
    (26100, "24H2", date(2026, 10, 13), date(2027, 10, 12)),
    (26200, "25H2", date(2027, 10, 12), date(2028, 10, 10)),
    (26300, "26H2", date(2028, 10, 10), date(2029, 10, 9)),
    (28000, "26H1", date(2028, 3, 14), date(2029, 3, 13)),
];

/// Long-term servicing builds: LTSC end and IoT LTSC end of security updates.
const LTSC: [(u32, Date, Date); 3] = [
    (17763, date(2029, 1, 9), date(2029, 1, 9)),
    (19044, date(2027, 1, 12), date(2032, 1, 13)),
    (26100, date(2029, 10, 9), date(2034, 10, 10)),
];

/// Windows 10 22H2, whose support ended on 2025-10-14, and the last day of the third
/// commercial Extended Security Updates year.
const WINDOWS_10_BUILD: u32 = 19045;
const WINDOWS_10_ESU_LAST: Date = date(2028, 10, 10);

fn windows(version: &WindowsVersion) -> OsSupport {
    let name = windows_name(version);
    OsSupport {
        product: format!("{} {} {}", name.name, version.edition_id, name.version),
        status: windows_status(version.build, edition(&version.edition_id)),
    }
}

fn windows_status(build: u32, edition: Option<Edition>) -> SupportStatus {
    let ltsc = |pick: fn(&(u32, Date, Date)) -> Date| {
        LTSC.iter()
            .find(|row| row.0 == build)
            .map_or(SupportStatus::Unknown, |row| {
                SupportStatus::Until(pick(row))
            })
    };
    let general = |pick: fn(&(u32, &str, Date, Date)) -> Date| {
        if build == WINDOWS_10_BUILD {
            return SupportStatus::ExtendedSecurityUpdates {
                last: WINDOWS_10_ESU_LAST,
            };
        }
        WINDOWS_11
            .iter()
            .find(|row| row.0 == build)
            .map_or(SupportStatus::Unknown, |row| {
                SupportStatus::Until(pick(row))
            })
    };
    match edition {
        Some(Edition::Home) => SupportStatus::HomeEdition,
        Some(Edition::Pro) => general(|row| row.2),
        Some(Edition::Enterprise) => general(|row| row.3),
        Some(Edition::Ltsc) => ltsc(|row| row.1),
        Some(Edition::IotLtsc) => ltsc(|row| row.2),
        None => SupportStatus::Unknown,
    }
}

/// macOS major versions in release order (numbering jumped from 15 to 26 in 2025).
const MACOS_MAJORS: [u32; 7] = [11, 12, 13, 14, 15, 26, 27];
/// How many of the newest majors count as supported: the current and two previous.
const MACOS_SUPPORTED: usize = 3;

fn macos(version: &MacosVersion) -> OsSupport {
    let position = MACOS_MAJORS
        .iter()
        .position(|&major| major == version.major);
    let status = match position {
        Some(index) if index + MACOS_SUPPORTED >= MACOS_MAJORS.len() => SupportStatus::MacosRecent,
        Some(_) => SupportStatus::MacosOutdated,
        None if version.major < MACOS_MAJORS[0] => SupportStatus::MacosOutdated,
        None => SupportStatus::Unknown,
    };
    OsSupport {
        product: format!("macOS {}", version.product_version),
        status,
    }
}

/// Linux releases by `ID` and version: the end of standard security support (Ubuntu
/// standard support, end of month; Debian LTS; RHEL maintenance support).
const LINUX: [(&str, &str, Date); 8] = [
    ("ubuntu", "22.04", date(2027, 5, 31)),
    ("ubuntu", "24.04", date(2029, 5, 31)),
    ("ubuntu", "26.04", date(2031, 5, 31)),
    ("debian", "12", date(2028, 6, 30)),
    ("debian", "13", date(2030, 6, 30)),
    ("rhel", "8", date(2029, 5, 31)),
    ("rhel", "9", date(2032, 5, 31)),
    ("rhel", "10", date(2035, 5, 31)),
];

fn linux(release: &LinuxRelease) -> OsSupport {
    // RHEL minor releases (9.4) share their major release's dates.
    let version = if release.id == "rhel" {
        release.version_id.split('.').next().unwrap_or_default()
    } else {
        release.version_id.as_str()
    };
    let status = LINUX
        .iter()
        .find(|row| row.0 == release.id && row.1 == version)
        .map_or(SupportStatus::Unknown, |row| SupportStatus::Until(row.2));
    let product = release
        .pretty_name
        .clone()
        .unwrap_or_else(|| format!("{} {}", release.id, release.version_id));
    OsSupport { product, status }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(build: u32, edition: &str) -> SupportStatus {
        let version = WindowsVersion {
            build,
            display_version: None,
            edition_id: edition.to_owned(),
        };
        assess(&OsIdentity::Windows(version)).status
    }

    #[test]
    fn windows_11_dates_depend_on_the_edition() {
        assert_eq!(
            win(26100, "Professional"),
            SupportStatus::Until(date(2026, 10, 13))
        );
        assert_eq!(
            win(26100, "Enterprise"),
            SupportStatus::Until(date(2027, 10, 12))
        );
        assert_eq!(
            win(26100, "EducationN"),
            SupportStatus::Until(date(2027, 10, 12))
        );
        assert_eq!(
            win(26100, "EnterpriseS"),
            SupportStatus::Until(date(2029, 10, 9))
        );
        assert_eq!(
            win(26100, "IoTEnterpriseS"),
            SupportStatus::Until(date(2034, 10, 10))
        );
        assert_eq!(win(26200, "Core"), SupportStatus::HomeEdition);
        assert_eq!(win(19045, "CoreSingleLanguage"), SupportStatus::HomeEdition);
    }

    #[test]
    fn windows_10_needs_extended_security_updates_except_ltsc() {
        assert_eq!(
            win(19045, "Professional"),
            SupportStatus::ExtendedSecurityUpdates {
                last: date(2028, 10, 10)
            }
        );
        assert_eq!(
            win(19044, "EnterpriseS"),
            SupportStatus::Until(date(2027, 1, 12))
        );
        assert_eq!(
            win(19044, "IoTEnterpriseS"),
            SupportStatus::Until(date(2032, 1, 13))
        );
    }

    #[test]
    fn unlisted_windows_builds_and_editions_are_unknown() {
        assert_eq!(win(19044, "Professional"), SupportStatus::Unknown);
        assert_eq!(win(29999, "Professional"), SupportStatus::Unknown);
        assert_eq!(win(26100, "ServerStandard"), SupportStatus::Unknown);
        assert_eq!(win(22631, "EnterpriseS"), SupportStatus::Unknown);
    }

    #[test]
    fn windows_product_text_names_family_edition_and_build() {
        let version = WindowsVersion {
            build: 26100,
            display_version: Some("24H2".into()),
            edition_id: "Professional".into(),
        };
        assert_eq!(
            assess(&OsIdentity::Windows(version)).product,
            "Windows 11 Professional 24H2 (build 26100)"
        );
    }

    #[test]
    fn this_computer_parts_join_into_the_support_checks_product() {
        let windows = OsIdentity::Windows(WindowsVersion {
            build: 19045,
            display_version: Some("22H2".into()),
            edition_id: "Enterprise".into(),
        });
        let name = describe(&windows);
        assert_eq!(
            (name.name.as_str(), name.version.as_str()),
            ("Windows 10", "22H2 (build 19045)")
        );
        assert_eq!(name.edition.as_deref(), Some("Enterprise"));
        assert_eq!(
            assess(&windows).product,
            "Windows 10 Enterprise 22H2 (build 19045)"
        );
        let old = describe(&OsIdentity::Windows(WindowsVersion {
            build: 17763,
            display_version: None,
            edition_id: "EnterpriseS".into(),
        }));
        assert_eq!(old.version, "(build 17763)");
        let mac = describe(&OsIdentity::Macos(MacosVersion {
            product_version: "26.1".into(),
            major: 26,
        }));
        assert_eq!((mac.name.as_str(), mac.version.as_str()), ("macOS", "26.1"));
        assert_eq!(mac.edition, None);
    }

    #[test]
    fn linux_parts_split_the_pretty_name_after_the_name() {
        let release = |name: Option<&str>, pretty: Option<&str>| {
            describe(&OsIdentity::Linux(LinuxRelease {
                id: "ubuntu".into(),
                version_id: "24.04".into(),
                name: name.map(Into::into),
                pretty_name: pretty.map(Into::into),
            }))
        };
        let full = release(Some("Ubuntu"), Some("Ubuntu 24.04.1 LTS"));
        assert_eq!(
            (full.name.as_str(), full.version.as_str()),
            ("Ubuntu", "24.04.1 LTS")
        );
        let other = release(Some("Ubuntu"), Some("Custom build"));
        assert_eq!(other.version, "24.04");
        let bare = release(None, None);
        assert_eq!(
            (bare.name.as_str(), bare.version.as_str()),
            ("ubuntu", "24.04")
        );
    }

    fn mac(major: u32) -> SupportStatus {
        let version = MacosVersion {
            product_version: format!("{major}.1"),
            major,
        };
        assess(&OsIdentity::Macos(version)).status
    }

    #[test]
    fn macos_counts_the_current_and_two_previous_majors_as_recent() {
        for major in [15, 26, 27] {
            assert_eq!(mac(major), SupportStatus::MacosRecent, "{major}");
        }
        for major in [10, 11, 14] {
            assert_eq!(mac(major), SupportStatus::MacosOutdated, "{major}");
        }
        assert_eq!(mac(28), SupportStatus::Unknown);
        assert_eq!(mac(20), SupportStatus::Unknown);
    }

    fn lin(id: &str, version_id: &str) -> SupportStatus {
        let release = LinuxRelease {
            id: id.into(),
            version_id: version_id.into(),
            name: None,
            pretty_name: None,
        };
        assess(&OsIdentity::Linux(release)).status
    }

    #[test]
    fn linux_releases_use_their_security_support_end() {
        assert_eq!(
            lin("ubuntu", "24.04"),
            SupportStatus::Until(date(2029, 5, 31))
        );
        assert_eq!(lin("debian", "12"), SupportStatus::Until(date(2028, 6, 30)));
        assert_eq!(lin("rhel", "9.4"), SupportStatus::Until(date(2032, 5, 31)));
        assert_eq!(lin("ubuntu", "25.10"), SupportStatus::Unknown);
        assert_eq!(lin("arch", "rolling"), SupportStatus::Unknown);
        assert_eq!(lin("debian", "12.7"), SupportStatus::Unknown);
    }
}
