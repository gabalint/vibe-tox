//! Interfaces (ticket #4555, step 4): a released report's HL7 result sent over MLLP to a
//! partner's interface on the lab network (here, localhost). Without an ACK in time it waits
//! as awaiting its acknowledgement and is sent again, the same message, until the partner
//! accepts it (AA); its correction is not accepted (AE) and the partner's reason shows in
//! Logs (REQ-INT-002, REQ-INT-006, REQ-RPT-012). Synthetic data only.

use std::sync::{Arc, Mutex};

use lis_interfaces::partner::mllp::{control_id, frame, unframe};
use lis_interfaces::partner::{
    AckCode, MllpAcknowledgement, MllpPartner, PartnerConnectionSettings, PartnerMessageType,
    PartnerProtocol, Schedule, TransportOutcome,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::partner_hl7::{approve, draft_held};
use super::partner_pdfs::{AMENDMENT, Pdfs};

/// What the partner read, message by message.
type Received = Arc<Mutex<Vec<Vec<u8>>>>;

/// A partner's interface on localhost that keeps each message it reads and answers it with
/// the next code of `answers`, or with silence for `None`.
async fn partner(answers: Vec<Option<&'static str>>) -> (u16, Received) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received = Received::default();
    let kept = Arc::clone(&received);
    tokio::spawn(async move {
        let mut answers = answers.into_iter();
        while let Ok((mut stream, _)) = listener.accept().await {
            let message = read_message(&mut stream).await;
            let id = control_id(&message).unwrap_or_default();
            kept.lock().unwrap().push(message);
            if let Some(code) = answers.next().flatten() {
                let ack = format!(
                    "MSH|^~\\&|SYN-EMR|HOSP-B|VIBETOX LIS|LAB|20261105101501||ACK|A-{id}|P|2.5.1\r\
                     MSA|{code}|{id}|Unknown provider NPI 1234567893\r"
                );
                stream.write_all(&frame(ack.as_bytes())).await.unwrap();
            }
            // Open until the LIS closes the connection, or gives up waiting.
            let _ = stream.read(&mut [0_u8; 1]).await;
        }
    });
    (port, received)
}

async fn read_message(stream: &mut TcpStream) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        if let Some((message, _)) = unframe(&buffer, 1 << 20).unwrap() {
            return message;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        buffer.extend_from_slice(&chunk[..read]);
    }
}

fn settings(port: u16) -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "HL7 Results - Hospital B".to_owned(),
        partner: "CLINIC-B".to_owned(),
        message_type: PartnerMessageType::Hl7Oru,
        protocol: PartnerProtocol::MllpConnect,
        rules: Vec::new(),
        schedule: Schedule::Immediately,
        file_name_pattern: String::new(),
        sftp: None,
        intake: None,
        mllp: Some(MllpPartner {
            address: "127.0.0.1".to_owned(),
            port,
            acknowledgement: MllpAcknowledgement::Application,
            ack_timeout_seconds: 1,
        }),
    }
}

#[tokio::test]
async fn an_hl7_result_over_mllp_is_sent_again_until_acknowledged_and_shows_the_answer() {
    let (port, received) = partner(vec![None, Some("AA"), Some("AE")]).await;
    let hl7 = Pdfs::start_with(settings(port)).await;
    // An MLLP connection keeps no folder.
    assert_eq!(hl7.connection.connection.folder, "");
    hl7.release(None).await;
    hl7.deliver().await;
    draft_held(&hl7).await;
    approve(&hl7).await;
    hl7.deliver().await;
    // No ACK within the second: it left, and awaits its acknowledgement.
    let row = hl7.rows().await.remove(0);
    assert!(row.awaiting_ack);
    assert_eq!(row.transport, TransportOutcome::Written);
    assert_eq!(row.error_code.as_deref(), Some("no_ack"));
    assert_eq!(row.ack_code, None);

    hl7.bench.lab().t().advance(60);
    hl7.deliver().await;
    let row = hl7.rows().await.remove(0);
    assert_eq!(row.ack_code, Some(AckCode::Accepted));
    assert!(!row.awaiting_ack && !row.outcome_unknown);
    assert_eq!(row.written_path, Some(format!("127.0.0.1:{port}")));
    {
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        // Sent again as it was, so the partner can tell a resend from a new result.
        assert_eq!(received[0], received[1]);
        assert!(String::from_utf8_lossy(&received[0]).contains("|ORU^R01^ORU_R01|"));
    }

    hl7.correct_result().await;
    hl7.approve_correction().await;
    hl7.release(Some(AMENDMENT)).await;
    hl7.deliver().await;
    let rows = hl7.rows().await;
    let correction = rows
        .iter()
        .find(|row| row.source_number == Some(2))
        .unwrap();
    assert_eq!(correction.ack_code, Some(AckCode::Error));
    assert_eq!(
        correction.ack_text.as_deref(),
        Some("Unknown provider NPI 1234567893")
    );
    assert_eq!(received.lock().unwrap().len(), 3);
}
