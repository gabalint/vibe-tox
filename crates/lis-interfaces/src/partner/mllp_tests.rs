//! MLLP framing and acknowledgements, on synthetic messages only.

use super::*;

const ORU: &[u8] =
    b"MSH|^~\\&|OPENBGA TOX|LAB|SYN-EMR|HOSP-B|20261105101500||ORU^R01|MSG-0001|P|2.5.1\r\
PID|1||MRN-00042^^^HOSP-B^MR||Doe^Jane\r";

fn ack(code: &str, control_id: &str, rest: &str) -> Vec<u8> {
    format!(
        "MSH|^~\\&|SYN-EMR|HOSP-B|OPENBGA TOX|LAB|20261105101501||ACK^R01|ACK-1|P|2.5.1\r\
         MSA|{code}|{control_id}{rest}\r"
    )
    .into_bytes()
}

#[test]
fn a_framed_message_comes_back_whole_and_only_when_complete() {
    let framed = frame(ORU);
    assert_eq!(framed[0], START_BLOCK);
    assert_eq!(&framed[framed.len() - 2..], [END_BLOCK, b'\r']);
    // Arriving in pieces: nothing until the end block and carriage return.
    assert_eq!(unframe(&framed[..framed.len() - 1], 4096), Ok(None));
    let mut stream = b"\r\n".to_vec();
    stream.extend_from_slice(&framed);
    stream.extend_from_slice(&frame(b"MSH|second"));
    let (message, used) = unframe(&stream, 4096).unwrap().unwrap();
    assert_eq!(message, ORU);
    assert_eq!(
        unframe(&stream[used..], 4096).unwrap().unwrap().0,
        b"MSH|second"
    );
}

#[test]
fn unframed_or_oversized_bytes_are_refused() {
    assert_eq!(unframe(b"MSH|^~\\&|", 4096), Err(FrameProblem::NotFramed));
    let long = frame(&[b'x'; 100]);
    assert_eq!(unframe(&long, 50), Err(FrameProblem::TooLarge));
    // Still arriving, and already past the limit.
    assert_eq!(unframe(&long[..80], 50), Err(FrameProblem::TooLarge));
    assert_eq!(unframe(b"", 50), Ok(None));
}

#[test]
fn the_control_id_comes_from_msh_10() {
    assert_eq!(control_id(ORU).as_deref(), Some("MSG-0001"));
    assert_eq!(control_id(b"not hl7"), None);
}

#[test]
fn an_acknowledgement_says_accepted_error_or_rejected_with_the_partners_reason() {
    assert_eq!(
        read_acknowledgement(&ack("AA", "MSG-0001", ""), "MSG-0001"),
        Ok(Acknowledgement {
            code: AckCode::Accepted,
            text: None,
        })
    );
    let error = ack(
        "AE",
        "MSG-0001",
        "|Unknown provider\rERR||PID^1|101^Required field missing^HL70357|E||||Unknown provider NPI 1234567893",
    );
    assert_eq!(
        read_acknowledgement(&error, "MSG-0001"),
        Ok(Acknowledgement {
            code: AckCode::Error,
            text: Some("Unknown provider NPI 1234567893".to_owned()),
        })
    );
    let rejected = ack("AR", "MSG-0001", "|Sender not \\T\\ accepted");
    assert_eq!(
        read_acknowledgement(&rejected, "MSG-0001")
            .unwrap()
            .text
            .as_deref(),
        Some("Sender not & accepted")
    );
}

#[test]
fn an_answer_for_another_message_or_without_a_code_is_not_an_acknowledgement() {
    assert_eq!(
        read_acknowledgement(&ack("AA", "MSG-0002", ""), "MSG-0001"),
        Err(AckProblem::OtherMessage)
    );
    for unreadable in [
        ack("CA", "MSG-0001", ""),
        b"MSH|^~\\&|SYN-EMR\r".to_vec(),
        b"\xff\xfe".to_vec(),
    ] {
        assert_eq!(
            read_acknowledgement(&unreadable, "MSG-0001"),
            Err(AckProblem::Unreadable)
        );
    }
}
