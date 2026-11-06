//! HL7 orders received over MLLP from a clinic's EMR on the lab network (ticket #4555 step
//! 4b; REQ-INT-002, REQ-INT-003, REQ-INT-006): once the destination check covers it, the
//! LIS listens on the connection's address and port and lets only the allowed partner
//! addresses in. Each message is saved, then answered: AA when it waits for its patient,
//! AE for a content error, AR for another sender; a replay gets the earlier answer.
//! Stopping the connection frees the port; a port another program holds shows on the
//! connection. Synthetic data only.

use std::net::{Ipv4Addr, TcpListener};
use std::time::Duration;

use lis_engine::{
    CreatePartnerConnectionInput, PartnerConnectionDetail, SetPartnerConnectionActive,
    TestPartnerMllpInput,
};
use lis_interfaces::partner::mllp::{control_id, frame, read_acknowledgement, unframe};
use lis_interfaces::partner::{
    AckCode, AllowedPartner, CharacterSet, ConnectionStatus, MllpAcknowledgement, MllpListener,
    PartnerConnectionId, PartnerConnectionSettings, PartnerProtocol,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::partner_orders::{SENDER, connect, oml, revision, settings};

/// A port free on this computer now.
fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn listener(port: u16, partner: &str) -> MllpListener {
    MllpListener {
        address: "127.0.0.1".to_owned(),
        port,
        allowed_partners: vec![AllowedPartner {
            address: partner.to_owned(),
            note: "Synthetic EMR".to_owned(),
        }],
        acknowledgement: MllpAcknowledgement::Application,
        character_set: CharacterSet::Utf8,
        idle_timeout_seconds: 30,
    }
}

/// A bench with an HL7 Orders connection that listens on `port` for `partner`.
async fn listening(port: u16, partner: &str) -> (Bench, PartnerConnectionDetail) {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Desk(Role::Admin)).await;
    let create = CreatePartnerConnectionInput {
        settings: PartnerConnectionSettings {
            protocol: PartnerProtocol::MllpListen,
            mllp_listener: Some(listener(port, partner)),
            ..settings()
        },
        folder_choice: None,
        processed_folder_choice: None,
        failed_folder_choice: None,
        password: None,
        active: true,
    };
    let connection = connect(&bench, create).await;
    (bench, connection)
}

/// The EMR sends `message` and reads the acknowledgement; `None` when the LIS closes the
/// connection instead.
async fn send(stream: &mut TcpStream, message: &str) -> Option<(AckCode, Option<String>)> {
    stream.write_all(&frame(message.as_bytes())).await.ok()?;
    let control_id = control_id(message.as_bytes()).unwrap();
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if let Some((ack, _)) = unframe(&buffer, 64 * 1024).unwrap() {
            let ack = read_acknowledgement(&ack, &control_id).unwrap();
            return Some((ack.code, ack.text));
        }
        let wait = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut chunk));
        let read = wait.await.unwrap().ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

async fn set_active(
    bench: &Bench,
    connection_id: PartnerConnectionId,
    active: bool,
) -> PartnerConnectionDetail {
    let expected_revision = revision(bench, connection_id).await;
    bench
        .lab()
        .engine()
        .set_partner_connection_active(SetPartnerConnectionActive {
            connection_id,
            expected_revision,
            active,
            reason: "Synthetic: the clinic's EMR moves".to_owned(),
        })
        .await
        .unwrap()
}

fn text(reason: &str) -> Option<String> {
    Some(reason.to_owned())
}

#[tokio::test]
async fn an_emr_order_over_mllp_is_saved_then_acknowledged() {
    let port = free_port();
    let (bench, connection) = listening(port, "127.0.0.1").await;
    let view = connection.connection.listener.clone().unwrap();
    assert!(view.listening, "{view:?}");
    assert_eq!(connection.connection.folder, "");

    let mut emr = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    // It waits for its patient: kept, so accepted. Its replay gets the same answer.
    let order = oml(SENDER, "MSG-1");
    assert_eq!(
        send(&mut emr, &order).await,
        Some((AckCode::Accepted, None))
    );
    assert_eq!(
        send(&mut emr, &order).await,
        Some((AckCode::Accepted, None))
    );
    // Another sending facility is rejected; a test code without a mapping is an error.
    let other = oml("OTHER-FAC", "MSG-2");
    let sender = text("Not from the accepted sending facility (MSH-4)");
    assert_eq!(
        send(&mut emr, &other).await,
        Some((AckCode::Rejected, sender))
    );
    let unmapped = oml(SENDER, "MSG-3").replace("SYN-PANEL", "SYN-OTHER");
    let unknown = text("Unknown test code: no approved code mapping");
    assert_eq!(
        send(&mut emr, &unmapped).await,
        Some((AckCode::Error, unknown))
    );

    let conn = bench.lab().t().conn();
    let mut statement = conn
        .prepare("SELECT state, ack_code, peer, file_name FROM ifc_partner_received ORDER BY rowid")
        .unwrap();
    let rows: Vec<(String, String, String, String)> = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let row = |state: &str, ack: &str| {
        (
            state.to_owned(),
            ack.to_owned(),
            "127.0.0.1".to_owned(),
            String::new(),
        )
    };
    assert_eq!(
        rows,
        [
            row("waiting_patient", "AA"),
            row("duplicate", "AA"),
            row("rejected", "AR"),
            row("rejected", "AE"),
        ]
    );
}

#[tokio::test]
async fn only_allowed_partners_get_in_and_a_stopped_connection_frees_its_port() {
    let port = free_port();
    let (bench, connection) = listening(port, "10.20.0.15").await;
    let id = connection.connection.id;
    // 127.0.0.1 is not an allowed partner: closed with no answer, and nothing is saved.
    let mut stranger = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    assert_eq!(send(&mut stranger, &oml(SENDER, "MSG-1")).await, None);

    let stopped = set_active(&bench, id, false).await;
    assert_eq!(stopped.connection.listener, None);
    let count = |sql: &str| -> i64 {
        let conn = bench.lab().t().conn();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    };
    assert_eq!(count("SELECT count(*) FROM ifc_partner_received"), 0);
    assert_eq!(
        count(
            "SELECT count(*) FROM audit_events WHERE stream = 'security' \
             AND action = 'interfaces.reject_partner_connection' AND outcome = 'denied'"
        ),
        1
    );

    // Another program holds the port now: the connection says why it cannot listen.
    let held = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
    let started = set_active(&bench, id, true).await;
    let view = started.connection.listener.unwrap();
    assert!(!view.listening);
    assert_eq!(view.problem.as_deref(), Some("port_in_use"));
    assert_eq!(started.connection.status, ConnectionStatus::Error);
    let tested = bench
        .lab()
        .engine()
        .test_partner_mllp(TestPartnerMllpInput {
            partner: None,
            listener: Some(listener(port, "10.20.0.15")),
            connection_id: Some(id),
        })
        .await
        .unwrap();
    assert_eq!(tested.problem.as_deref(), Some("port_in_use"));

    // Freed, the next pass listens; a test of its own listener passes.
    drop(held);
    bench.lab().engine().listen_for_partners().await;
    bench.sign_in(Staff::Desk(Role::Admin)).await;
    let tested = bench
        .lab()
        .engine()
        .test_partner_mllp(TestPartnerMllpInput {
            partner: None,
            listener: Some(listener(port, "10.20.0.15")),
            connection_id: Some(id),
        })
        .await
        .unwrap();
    assert_eq!(tested.problem, None);
}
