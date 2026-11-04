//! Writing to a partner's SFTP server (ticket #4555 step 3; REQ-RPT-012, REQ-INT-006). The
//! LIS only connects outward. It accepts the server only when its key matches the pinned
//! SHA-256 fingerprint, signs in with its own key or the sealed password, writes each file
//! under a `.tmp` name and renames it only when complete, and never replaces a file: a name
//! taken by other content gets `-1`, `-2`, …, and a name already holding these exact bytes
//! is an earlier write of the same delivery. Codes are PHI-free; nothing here logs a path.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use lis_auth::DataKey;
use lis_interfaces::partner::{SftpAuth, SftpSettings};
use lis_store::Database;
use russh::client::{self, Handle};
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh_sftp::client::SftpSession;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::protocol::{OpenFlags, StatusCode};
use tokio::io::AsyncWriteExt;
use zeroize::Zeroizing;

use super::folder_write::{WriteProblem, random_suffix};

/// The sealing context of a connection's password.
pub(crate) const PASSWORD_CONTEXT: &str = "ifc_partner_connections.sftp_password";
/// The sealing context of the LIS's private SSH key.
pub(crate) const KEY_CONTEXT: &str = "ifc_sftp_identity.private_key";
/// How long connecting, and then each step without an answer, may take.
const TIMEOUT: Duration = Duration::from_secs(20);
/// Names tried before giving up on a free one, as in a folder.
const MAX_SAME_NAME: u32 = 100;

/// How the LIS signs in.
pub(crate) enum Credential {
    Password(Zeroizing<String>),
    Key(Arc<PrivateKey>),
}

/// Why nothing was written to the server; the codes show in Logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SftpProblem {
    /// The server did not answer in time, or refused the connection.
    Unreachable,
    /// The server's key is not the pinned one; it presented `presented`.
    FingerprintMismatch { presented: String },
    /// The server refused the user's key or password.
    SignInFailed,
    /// The password or the LIS's key is missing, or cannot be opened with the
    /// installation key.
    CredentialUnavailable,
    /// A folder-level failure, as for a local folder.
    Write(WriteProblem),
}

impl SftpProblem {
    /// The stored and displayed code.
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Unreachable => "server_unreachable",
            Self::FingerprintMismatch { .. } => "fingerprint_mismatch",
            Self::SignInFailed => "sign_in_failed",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::Write(problem) => problem.code(),
        }
    }
}

/// The sign-in for `auth`: the connection's sealed password, or the LIS's sealed key,
/// opened with the installation key. Blocking: the key is read from the database.
pub(crate) fn credential(
    db: &Database,
    key: Option<&DataKey>,
    auth: SftpAuth,
    sealed_password: Option<&[u8]>,
) -> Result<Credential, SftpProblem> {
    let unavailable = SftpProblem::CredentialUnavailable;
    let key = key.ok_or(unavailable.clone())?;
    let opened = match auth {
        SftpAuth::Password => {
            let sealed = sealed_password.ok_or(unavailable.clone())?;
            lis_auth::open_secret(key, PASSWORD_CONTEXT, sealed)
        }
        SftpAuth::Key => {
            let sealed = db
                .read_blocking(lis_interfaces::partner::sealed_sftp_key)
                .ok()
                .flatten()
                .ok_or(unavailable.clone())?;
            lis_auth::open_secret(key, KEY_CONTEXT, &sealed)
        }
    }
    .map_err(|_| unavailable.clone())?;
    let text = std::str::from_utf8(&opened).map_err(|_| unavailable.clone())?;
    match auth {
        SftpAuth::Password => Ok(Credential::Password(Zeroizing::new(text.to_owned()))),
        SftpAuth::Key => PrivateKey::from_openssh(text)
            .map(|key| Credential::Key(Arc::new(key)))
            .map_err(|_| unavailable),
    }
}

/// Accepts only the server whose key has the pinned fingerprint, and keeps the one it saw.
struct Pinned {
    fingerprint: String,
    presented: Arc<Mutex<Option<String>>>,
}

impl client::Handler for Pinned {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let presented = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.fingerprint(HashAlg::Sha256),
            PublicKeyOrCertificate::Certificate(certificate) => {
                certificate.public_key().fingerprint(HashAlg::Sha256)
            }
        }
        .to_string();
        let matches = presented == self.fingerprint;
        if let Ok(mut seen) = self.presented.lock() {
            *seen = Some(presented);
        }
        Ok(matches)
    }
}

/// A signed-in SFTP session; the SSH connection lives as long as it.
struct Open {
    _ssh: Handle<Pinned>,
    sftp: SftpSession,
}

