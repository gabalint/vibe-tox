//! The SFTP transport against an in-process server on localhost, whose files live in a
//! temporary folder. Synthetic keys, user and password only.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use russh::server::{Auth, ChannelOpenHandle, Msg, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{
    Attrs, Data, FileAttributes, Handle, OpenFlags, Status, StatusCode, Version,
};
use tempfile::TempDir;
use tokio::net::TcpListener;

use super::*;

const USER: &str = "vibetox_lab";
const PASSWORD: &str = "synthetic-sftp-password";

fn key(seed: u8) -> PrivateKey {
    PrivateKey::from(Ed25519Keypair::from_seed(&[seed; 32]))
}

/// A running test server: its settings as a connection would hold them, and its files.
struct Server {
    settings: SftpSettings,
    files: TempDir,
}

impl Server {
    /// Starts a server whose folder `/inbound` exists, allowing the LIS key `client` and
    /// the synthetic password.
    async fn start(client: &PrivateKey) -> Self {
        let files = TempDir::new().unwrap();
        fs::create_dir(files.path().join("inbound")).unwrap();
        let host_key = key(7);
        let fingerprint = host_key
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
        let config = Arc::new(russh::server::Config {
            keys: vec![host_key],
            auth_rejection_time: std::time::Duration::from_millis(10),
            auth_rejection_time_initial: Some(std::time::Duration::ZERO),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = files.path().to_owned();
        let allowed = client.public_key().clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let ssh = Ssh {
                    root: root.clone(),
                    allowed: allowed.clone(),
                    channels: HashMap::new(),
                };
                tokio::spawn(serve(Arc::clone(&config), socket, ssh));
            }
        });
        Self {
            settings: SftpSettings {
                host: "127.0.0.1".to_owned(),
                port,
                username: USER.to_owned(),
                auth: SftpAuth::Key,
                fingerprint,
                remote_folder: "/inbound".to_owned(),
            },
            files,
        }
    }

    fn inbound(&self) -> PathBuf {
        self.files.path().join("inbound")
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.inbound())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

/// One client's SSH session, until it ends.
async fn serve(config: Arc<russh::server::Config>, socket: tokio::net::TcpStream, ssh: Ssh) {
    if let Ok(session) = russh::server::run_stream(config, socket, ssh).await {
        let _ = session.await;
    }
}

struct Ssh {
    root: PathBuf,
    allowed: PublicKey,
    channels: HashMap<ChannelId, Channel<Msg>>,
}

