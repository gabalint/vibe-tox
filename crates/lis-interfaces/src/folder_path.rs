//! The usual form of a canonical Windows folder path. `fs::canonicalize` returns
//! `\\?\C:\…` and `\\?\UNC\server\share\…` on Windows; the LIS keeps and shows
//! `C:\…` and `\\server\share\…`, which name the same folder. Other paths are kept
//! as they are, so nothing changes on Linux or macOS.

const VERBATIM: &str = r"\\?\";
const VERBATIM_UNC: &str = r"\\?\UNC\";

/// `path` without the Windows verbatim prefix, when the plain form names the same
/// folder. A path whose plain form Windows would read differently keeps the prefix:
/// a `/`, a part ending in a dot or space, or a device name such as `CON`.
pub fn without_verbatim_prefix(path: &str) -> String {
    let plain = if let Some(rest) = path.strip_prefix(VERBATIM_UNC) {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(VERBATIM)
        && is_drive(rest)
    {
        rest.to_owned()
    } else {
        return path.to_owned();
    };
    let reads_the_same = !plain.contains('/')
        && plain
            .split('\\')
            .skip(1)
            .filter(|part| !part.is_empty())
            .all(plain_part);
    if reads_the_same {
        plain
    } else {
        path.to_owned()
    }
}

fn is_drive(rest: &str) -> bool {
    let bytes = rest.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

fn plain_part(part: &str) -> bool {
    if part.ends_with('.') || part.ends_with(' ') {
        return false;
    }
    let stem = part.split('.').next().unwrap_or(part).trim_end();
    let device = match stem.to_ascii_uppercase().as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        name => {
            (name.starts_with("COM") || name.starts_with("LPT"))
                && name.len() == 4
                && name.as_bytes()[3].is_ascii_digit()
        }
    };
    !device
}

#[cfg(test)]
mod tests {
    use super::without_verbatim_prefix as plain;

    #[test]
    fn drive_paths_lose_the_prefix() {
        assert_eq!(
            plain(r"\\?\C:\Users\Test\Desktop\HL7\out-hl7"),
            r"C:\Users\Test\Desktop\HL7\out-hl7"
        );
        assert_eq!(plain(r"\\?\d:\"), r"d:\");
    }

    #[test]
    fn network_paths_take_their_usual_form() {
        assert_eq!(
            plain(r"\\?\UNC\server\share\folder"),
            r"\\server\share\folder"
        );
    }

    #[test]
    fn other_paths_are_kept() {
        for path in [
            "/srv/reports/out",
            r"C:\Users\Test",
            r"\\server\share\folder",
            r"\\?\Volume{0000-1111}\out",
            r"\\?\GLOBALROOT\Device\out",
            r"\\?\C:",
            "",
        ] {
            assert_eq!(plain(path), path);
        }
    }

    #[test]
    fn paths_windows_would_read_differently_keep_the_prefix() {
        for path in [
            r"\\?\C:\out.",
            r"\\?\C:\out \in",
            r"\\?\C:\a/b",
            r"\\?\C:\CON",
            r"\\?\C:\data\nul.txt",
            r"\\?\C:\lpt1",
            r"\\?\UNC\server\share\aux",
        ] {
            assert_eq!(plain(path), path);
        }
    }

    #[test]
    fn names_that_only_look_like_devices_are_plain() {
        assert_eq!(plain(r"\\?\C:\console\com10\lpt"), r"C:\console\com10\lpt");
    }
}
