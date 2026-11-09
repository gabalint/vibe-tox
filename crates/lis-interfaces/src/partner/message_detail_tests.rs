//! Showing a kept HL7 message: its segments one per line and its MSH-10. Synthetic data.

use lis_core::Id;

use super::{MessageContent, PartnerMessageDetail};

fn kept() -> PartnerMessageDetail {
    PartnerMessageDetail {
        delivery_id: Id::generate(),
        control_id: None,
        content: MessageContent::Unavailable,
        segments: Vec::new(),
        attempts: Vec::new(),
        kept_sha256: Some("0".repeat(64)),
    }
}

#[test]
fn kept_bytes_show_as_segments_with_their_control_id() {
    let mut detail = kept();
    detail.show(
        b"MSH|^~\\&|VIBETOX LIS|LAB||CLINIC-B|20261105100000+0000||ORU^R01^ORU_R01|CTRL-1|T|2.5.1\r\
          PID|1||TEST-00000^^^VIBETOX^MR||TEST^SYNTHETIC\r",
    );
    assert_eq!(detail.content, MessageContent::Sent);
    assert_eq!(detail.control_id.as_deref(), Some("CTRL-1"));
    assert_eq!(detail.segments.len(), 2);
    assert!(detail.segments[1].starts_with("PID|1|"));
}

#[test]
fn bytes_that_are_not_hl7_text_stay_unavailable() {
    let mut detail = kept();
    detail.show(&[0xff, 0xfe, 0x00]);
    assert_eq!(detail.content, MessageContent::Unavailable);
    assert!(detail.segments.is_empty() && detail.control_id.is_none());
}
