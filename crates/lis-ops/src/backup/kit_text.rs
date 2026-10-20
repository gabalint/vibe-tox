//! The `vtkit-1` recovery kit text and its `vtk1-` key line (M4 design brief §3.3): the
//! one home of both. Only the key line holds key material; every text holding it is wiped
//! from memory when dropped.

use std::path::Path;

use lis_auth::DataKey;
use lis_core::UtcTimestamp;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::BackupError;
use crate::format::{BackupKey, KeyId};
use crate::targets::Target;

const KIT_FORMAT: &str = "vtkit-1";
const KEY_PREFIX: &str = "vtk1-";
const KEY_GROUPS: usize = 16;
const GROUP_CHARS: usize = 4;

/// What the kit text names besides the key and the locations.
pub(crate) struct KitHeader<'a> {
    pub(crate) installation: String,
    pub(crate) kit_no: i64,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) created_by_name: &'a str,
    pub(crate) operating_system: &'a str,
    pub(crate) data_dir: &'a Path,
    pub(crate) reference: &'a str,
}

/// The key line's value: `vtk1-` then the 64 hex digits of the key in 16 groups of 4,
/// then the first 4 hex digits of SHA-256(key) as a typing checksum.
#[must_use]
pub fn key_text(key: &DataKey) -> Zeroizing<String> {
    let digits = Zeroizing::new(hex::encode(key.expose()));
    let mut text = Zeroizing::new(String::from(KEY_PREFIX));
    for (index, group) in digits.as_bytes().chunks(GROUP_CHARS).enumerate() {
        if index > 0 {
            text.push('-');
        }
        text.extend(group.iter().map(|&byte| char::from(byte)));
    }
    text.push('-');
    text.push_str(&checksum(key));
    text
}

/// Parses a key typed or pasted from a kit's key line (restore, §3.7). Accepts upper or
/// lower case and surrounding spaces, nothing else: a wrong structure is
/// [`BackupError::KeyFormat`], a wrong checksum (a typing error) [`BackupError::KeyChecksum`].
pub fn parse_key(text: &str) -> Result<DataKey, BackupError> {
    let text = Zeroizing::new(text.trim().to_ascii_lowercase());
    let groups = text
        .strip_prefix(KEY_PREFIX)
        .ok_or(BackupError::KeyFormat)?;
    let groups: Vec<&str> = groups.split('-').collect();
    let well_formed = groups.len() == KEY_GROUPS + 1
        && groups.iter().all(|group| {
            group.len() == GROUP_CHARS && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    if !well_formed {
        return Err(BackupError::KeyFormat);
    }
    let digits = Zeroizing::new(groups[..KEY_GROUPS].concat());
    let mut bytes = Zeroizing::new([0_u8; 32]);
    hex::decode_to_slice(digits.as_bytes(), bytes.as_mut_slice())
        .map_err(|_| BackupError::KeyFormat)?;
    let key = DataKey::from_bytes(*bytes);
    if checksum(&key) != groups[KEY_GROUPS] {
        return Err(BackupError::KeyChecksum);
    }
    Ok(key)
}

fn checksum(key: &DataKey) -> String {
    hex::encode(&Sha256::digest(key.expose())[..2])
}

/// The key ID in groups of four, for reading aloud and comparing with the app.
fn fingerprint(key_id: KeyId) -> String {
    let digits = key_id.to_string();
    let groups: Vec<&str> = digits
        .as_bytes()
        .chunks(GROUP_CHARS)
        .filter_map(|group| std::str::from_utf8(group).ok())
        .collect();
    groups.join("-")
}

const RESTORE_STEPS: &str = "\
Restore steps (on a replacement computer):
1. Install the same or a newer version of Vibe Toxicology LIS and connect one of the
   backup locations listed above.
2. Open the app. On the setup screen, before creating any user, choose to restore
   from a backup.
3. Enter the installation key exactly as printed above.
4. Choose the backup location and the newest backup set, and wait for its check.
5. Confirm the restore, then open the app again to finish.
6. Sign in and work through the recovery reconciliation before releasing or
   delivering reports.
";

/// The `vtkit-1` text. Only the key line holds key material, and the text is wiped
/// from memory when dropped.
pub(crate) fn kit_text(
    key: &DataKey,
    header: &KitHeader<'_>,
    locations: &[Target],
) -> Zeroizing<String> {
    let mut text = Zeroizing::new(String::new());
    for line in [
        "Vibe Toxicology LIS recovery kit".to_owned(),
        format!("Format: {KIT_FORMAT}"),
        String::new(),
        "Anyone with this file can read your backups. Store it like a disk-encryption \
         recovery key, in two places off this computer."
            .to_owned(),
        String::new(),
        format!("Installation ID: {}", header.installation),
        format!("Kit number: {}", header.kit_no),
        format!("Created at: {}", header.created_at),
        format!("Created by: {}", header.created_by_name),
        format!("App version: {}", env!("CARGO_PKG_VERSION")),
        format!(
            "Platform: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        format!("Operating system: {}", header.operating_system),
        format!("Data folder: {}", header.data_dir.display()),
        String::new(),
    ] {
        push_line(&mut text, &line);
    }
    text.push_str("Installation key: ");
    text.push_str(&key_text(key));
    text.push('\n');
    let key_id = BackupKey::derive(key).key_id();
    push_line(
        &mut text,
        &format!("Key fingerprint: {}", fingerprint(key_id)),
    );
    push_line(&mut text, "");
    push_line(
        &mut text,
        &format!("Disk-encryption recovery key: {}", header.reference),
    );
    push_line(&mut text, "");
    push_line(&mut text, "Backup locations at export time:");
    if locations.is_empty() {
        push_line(&mut text, "- none");
    }
    for target in locations {
        push_line(
            &mut text,
            &format!(
                "- {} ({}, {}): {}",
                target.label,
                target.kind.as_str(),
                target.purpose.as_str(),
                target.path.display()
            ),
        );
    }
    push_line(&mut text, "");
    text.push_str(RESTORE_STEPS);
    push_line(&mut text, "");
    push_line(
        &mut text,
        "Cloud re-enrollment: not applicable in this version",
    );
    push_line(
        &mut text,
        "Break-glass envelope: not applicable in this version",
    );
    text
}

fn push_line(text: &mut String, line: &str) {
    text.push_str(line);
    text.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_text_round_trips_and_has_the_documented_shape() {
        let key = DataKey::from_bytes(std::array::from_fn(|i| i as u8));
        let text = key_text(&key);
        assert_eq!(text.len(), 5 + 16 * 5 + 4);
        assert_eq!(
            text.as_str(),
            "vtk1-0001-0203-0405-0607-0809-0a0b-0c0d-0e0f-1011-1213-1415-1617-1819-1a1b-1c1d-1e1f-630d"
        );
        assert_eq!(parse_key(&text).unwrap().expose(), key.expose());
        assert_eq!(
            parse_key(&format!("  {}  ", text.to_uppercase()))
                .unwrap()
                .expose(),
            key.expose()
        );
    }
}
