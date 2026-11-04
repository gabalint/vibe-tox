//! Orders read from an SFTP input folder (ticket #4555 step 3c): one session lists the
//! regular files, reads one, and moves read files to Processed or Failed › year › month
//! without replacing a file there. Against the in-process test server; synthetic files only.

use std::fs;
use std::sync::Arc;

use lis_interfaces::partner::MovedTo;

use super::test_server::{Server, key};
use super::*;

/// The server's Processed and Failed folders, as an HL7 Orders connection names them.
fn moved_to() -> MovedTo {
    MovedTo {
        processed: "/processed".to_owned(),
        failed: "/failed".to_owned(),
    }
}

/// Runs `steps` on a session opened as the intake worker opens it, off the runtime.
async fn with_inbox<T: Send + 'static>(
    settings: &SftpSettings,
    steps: impl FnOnce(&mut RemoteInbox) -> Result<T, SftpProblem> + Send + 'static,
) -> Result<T, SftpProblem> {
    let settings = settings.clone();
    tokio::task::spawn_blocking(move || {
        let credential = Credential::Key(Arc::new(key(9)));
        let mut inbox = RemoteInbox::open(&settings, credential, moved_to())?;
        steps(&mut inbox)
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn orders_are_listed_read_and_moved_out_without_replacing_a_file() {
    let server = Server::start(&key(9)).await;
    fs::write(server.inbound().join("a.hl7"), b"MSH|a").unwrap();
    fs::write(server.inbound().join("b.hl7.tmp"), b"MSH|b").unwrap();
    fs::write(server.inbound().join("c.hl7"), b"MSH|c").unwrap();
    fs::create_dir(server.inbound().join("older")).unwrap();
    fs::create_dir_all(server.path("processed/2026/11")).unwrap();
    fs::write(server.path("processed/2026/11/a.hl7"), b"MSH|earlier").unwrap();
    fs::create_dir(server.path("failed")).unwrap();
    let (mut names, bytes, processed, failed) = with_inbox(&server.settings, |inbox| {
        let names = inbox.names()?;
        let bytes = inbox.read("a.hl7")?;
        let processed = inbox.move_out("a.hl7", false, "2026", "11")?;
        let failed = inbox.move_out("c.hl7", true, "2026", "11")?;
        Ok((names, bytes, processed, failed))
    })
    .await
    .unwrap();
    // Regular files only; which of them are complete is the intake's choice.
    names.sort();
    assert_eq!(names, ["a.hl7", "b.hl7.tmp", "c.hl7"]);
    assert_eq!(bytes, b"MSH|a");
    // A name taken in Processed gets the next free one; Failed › 2026 › 11 is made.
    assert_eq!(processed, "/processed/2026/11/a-1.hl7");
    assert_eq!(failed, "/failed/2026/11/c.hl7");
    let kept = |path: &str| fs::read(server.path(path)).unwrap();
    assert_eq!(kept("processed/2026/11/a.hl7"), b"MSH|earlier");
    assert_eq!(kept("processed/2026/11/a-1.hl7"), b"MSH|a");
    assert_eq!(kept("failed/2026/11/c.hl7"), b"MSH|c");
    assert_eq!(server.names(), ["b.hl7.tmp", "older"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_input_or_processed_folder_is_reported_and_the_file_stays() {
    let server = Server::start(&key(9)).await;
    fs::write(server.inbound().join("a.hl7"), b"MSH|a").unwrap();
    let elsewhere = SftpSettings {
        remote_folder: "/orders".to_owned(),
        ..server.settings.clone()
    };
    assert_eq!(
        with_inbox(&elsewhere, |_| Ok(())).await,
        Err(SftpProblem::Write(WriteProblem::FolderMissing))
    );
    // No /processed on the server: the year folder cannot be made there.
    let moved = with_inbox(&server.settings, |inbox| {
        inbox.move_out("a.hl7", false, "2026", "11")
    })
    .await;
    assert_eq!(moved, Err(SftpProblem::Write(WriteProblem::FolderMissing)));
    assert_eq!(server.names(), ["a.hl7"]);
}
