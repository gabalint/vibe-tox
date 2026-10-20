//! The billing owner's work queue adapter (REQ-OPS-004, brief §6).

use super::{QueueDefinition, WorkQueueError, WorkQueueItem};

/// Billing work oldest first: an item is its handoff item, else its
/// derivation, else its subject and date of service, and its kind says
/// which; titles are accession numbers, never demographics.
pub(super) fn billing_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    lis_billing::billing_work_queue(conn, limit)?
        .into_iter()
        .map(|row| {
            let (item_kind, item_id) = match (row.item_id, row.derivation_id) {
                (Some(item), _) => (
                    "billing_handoff_item",
                    format!(
                        "{}:{item}",
                        row.handoff_id
                            .ok_or(lis_billing::BillingError::Inconsistent(
                                "handoff item without handoff"
                            ))?
                    ),
                ),
                (None, Some(derivation)) => ("billing_derivation", derivation.to_string()),
                (None, None) => (
                    "billing_subject",
                    format!(
                        "{}|{}",
                        row.subject_id,
                        row.date_of_service.as_deref().unwrap_or("")
                    ),
                ),
            };
            let state = row.state.as_str().to_owned();
            Ok(WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: item_kind.to_owned(),
                item_id,
                title: row.accession_numbers.join(", "),
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.since,
                next_action: row
                    .allowed_actions
                    .first()
                    .map_or(definition.next_action, |action| next_action(action))
                    .to_owned(),
                has_more: Some(false),
            })
        })
        .collect()
}

/// The plain wording of the command a billing state calls for.
fn next_action(command: &str) -> &'static str {
    match command {
        "derive_services" => "Derive services",
        "approve_service_derivation" => "Review the derivation",
        "create_billing_handoff" => "Hand off",
        "record_handoff_outcome" => "Reconcile the outcome",
        _ => "Review",
    }
}
