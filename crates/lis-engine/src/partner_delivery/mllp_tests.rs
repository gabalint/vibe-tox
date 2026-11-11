//! Sending over MLLP to an in-process partner on localhost that answers as told. Synthetic
//! messages only.

use lis_interfaces::partner::mllp::unframe;
use lis_interfaces::partner::{AckCode, MllpAcknowledgement};
use tokio::net::TcpListener;

use super::*;

const ORU: &[u8] =
    b"MSH|^~\\&|VIBETOX LIS|LAB|SYN-EMR|HOSP-B|20261105101500||ORU^R01|MSG-0001|P|2.5.1\r";

/// What the test partner does with the message it reads.
#[derive(Clone, Copy)]
enum Answer {
    /// An ACK with this MSA-1 and MSA-2, and an ERR-8 reason.
    Ack(&'static str, &'static str),
    /// Nothing; it keeps the connection open.
    Silence,
    /// It closes the connection.
    Hang,
}

/// A partner on a free localhost port that answers each connection with `answer`.
async fn partner(answer: Answer) -> MllpPartner {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(answer_one(stream, answer));
        }
    });
    settings("127.0.0.1", port)
}

/// Reads one framed message from the LIS and answers it as told.
async fn answer_one(mut stream: TcpStream, answer: Answer) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    while unframe(&buffer, 4096).unwrap().is_none() {
        let read = stream.read(&mut chunk).await.unwrap();
        buffer.extend_from_slice(&chunk[..read]);
    }
    match answer {
        Answer::Ack(code, control_id) => {
            let ack = format!(
                "MSH|^~\\&|SYN-EMR|HOSP-B|VIBETOX LIS|LAB|20261105101501||ACK|A1|P|2.5.1\r\
                 MSA|{code}|{control_id}\rERR||||E||||Unknown provider NPI 1234567893\r"
            );
            stream.write_all(&frame(ack.as_bytes())).await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Answer::Silence => tokio::time::sleep(Duration::from_secs(5)).await,
        Answer::Hang => {}
    }
}

fn settings(address: &str, port: u16) -> MllpPartner {
    MllpPartner {
        address: address.to_owned(),
        port,
        acknowledgement: MllpAcknowledgement::Application,
        ack_timeout_seconds: 1,
    }
}

#[tokio::test]
async fn the_partners_acknowledgement_comes_back_with_its_reason() {
    let accepted = send(&partner(Answer::Ack("AA", "MSG-0001")).await, ORU)
        .await
        .unwrap();
    assert_eq!(accepted.code, AckCode::Accepted);
    let error = send(&partner(Answer::Ack("AE", "MSG-0001")).await, ORU)
        .await
        .unwrap();
    assert_eq!(error.code, AckCode::Error);
    assert_eq!(
        error.text.as_deref(),
        Some("Unknown provider NPI 1234567893")
    );
}

#[tokio::test]
async fn no_answer_another_messages_answer_or_a_closed_connection_is_no_acknowledgement() {
    for (answer, problem) in [
        (Answer::Silence, MllpProblem::NoAck),
        (Answer::Hang, MllpProblem::ConnectionLost),
        (
            Answer::Ack("AA", "MSG-0002"),
            MllpProblem::Ack(AckProblem::OtherMessage),
        ),
    ] {
        assert_eq!(send(&partner(answer).await, ORU).await, Err(problem));
    }
}

#[tokio::test]
async fn nothing_is_sent_off_the_lab_network_or_to_a_closed_port() {
    assert_eq!(
        send(&settings("8.8.8.8", 6661), ORU).await,
        Err(MllpProblem::NotLabNetwork)
    );
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    assert_eq!(
        probe(&settings("127.0.0.1", port)).await,
        Err(MllpProblem::Unreachable)
    );
    assert_eq!(probe(&partner(Answer::Hang).await).await, Ok(()));
}
