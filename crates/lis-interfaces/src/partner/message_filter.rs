//! Which log rows the filters of `list_partner_messages` keep.

use super::message_queries::{
    ListPartnerMessages, MessageView, PartnerMessageRow, TransportOutcome,
};
use super::model::MessageSource;

/// Whether the row passes the input's view, test-message choice, selected rows and search.
pub(super) fn kept(row: &PartnerMessageRow, input: &ListPartnerMessages) -> bool {
    let mut terms = input
        .search
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .peekable();
    in_view(row, input.view)
        && !(input.hide_tests && row.source == MessageSource::Test)
        && selected(row, &input.row_ids)
        && (terms.peek().is_none() || terms.any(|term| found(row, term)))
}

/// Whether the row is among those selected, by its delivery's ID or else its message's.
fn selected(row: &PartnerMessageRow, row_ids: &[String]) -> bool {
    if row_ids.is_empty() {
        return true;
    }
    let id = row
        .delivery_id
        .as_ref()
        .map_or_else(|| row.message_id.to_string(), |id| id.to_string());
    row_ids.contains(&id)
}

fn in_view(row: &PartnerMessageRow, view: MessageView) -> bool {
    match view {
        MessageView::All => true,
        MessageView::Failed => row.transport == TransportOutcome::Failed,
        MessageView::NotSent => row.transport == TransportOutcome::NotSent,
        MessageView::OutcomeUnknown => row.outcome_unknown,
    }
}

/// Digits find a handoff number; other text is found in the report number or
/// the written file's path.
fn found(row: &PartnerMessageRow, search: &str) -> bool {
    // A whole control ID (MSH-10, the delivery ID) finds its message (#4678).
    if row
        .delivery_id
        .is_some_and(|id| id.to_string().eq_ignore_ascii_case(search))
    {
        return true;
    }
    if search.bytes().all(|byte| byte.is_ascii_digit()) && row.source_label.is_none() {
        return row
            .source_number
            .is_some_and(|number| number.to_string() == search);
    }
    let search = search.to_lowercase();
    [row.source_label.as_deref(), row.written_path.as_deref()]
        .into_iter()
        .flatten()
        .any(|text| text.to_lowercase().contains(&search))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partner::model::PartnerMessageType;

    fn row(source: MessageSource, label: Option<&str>, number: i64) -> PartnerMessageRow {
        PartnerMessageRow {
            message_id: crate::partner::PartnerMessageId::generate(),
            delivery_id: None,
            at: "2026-11-05T10:00:00.000000000Z".parse().unwrap(),
            message_type: PartnerMessageType::Hl7Oru,
            source,
            source_id: None,
            source_number: Some(number),
            source_label: label.map(str::to_owned),
            connection: None,
            reason: None,
            transport: TransportOutcome::Written,
            outcome_unknown: false,
            waiting: None,
            attempts: 0,
            given_up: false,
            superseded: false,
            next_attempt_at: None,
            error_code: None,
            written_path: None,
            ack_code: None,
            ack_text: None,
            awaiting_ack: false,
            peer: None,
            unrouted: false,
            resolution: None,
            rejection_resolved: None,
            allowed_actions: Vec::new(),
            received: None,
        }
    }

    fn input(search: Option<&str>, hide_tests: bool) -> ListPartnerMessages {
        ListPartnerMessages {
            since: None,
            connection_id: None,
            view: MessageView::All,
            search: search.map(str::to_owned),
            hide_tests,
            row_ids: Vec::new(),
        }
    }

    #[test]
    fn hides_test_messages_only_when_asked() {
        let test = row(MessageSource::Test, None, 0);
        let report = row(MessageSource::ReportRevision, Some("A26000009-UR"), 1);
        assert!(kept(&test, &input(None, false)));
        assert!(!kept(&test, &input(None, true)));
        assert!(kept(&report, &input(None, true)));
    }

    #[test]
    fn finds_a_report_number_or_an_exact_handoff_number() {
        let report = row(MessageSource::ReportRevision, Some("A26000009-UR"), 1);
        let handoff = row(MessageSource::BillingHandoff, None, 12);
        assert!(kept(&report, &input(Some(" a26000009 "), false)));
        assert!(kept(&handoff, &input(Some("12"), false)));
        assert!(!kept(&handoff, &input(Some("1"), false)));
    }

    #[test]
    fn finds_the_rows_of_any_comma_separated_value() {
        let first = row(MessageSource::ReportRevision, Some("A26000009-UR"), 1);
        let second = row(MessageSource::ReportRevision, Some("A26000022-UR"), 1);
        let other = row(MessageSource::ReportRevision, Some("A26000033-UR"), 1);
        let search = input(Some("A26000009, A26000022,"), false);
        assert!(kept(&first, &search));
        assert!(kept(&second, &search));
        assert!(!kept(&other, &search));
        assert!(kept(&other, &input(Some(" , "), false)));
    }

    #[test]
    fn keeps_only_the_selected_rows() {
        let chosen = row(MessageSource::ReportRevision, Some("A26000009-UR"), 1);
        let other = row(MessageSource::ReportRevision, Some("A26000022-UR"), 1);
        let only = ListPartnerMessages {
            row_ids: vec![chosen.message_id.to_string()],
            ..input(None, false)
        };
        assert!(kept(&chosen, &only));
        assert!(!kept(&other, &only));
    }
}
