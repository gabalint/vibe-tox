//! Interfaces › Logs (ticket #4678): a sent HL7 result's details show the message exactly as
//! the partner received it, its control ID (MSH-10) and each attempt with its outcome; every
//! opening is an `access` event, and the log search finds the message by its control ID.
//! Synthetic data only.

use lis_core::ErrorCode;
use lis_engine::{GetPartnerMessage, ListPartnerMessages, PartnerMessageDetail};
use lis_interfaces::partner::{MessageContent, MessageView, PartnerDeliveryId};

use super::partner_hl7::{approve, draft_held};
use super::partner_mllp::{partner, settings};
use super::partner_pdfs::Pdfs;
use crate::m2_lab::Role;
use crate::m3_fixture::Staff;

async fn detail(hl7: &Pdfs, delivery_id: PartnerDeliveryId) -> PartnerMessageDetail {
    hl7.bench.sign_in(Staff::Desk(Role::Admin)).await;
    hl7.bench
        .lab()
        .engine()
        .get_partner_message(GetPartnerMessage { delivery_id })
        .await
        .unwrap()
}

#[tokio::test]
async fn a_sent_hl7_result_shows_as_sent_with_its_control_id_and_attempts() {
    let (port, received) = partner(vec![None, Some("AA")]).await;
    let hl7 = Pdfs::start_with(settings(port)).await;
    hl7.release(None).await;
    hl7.deliver().await;
    draft_held(&hl7).await;
    let delivery_id = hl7.rows().await.remove(0).delivery_id.unwrap();
    // Held for its code mappings: nothing is made yet, so there is nothing to show.
    let held = detail(&hl7, delivery_id).await;
    assert_eq!(held.content, MessageContent::NotMade);
    assert!(held.segments.is_empty() && held.control_id.is_none() && held.attempts.is_empty());

    approve(&hl7).await;
    hl7.deliver().await;
    hl7.bench.lab().t().advance(60);
    hl7.deliver().await;
    let shown = detail(&hl7, delivery_id).await;
    assert_eq!(shown.content, MessageContent::Sent);
    assert_eq!(shown.control_id, Some(delivery_id.to_string()));
    let sent = String::from_utf8(received.lock().unwrap()[1].clone()).unwrap();
    assert_eq!(format!("{}\r", shown.segments.join("\r")), sent);
    let outcomes: Vec<_> = shown
        .attempts
        .iter()
        .map(|attempt| {
            (
                attempt.succeeded,
                attempt.error_code.as_deref(),
                attempt.ack_code.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        [(false, Some("no_ack"), None), (true, None, Some("AA"))]
    );
    let opened = hl7
        .bench
        .lab()
        .t()
        .events("access")
        .into_iter()
        .filter(|(action, _)| action == "interfaces.get_message")
        .count();
    assert_eq!(opened, 2);

    // The partner's control ID finds the message, whatever its letter case.
    let input = ListPartnerMessages {
        since: None,
        connection_id: None,
        view: MessageView::All,
        search: Some(delivery_id.to_string().to_uppercase()),
        hide_tests: false,
        row_ids: Vec::new(),
    };
    let page = hl7
        .bench
        .lab()
        .engine()
        .list_partner_messages(input)
        .await
        .unwrap();
    let found: Vec<_> = page.rows.iter().map(|row| row.delivery_id).collect();
    assert_eq!(found, [Some(delivery_id)]);

    // Without manage_configuration the message is not shown.
    hl7.bench.sign_in(Staff::Releaser).await;
    let refused = hl7
        .bench
        .lab()
        .engine()
        .get_partner_message(GetPartnerMessage { delivery_id })
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Forbidden);
}
