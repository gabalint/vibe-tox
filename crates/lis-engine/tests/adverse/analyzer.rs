//! Analyzer adverse cases over the real TCP listener (REQ-INT-009, REQ-OPS-013; VAL-04,
//! VAL-50, VAL-56): an app stopped mid-transfer, receipt while nobody is signed in,
//! garbage and partial frames, a burst of duplicates, and an older message arriving
//! after its correction. Expected outcomes: `docs/planning/m2-design-brief.md` §3.5,
//! `fixtures/astm/m2/README.md`, the validation plan rows named per test.

use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::path::Path;
use std::time::Duration;

use lis_analytical::RevisionIndicator;
use lis_engine::Engine;
use lis_interfaces::ListenerState;
use lis_store::DATABASE_FILE;
use rusqlite::Connection;

use crate::m2_lab::Lab;
use crate::m2_steps::{CORRECTION, JOURNEY, line};

const ENQ: u8 = 0x05;
const ACK: u8 = 0x06;
const EOT: u8 = 0x04;
const NAK: u8 = 0x15;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

/// Journey counts from the fixture README: six patient measurements, four QC rows.
const PATIENT_ROWS: i64 = 6;
const QC_ROWS: i64 = 4;

/// A synthetic analyzer on one TCP connection.
struct Analyzer(TcpStream);

impl Analyzer {
    fn connect(port: u16) -> Self {
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Self(stream)
    }

    /// Writes `bytes` and returns the one-byte reply.
    fn send(&mut self, bytes: &[u8]) -> u8 {
        self.0.write_all(bytes).unwrap();
        let mut reply = [0_u8; 1];
        self.0.read_exact(&mut reply).unwrap();
        reply[0]
    }
}

/// One final frame per record, numbered from 1 modulo 8, with the checksum over the
/// frame number through ETX (the same framing as `m2_steps`, whose encoder is private).
fn frames(artifact: &[u8]) -> Vec<Vec<u8>> {
    artifact
        .split(|byte| *byte == CR)
        .filter(|record| !record.is_empty())
        .enumerate()
        .map(|(index, record)| {
            let number = b'0' + u8::try_from((index + 1) % 8).unwrap();
            let mut frame = vec![STX, number];
            frame.extend_from_slice(record);
            frame.extend_from_slice(&[CR, ETX]);
            let sum = frame[1..].iter().fold(0_u8, |sum, b| sum.wrapping_add(*b));
            frame.extend_from_slice(format!("{sum:02X}").as_bytes());
            frame.extend_from_slice(&[CR, LF]);
            frame
        })
        .collect()
}

fn conn(data_dir: &Path) -> Connection {
    Connection::open(data_dir.join(DATABASE_FILE)).unwrap()
}

