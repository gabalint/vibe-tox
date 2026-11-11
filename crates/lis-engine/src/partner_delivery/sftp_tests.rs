//! The SFTP transport against the in-process test server. Synthetic keys, user and password
//! only.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use russh::keys::HashAlg;
use tokio::net::TcpListener;

use super::test_server::{PASSWORD, Server, key};
use super::*;

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
