//! The LIS's acknowledgement of an order message received over MLLP (ticket #4555 step 4b;
//! REQ-INT-002): MSH addressed back to the sender, MSA with the code and the message's
//! control ID, and, when the message is not accepted, the reason in words (MSA-3). It
//! carries codes and fixed wording only, never patient data. Pure.

use super::hl7::{SENDING_APPLICATION, esc};
use super::hl7_read::{Reader, message_text, segments, separators};
use super::model::AckCode;

/// What an acknowledgement takes from the message it answers; empty when unreadable.
#[derive(Default)]
struct Answered {
    sending_application: String,
    sending_facility: String,
    receiving_facility: String,
    /// MSH-9.2, such as O21.
    trigger: String,
    control_id: String,
    processing_id: String,
}

fn answered(message: &[u8]) -> Option<Answered> {
    let text = message_text(message)?;
    let separators = separators(text)?;
    let segments = segments(text, separators);
    let msh = segments.first()?;
    let reader = Reader { separators };
    Some(Answered {
        sending_application: reader.component(msh.field(3), 1),
        sending_facility: reader.component(msh.field(4), 1),
        receiving_facility: reader.component(msh.field(6), 1),
        trigger: reader.component(msh.field(9), 2),
        control_id: reader.text(msh.field(10)),
        processing_id: reader.component(msh.field(11), 1),
    })
}

/// The acknowledgement of `message` with `code`; for AE and AR, `reason` (a refusal code)
/// in words. `at` is MSH-7, such as `20261106101500+0000`.
#[must_use]
pub fn acknowledge(message: &[u8], code: AckCode, reason: Option<&str>, at: &str) -> Vec<u8> {
    let answered = answered(message).unwrap_or_default();
    let message_type = if answered.trigger.is_empty() {
        "ACK".to_owned()
    } else {
        format!("ACK^{}^ACK", esc(&answered.trigger))
    };
    let processing_id = if answered.processing_id.is_empty() {
        "P".to_owned()
    } else {
        esc(&answered.processing_id)
    };
    let control_id = esc(&answered.control_id);
    let msh = [
        "MSH|^~\\&".to_owned(),
        SENDING_APPLICATION.to_owned(),
        esc(&answered.receiving_facility),
        esc(&answered.sending_application),
        esc(&answered.sending_facility),
        at.to_owned(),
        String::new(),
        message_type,
        format!("ACK-{control_id}"),
        processing_id,
        "2.5.1".to_owned(),
    ]
    .join("|");
    let mut msa = format!("MSA|{}|{control_id}", code.as_str());
    if let (AckCode::Error | AckCode::Rejected, Some(reason)) = (code, reason) {
        msa.push('|');
        msa.push_str(&esc(reason_text(reason)));
    }
    format!("{msh}\r{msa}\r").into_bytes()
}

/// A refusal code in words, as the partner reads it in MSA-3.
#[must_use]
pub fn reason_text(code: &str) -> &str {
    match code {
        "not_accepted_sender" => "Not from the accepted sending facility (MSH-4)",
        "not_text" => "Not UTF-8 text",
        "not_hl7" => "Not an HL7 message",
        "unsupported_message" => "Not an OML^O21 or ORM^O01 order message",
        "no_control_id" => "No message control ID (MSH-10)",
        "unsupported_control" => "Only new orders (ORC-1 NW) are accepted",
        "no_patient" => "No patient identifier (PID-3)",
        "no_order" => "No order (ORC and OBR)",
        "no_test" => "An OBR without a test code (OBR-4)",
        "no_placer_order" => "No placer order number",
        "unknown_test" => "Unknown test code: no approved code mapping",
        "duplicate_test" => "The same test is ordered twice",
        "unknown_provider" => "Unknown ordering provider NPI",
        "no_provider" => "No ordering provider",
        "unknown_location" => "Unknown client location (PV1-3)",
        "unknown_client" => "The ordering client is not active",
        "diagnosis_system_missing" => "A diagnosis has no coding system",
        "patient_unknown" => "No registered patient has the message's identifier",
        "order_refused" => "The laboratory's order rules refused the order",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::mllp::read_acknowledgement;

    const OML: &[u8] =
        b"MSH|^~\\&|SYN-EMR|HOSP-B|VIBETOX LIS|LAB|20261106093000||OML^O21|MSG-0001|P|2.5.1\r";

    #[test]
    fn an_order_is_acknowledged_to_its_sender_with_its_control_id() {
        let ack = acknowledge(OML, AckCode::Accepted, None, "20261106093001+0000");
        let text = String::from_utf8(ack.clone()).unwrap();
        assert_eq!(
            text,
            "MSH|^~\\&|VIBETOX LIS|LAB|SYN-EMR|HOSP-B|20261106093001+0000||ACK^O21^ACK|\
             ACK-MSG-0001|P|2.5.1\rMSA|AA|MSG-0001\r"
        );
        // The LIS's own reader reads it back.
        assert_eq!(
            read_acknowledgement(&ack, "MSG-0001").unwrap().code,
            AckCode::Accepted
        );
    }

    #[test]
    fn a_refusal_says_why_in_words_and_an_unreadable_message_still_gets_an_answer() {
        let ack = acknowledge(
            OML,
            AckCode::Error,
            Some("unknown_test"),
            "20261106093001+0000",
        );
        let read = read_acknowledgement(&ack, "MSG-0001").unwrap();
        assert_eq!(read.code, AckCode::Error);
        assert_eq!(
            read.text.as_deref(),
            Some("Unknown test code: no approved code mapping")
        );
        let ack = acknowledge(b"garbage", AckCode::Rejected, Some("not_hl7"), "2026");
        let text = String::from_utf8(ack).unwrap();
        assert!(
            text.contains("||ACK|ACK-|P|2.5.1\rMSA|AR||Not an HL7 message\r"),
            "{text}"
        );
    }
}