/// `(state, duplicate_of is set)` of every receipt, oldest first.
fn receipts(data_dir: &Path) -> Vec<(String, bool)> {
    conn(data_dir)
        .prepare("SELECT state, duplicate_of IS NOT NULL FROM ifc_receipts ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(data_dir: &Path, table: &str) -> i64 {
    conn(data_dir)
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

/// `(outcome, rows_new, rows_qc, rows_duplicate, rows_quarantined)` of every import
/// attempt once there are `expected`, read from the database because nobody need be
/// signed in.
async fn attempts(data_dir: &Path, expected: usize) -> Vec<(String, i64, i64, i64, i64)> {
    for _ in 0..400 {
        let rows: Vec<(String, i64, i64, i64, i64)> = conn(data_dir)
            .prepare(
                "SELECT outcome, rows_new, rows_qc, rows_duplicate, rows_quarantined \
                 FROM res_import_attempts ORDER BY processed_at, id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        if rows.len() >= expected {
            return rows;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("fewer than {expected} import attempts were recorded");
}

/// Waits until the analyzer's listener is listening again after a start.
async fn listening(engine: &Engine) {
    for _ in 0..400 {
        let statuses = engine.list_listener_status().await.unwrap();
        if statuses[0].state == ListenerState::Listening {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the listener did not return to listening");
}

/// VAL-56, VAL-38: the host loses power after the analyzer's last frame was
/// acknowledged and before EOT. The data folder as it is at that moment (every
/// acknowledged frame committed, the receipt still receiving) is copied, and the app
/// starts on the copy: startup reconciliation completes the receipt from its durable
/// frames and the import runs once; the analyzer's full resend adds no observation.
#[tokio::test]
async fn power_lost_mid_transfer_recovers_acknowledged_frames_and_a_resend_adds_nothing() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let port = lab.port;
    let analyzer = tokio::task::spawn_blocking(move || every_frame(port))
        .await
        .unwrap();
    let host = tempfile::tempdir().unwrap();
    copy_folder(lab.t().data_dir(), host.path());
    lab.engine().shutdown().await;
    drop(analyzer);

    let engine = crate::storage::start(host.path(), &lab.t().clock)
        .await
        .unwrap();
    let recovered = attempts(host.path(), 1).await;
    assert_eq!(
        recovered[0],
        ("processed".to_owned(), PATIENT_ROWS, QC_ROWS, 0, 0)
    );
    assert_eq!(receipts(host.path()), [("complete".to_owned(), false)]);
    listening(&engine).await;
    tokio::task::spawn_blocking(move || every_frame(port).0.write_all(&[EOT]).unwrap())
        .await
        .unwrap();

    let mut states = receipts(host.path());
    for _ in 0..400 {
        if states.len() == 2 && states[1].0 == "complete" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
        states = receipts(host.path());
    }
    assert_eq!(
        states.len(),
        2,
        "the resend completed no receipt: {states:?}"
    );
    if !states[1].1 {
        attempts(host.path(), 2).await;
    }
    assert_eq!(
        count(host.path(), "res_measurements"),
        PATIENT_ROWS,
        "VAL-38"
    );
    assert_eq!(count(host.path(), "ana_qc_observations"), QC_ROWS, "VAL-38");
    engine.shutdown().await;
}

/// Sends ENQ and every frame of the journey message, each acknowledged only after it
/// is durable (REQ-INT-009), and leaves the session open before EOT.
fn every_frame(port: u16) -> Analyzer {
    let mut analyzer = Analyzer::connect(port);
    assert_eq!(analyzer.send(&[ENQ]), ACK);
    for frame in frames(JOURNEY) {
        assert_eq!(analyzer.send(&frame), ACK, "REQ-INT-009");
    }
    analyzer
}

/// Copies a stopped-in-time data folder, file by file.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// VAL-50, REQ-OPS-013: with nobody signed in to the app, a message is still received,
/// acknowledged and imported, and the sign-in screen's listener status shows activity.
#[tokio::test]
async fn messages_are_received_and_imported_while_nobody_is_signed_in() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.engine().sign_out().await.unwrap();

    let delivered = lab.deliver(JOURNEY).await;

    assert_eq!(delivered.duplicate_of, None);
    let imported = attempts(lab.t().data_dir(), 1).await;
    assert_eq!(
        imported[0],
        ("processed".to_owned(), PATIENT_ROWS, QC_ROWS, 0, 0)
    );
    let status = lab.engine().list_listener_status().await.unwrap();
    assert!(status[0].last_activity_at.is_some(), "{status:?}");
    assert_eq!(status[0].detail, None);
}

/// VAL-50 and the conformance corpus over a socket: bytes before ENQ, a frame with a
/// bad checksum, half a frame and a dropped connection store no message and import
/// nothing, and the next analyzer session is received normally.
#[tokio::test]
async fn garbage_and_partial_frames_store_nothing_and_the_next_session_imports() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let port = lab.port;
    tokio::task::spawn_blocking(move || {
        let mut noise = Analyzer::connect(port);
        noise
            .0
            .write_all(b"\x00\xff garbage \x17\x1b[2J noise")
            .unwrap();
        drop(noise);
        let mut partial = Analyzer::connect(port);
        assert_eq!(partial.send(&[ENQ]), ACK);
        let mut corrupt = frames(JOURNEY).remove(0);
        corrupt[4] ^= 0x20;
        assert_eq!(partial.send(&corrupt), NAK, "a bad checksum is refused");
        let half = frames(JOURNEY).remove(0);
        partial.0.write_all(&half[..half.len() / 2]).unwrap();
        drop(partial);
    })
    .await
    .unwrap();
    listening(lab.engine()).await;

    let delivered = lab.deliver(JOURNEY).await;

    assert_eq!(delivered.duplicate_of, None);
    let imported = attempts(lab.t().data_dir(), 1).await;
    assert_eq!(imported.len(), 1, "{imported:?}");
    assert_eq!(
        imported[0],
        ("processed".to_owned(), PATIENT_ROWS, QC_ROWS, 0, 0)
    );
    let states = receipts(lab.t().data_dir());
    let complete = states
        .iter()
        .filter(|(state, _)| state == "complete")
        .count();
    assert_eq!(complete, 1, "{states:?}");
    assert_eq!(count(lab.t().data_dir(), "res_measurements"), PATIENT_ROWS);
}

/// VAL-04, M2 brief §3.5: the same message sent five times is one receipt and four
/// linked duplicates, with one import attempt and no extra evidence.
#[tokio::test]
async fn a_burst_of_identical_messages_imports_once() {
    let lab = Lab::start(RevisionIndicator::None).await;

    let mut duplicates = Vec::new();
    for _ in 0..5 {
        duplicates.push(lab.deliver(JOURNEY).await.duplicate_of.is_some());
    }

    assert_eq!(duplicates, [false, true, true, true, true]);
    let imported = attempts(lab.t().data_dir(), 1).await;
    // A dispatcher poll (2 s) passes, so a wrong second import would have run.
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(
        count(lab.t().data_dir(), "res_import_attempts"),
        1,
        "{imported:?}"
    );
    assert_eq!(count(lab.t().data_dir(), "res_measurements"), PATIENT_ROWS);
    assert_eq!(count(lab.t().data_dir(), "ana_qc_observations"), QC_ROWS);
}

/// VAL-04, VAL-27, M2 brief §3.5 ("evidence from a different attempt never changes the
/// current revision automatically"): an analyzer's correction (`148.90`, status C)
/// arrives before the original final result (`152.40`, status F) it corrects. The older
/// message must not silently replace the newer value: it is held as a conflict.
#[tokio::test]
async fn an_older_result_arriving_after_its_correction_does_not_replace_it() {
    let lab = Lab::start(RevisionIndicator::ResultStatusCorrection).await;

    lab.import(CORRECTION).await;
    let original = lab.import(JOURNEY).await;

    assert_eq!(original.rows_quarantined, 1, "{original:?}");
    let results = lab.results(0).await;
    let current = line(&results, "SYN-MOR").current.as_ref().unwrap();
    assert_eq!(current.magnitude.as_deref(), Some("148.90"));
    assert_eq!(current.revision_no, 1);
}