async fn open(server: &SftpSettings, credential: &Credential) -> Result<Open, SftpProblem> {
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(TIMEOUT),
        ..client::Config::default()
    });
    let presented = Arc::new(Mutex::new(None));
    let handler = Pinned {
        fingerprint: server.fingerprint.clone(),
        presented: Arc::clone(&presented),
    };
    let address = (server.host.as_str(), server.port);
    let connected = tokio::time::timeout(TIMEOUT, client::connect(config, address, handler)).await;
    let Ok(Ok(mut ssh)) = connected else {
        let seen = presented.lock().ok().and_then(|seen| seen.clone());
        return Err(match seen {
            Some(presented) if presented != server.fingerprint => {
                SftpProblem::FingerprintMismatch { presented }
            }
            Some(_) | None => SftpProblem::Unreachable,
        });
    };
    let user = server.username.clone();
    let signed_in = match credential {
        Credential::Password(password) => ssh.authenticate_password(user, password.as_str()).await,
        Credential::Key(key) => {
            let key = PrivateKeyWithHashAlg::new(Arc::clone(key), None);
            ssh.authenticate_publickey(user, key).await
        }
    };
    if !signed_in.is_ok_and(|result| result.success()) {
        return Err(SftpProblem::SignInFailed);
    }
    let unreachable = |_| SftpProblem::Unreachable;
    let channel = ssh.channel_open_session().await.map_err(unreachable)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(unreachable)?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|_| SftpProblem::Unreachable)?;
    Ok(Open { _ssh: ssh, sftp })
}

/// Writes `bytes` as `name` in the server's folder and returns the path it got.
pub(crate) async fn upload(
    server: &SftpSettings,
    credential: &Credential,
    name: &str,
    bytes: &[u8],
) -> Result<String, SftpProblem> {
    let open = open(server, credential).await?;
    let folder = folder_of(&open.sftp, server).await?;
    let temporary = format!("{folder}/{name}.{}.tmp", random_suffix());
    if let Err(error) = fill(&open.sftp, &temporary, bytes).await {
        let _ = open.sftp.remove_file(temporary).await;
        return Err(problem(&error));
    }
    let placed = place(&open.sftp, &folder, name, &temporary, bytes).await;
    if placed.is_err() {
        let _ = open.sftp.remove_file(temporary).await;
    }
    placed
}

/// Connects, checks the fingerprint and the sign-in, then writes and removes a small probe
/// file: whether the LIS can create, write and delete files in the server's folder.
pub(crate) async fn probe(
    server: &SftpSettings,
    credential: &Credential,
) -> Result<(), SftpProblem> {
    let open = open(server, credential).await?;
    let folder = folder_of(&open.sftp, server).await?;
    let probe = format!("{folder}/.vibetox-probe-{}.tmp", random_suffix());
    let written = fill(&open.sftp, &probe, b"probe\n").await;
    let removed = open.sftp.remove_file(probe).await;
    written.and(removed).map_err(|error| problem(&error))
}

/// The remote folder without a trailing `/`, once it exists.
async fn folder_of(sftp: &SftpSession, server: &SftpSettings) -> Result<String, SftpProblem> {
    let folder = server.remote_folder.trim_end_matches('/').to_owned();
    let path = if folder.is_empty() { "/" } else { &folder };
    match sftp.metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(folder),
        Ok(_) => Err(SftpProblem::Write(WriteProblem::FolderMissing)),
        Err(error) => Err(problem(&error)),
    }
}

async fn fill(sftp: &SftpSession, path: &str, bytes: &[u8]) -> Result<(), SftpError> {
    let flags = OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE;
    let mut file = sftp.open_with_flags(path, flags).await?;
    file.write_all(bytes).await?;
    file.shutdown().await?;
    Ok(())
}

/// Gives the complete temporary file its final name without replacing a file.
async fn place(
    sftp: &SftpSession,
    folder: &str,
    name: &str,
    temporary: &str,
    bytes: &[u8],
) -> Result<String, SftpProblem> {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    for n in 0..MAX_SAME_NAME {
        let target = if n == 0 {
            format!("{folder}/{name}")
        } else {
            format!("{folder}/{stem}-{n}.{extension}")
        };
        match sftp.read(target.as_str()).await {
            Ok(existing) if existing == bytes => {
                let _ = sftp.remove_file(temporary).await;
                return Ok(target);
            }
            Ok(_) => continue,
            Err(error) if missing(&error) => {}
            Err(error) => return Err(problem(&error)),
        }
        match sftp.rename(temporary, target.as_str()).await {
            Ok(()) => return Ok(target),
            // An SFTP rename never replaces a file: a name taken meanwhile is tried again.
            Err(_) if sftp.try_exists(target.as_str()).await.unwrap_or(false) => {}
            Err(error) => return Err(problem(&error)),
        }
    }
    Err(SftpProblem::Write(WriteProblem::NameTaken))
}

fn missing(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

fn problem(error: &SftpError) -> SftpProblem {
    SftpProblem::Write(match error {
        SftpError::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => WriteProblem::FolderMissing,
            StatusCode::PermissionDenied => WriteProblem::AccessDenied,
            _ => WriteProblem::WriteFailed,
        },
        SftpError::IO(_) | SftpError::Timeout => return SftpProblem::Unreachable,
        _ => WriteProblem::WriteFailed,
    })
}