impl russh::server::Handler for Ssh {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == USER && password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: VerifiedPublicKey<'_>,
    ) -> Result<Auth, Self::Error> {
        Ok(
            if user == USER && key.public_key.key_data() == self.allowed.key_data() {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match self.channels.remove(&id) {
            Some(channel) if name == "sftp" => {
                session.channel_success(id)?;
                let files = Files {
                    root: self.root.clone(),
                    open: HashMap::new(),
                    next: 0,
                };
                russh_sftp::server::run(channel.into_stream(), files).await;
            }
            _ => session.channel_failure(id)?,
        }
        Ok(())
    }
}

/// SFTP onto the temporary folder. A rename never replaces a file, as OpenSSH's.
struct Files {
    root: PathBuf,
    open: HashMap<String, File>,
    next: u32,
}

fn status(error: &std::io::Error) -> StatusCode {
    match error.kind() {
        ErrorKind::NotFound => StatusCode::NoSuchFile,
        ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_owned(),
        language_tag: "en-US".to_owned(),
    }
}

impl Files {
    fn path(&self, path: &str) -> PathBuf {
        self.root.join(path.trim_start_matches('/'))
    }

    fn file(&mut self, handle: &str) -> Result<&mut File, StatusCode> {
        self.open.get_mut(handle).ok_or(StatusCode::Failure)
    }
}

impl russh_sftp::server::Handler for Files {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = self.path(&filename);
        let file = if flags.contains(OpenFlags::WRITE) {
            OpenOptions::new()
                .write(true)
                .create_new(flags.contains(OpenFlags::EXCLUDE))
                .create(flags.contains(OpenFlags::CREATE))
                .open(path)
        } else {
            File::open(path)
        }
        .map_err(|error| status(&error))?;
        self.next += 1;
        let handle = self.next.to_string();
        self.open.insert(handle.clone(), file);
        Ok(Handle { id, handle })
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.open.remove(&handle);
        Ok(ok(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let file = self.file(&handle)?;
        let mut data = vec![0; len as usize];
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| status(&error))?;
        let read = file.read(&mut data).map_err(|error| status(&error))?;
        if read == 0 {
            return Err(StatusCode::Eof);
        }
        data.truncate(read);
        Ok(Data { id, data })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let file = self.file(&handle)?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| status(&error))?;
        file.write_all(&data).map_err(|error| status(&error))?;
        Ok(ok(id))
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let metadata = self
            .file(&handle)?
            .metadata()
            .map_err(|error| status(&error))?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let metadata = fs::metadata(self.path(&path)).map_err(|error| status(&error))?;
        Ok(Attrs {
            id,
            attrs: FileAttributes::from(&metadata),
        })
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        fs::remove_file(self.path(&filename)).map_err(|error| status(&error))?;
        Ok(ok(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        let target = self.path(&newpath);
        if target.exists() {
            return Err(StatusCode::Failure);
        }
        fs::rename(self.path(&oldpath), target).map_err(|error| status(&error))?;
        Ok(ok(id))
    }
}

fn read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

#[tokio::test]
async fn a_file_is_uploaded_whole_and_never_replaces_another() {
    let client = key(9);
    let server = Server::start(&client).await;
    let credential = Credential::Key(Arc::new(client));
    let first = upload(&server.settings, &credential, "r1.hl7", b"MSH|first")
        .await
        .unwrap();
    assert_eq!(first, "/inbound/r1.hl7");
    // The same bytes again are the earlier write of the same delivery.
    let again = upload(&server.settings, &credential, "r1.hl7", b"MSH|first")
        .await
        .unwrap();
    assert_eq!(again, first);
    // Other bytes under a taken name get the next free one.
    let other = upload(&server.settings, &credential, "r1.hl7", b"MSH|other")
        .await
        .unwrap();
    assert_eq!(other, "/inbound/r1-1.hl7");
    assert_eq!(server.names(), ["r1-1.hl7", "r1.hl7"]);
    assert_eq!(read(&server.inbound().join("r1.hl7")), b"MSH|first");
    assert_eq!(read(&server.inbound().join("r1-1.hl7")), b"MSH|other");
}

#[tokio::test]
async fn a_server_with_another_key_is_refused_and_its_fingerprint_shown() {
    let client = key(9);
    let server = Server::start(&client).await;
    let pinned = SftpSettings {
        fingerprint: key(8).public_key().fingerprint(HashAlg::Sha256).to_string(),
        ..server.settings.clone()
    };
    let refused = probe(&pinned, &Credential::Key(Arc::new(client))).await;
    assert_eq!(
        refused,
        Err(SftpProblem::FingerprintMismatch {
            presented: server.settings.fingerprint.clone()
        })
    );
    assert!(server.names().is_empty());
}

#[tokio::test]
async fn a_wrong_password_or_key_is_a_failed_sign_in() {
    let server = Server::start(&key(9)).await;
    let wrong = Credential::Password(Zeroizing::new("not-the-password".to_owned()));
    assert_eq!(
        probe(&server.settings, &wrong).await,
        Err(SftpProblem::SignInFailed)
    );
    let other_key = Credential::Key(Arc::new(key(10)));
    assert_eq!(
        probe(&server.settings, &other_key).await,
        Err(SftpProblem::SignInFailed)
    );
    let right = Credential::Password(Zeroizing::new(PASSWORD.to_owned()));
    assert_eq!(probe(&server.settings, &right).await, Ok(()));
    // The probe file is removed.
    assert!(server.names().is_empty());
}

#[tokio::test]
async fn a_missing_remote_folder_or_server_is_reported() {
    let client = key(9);
    let server = Server::start(&client).await;
    let credential = Credential::Key(Arc::new(client));
    let missing = SftpSettings {
        remote_folder: "/outbound".to_owned(),
        ..server.settings.clone()
    };
    assert_eq!(
        upload(&missing, &credential, "r1.hl7", b"MSH").await,
        Err(SftpProblem::Write(WriteProblem::FolderMissing))
    );
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let nobody = SftpSettings {
        port,
        ..server.settings.clone()
    };
    assert_eq!(
        probe(&nobody, &credential).await,
        Err(SftpProblem::Unreachable)
    );
}
