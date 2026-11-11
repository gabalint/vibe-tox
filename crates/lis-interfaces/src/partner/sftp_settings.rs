//! Checks on an SFTP connection's server settings, and the destination a destination
//! check covers. Pure.

use lis_core::required_text;

use super::error::{PartnerError, invalid};
use super::mllp_listener_settings::listener_destination;
use super::mllp_settings::mllp_destination;
use super::model::{PartnerConnectionSettings, PartnerProtocol, SftpSettings};
use crate::validation::valid_text;

const HOST_MAX: usize = 253;
const USERNAME_MAX: usize = 64;
const REMOTE_FOLDER_MAX: usize = 1024;
/// `SHA256:` and 43 unpadded base64 characters, as OpenSSH prints a key fingerprint.
const FINGERPRINT_DIGEST: usize = 43;

/// The protocol's server settings, trimmed: present for SFTP, absent otherwise.
pub(crate) fn valid_sftp(
    protocol: PartnerProtocol,
    sftp: Option<SftpSettings>,
) -> Result<Option<SftpSettings>, PartnerError> {
    match (protocol, sftp) {
        (PartnerProtocol::Sftp, Some(sftp)) => valid_sftp_server(sftp).map(Some),
        (PartnerProtocol::Sftp, None) => Err(invalid(
            "settings.sftp.host",
            "required",
            "Enter the SFTP server.",
        )),
        (_, None) => Ok(None),
        (_, Some(_)) => Err(invalid(
            "settings.sftp",
            "not_for_protocol",
            "Only an SFTP connection has a server.",
        )),
    }
}

/// The server settings trimmed, or the first field that is wrong; a server test checks them
/// as saving does.
pub fn valid_sftp_server(sftp: SftpSettings) -> Result<SftpSettings, PartnerError> {
    let host =
        required_text("settings.sftp.host", &sftp.host, HOST_MAX).map_err(PartnerError::Invalid)?;
    if host.contains(|c: char| c.is_whitespace() || c == '/' || c == '@') {
        return Err(invalid(
            "settings.sftp.host",
            "invalid_host",
            "Enter a server name or address only, such as sftp.clinic-a.example.",
        ));
    }
    if sftp.port == 0 {
        return Err(invalid(
            "settings.sftp.port",
            "out_of_range",
            "Use a port from 1 to 65535.",
        ));
    }
    let username = required_text("settings.sftp.username", &sftp.username, USERNAME_MAX)
        .map_err(PartnerError::Invalid)?;
    let fingerprint = sftp.fingerprint.trim().to_owned();
    if !is_fingerprint(&fingerprint) {
        return Err(invalid(
            "settings.sftp.fingerprint",
            "invalid_fingerprint",
            "Enter the server's SHA256 fingerprint as the partner's IT gives it, such as SHA256:8Zq3…",
        ));
    }
    let remote_folder = valid_remote_folder("settings.sftp.remoteFolder", &sftp.remote_folder)?;
    Ok(SftpSettings {
        host,
        username,
        fingerprint,
        remote_folder,
        ..sftp
    })
}

/// A folder on the server, trimmed: from its root, without `..`.
pub(crate) fn valid_remote_folder(field: &str, folder: &str) -> Result<String, PartnerError> {
    let folder = folder.trim().to_owned();
    if valid_text(&folder, REMOTE_FOLDER_MAX)
        && folder.starts_with('/')
        && !folder.split('/').any(|part| part == "..")
    {
        Ok(folder)
    } else {
        Err(invalid(
            field,
            "invalid_folder",
            "Enter the folder on the server from its root, such as /inbound/results.",
        ))
    }
}

/// `SHA256:` followed by 43 standard base64 characters without padding.
fn is_fingerprint(text: &str) -> bool {
    text.strip_prefix("SHA256:").is_some_and(|digest| {
        digest.len() == FINGERPRINT_DIGEST
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/')
    })
}

/// What a destination check covers: the folder, every setting that decides which SFTP
/// server and folder receive the files, or the MLLP partner's address and port. Changing
/// any of them needs a new check.
pub(crate) fn destination_of(settings: &PartnerConnectionSettings, folder: &str) -> String {
    if let Some(partner) = &settings.mllp {
        return mllp_destination(partner);
    }
    if let Some(listener) = &settings.mllp_listener {
        return listener_destination(listener);
    }
    match &settings.sftp {
        Some(sftp) => format!(
            "sftp://{}@{}:{}{} {}",
            sftp.username, sftp.host, sftp.port, sftp.remote_folder, sftp.fingerprint
        ),
        None => folder.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::SftpAuth;

    fn server() -> SftpSettings {
        SftpSettings {
            host: " sftp.clinic-a.example ".to_owned(),
            port: 22,
            username: "vibetox_lab".to_owned(),
            auth: SftpAuth::Key,
            fingerprint: format!("SHA256:{}", "A".repeat(43)),
            remote_folder: "/inbound/results".to_owned(),
        }
    }

    #[test]
    fn a_server_is_trimmed_and_checked() {
        let valid = valid_sftp(PartnerProtocol::Sftp, Some(server()))
            .unwrap()
            .unwrap();
        assert_eq!(valid.host, "sftp.clinic-a.example");
        assert!(valid_sftp(PartnerProtocol::Folder, Some(server())).is_err());
        assert!(valid_sftp(PartnerProtocol::Sftp, None).is_err());
        for broken in [
            SftpSettings {
                fingerprint: "SHA256:short".to_owned(),
                ..server()
            },
            SftpSettings {
                remote_folder: "inbound".to_owned(),
                ..server()
            },
            SftpSettings {
                remote_folder: "/inbound/../etc".to_owned(),
                ..server()
            },
            SftpSettings {
                host: "user@host".to_owned(),
                ..server()
            },
            SftpSettings {
                port: 0,
                ..server()
            },
        ] {
            assert!(valid_sftp(PartnerProtocol::Sftp, Some(broken)).is_err());
        }
    }
}
