//! Adapters from owner queue records to the common PHI-free engine shape.

use super::{QueueDefinition, WorkQueueError, WorkQueueItem};

pub(super) fn order_decision_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_orders::orders_awaiting_decision(conn, limit)?
        .into_iter()
        .map(|item| item_from_order(definition, item))
        .collect())
}

pub(super) fn order_hold_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_orders::orders_with_open_hold(conn, limit)?
        .into_iter()
        .map(|item| item_from_order(definition, item))
        .collect())
}

fn item_from_order(definition: QueueDefinition, row: lis_orders::OrderQueueItem) -> WorkQueueItem {
    item(
        definition,
        "order",
        row.order_id,
        row.order_number,
        row.state,
        row.since,
    )
}

pub(super) fn exception_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let rows = if definition.code == "performed_not_ordered" {
        lis_results::out_of_scope_queue(conn, limit)?
    } else {
        lis_results::open_quarantine_queue(conn, limit)?
    };
    Ok(rows
        .into_iter()
        .map(|row| {
            let title = row
                .accession_number
                .unwrap_or_else(|| format!("Import {}", row.import_attempt_id));
            item(
                definition,
                "import_attempt",
                row.import_attempt_id,
                title,
                row.state,
                row.since,
            )
        })
        .collect())
}

pub(super) fn identity_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_identity::list_all_open_identity_holds(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "subject",
                row.subject_id,
                format!("Identity hold {}", row.id),
                match row.kind {
                    lis_identity::HoldKind::PossibleDuplicate => "possible_duplicate".to_owned(),
                },
                row.opened_at,
            )
        })
        .collect())
}

pub(super) fn specimen_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_specimens::specimens_awaiting_decision(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "accession",
                row.accession_id,
                row.accession_number,
                row.acceptance.as_str().to_owned(),
                row.since,
            )
        })
        .collect())
}

pub(super) fn label_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_specimens::failed_label_jobs(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "accession",
                row.accession_id,
                format!("{} / {}", row.accession_number, row.label_code),
                row.error_code,
                row.failed_at,
            )
        })
        .collect())
}

pub(super) fn qc_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_analytical::qc_work_queue(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "run_analyte_qc",
                format!("{}:{}", row.run_id, row.analyte_id),
                format!("Run {} / analyte {}", row.run_id, row.analyte_id),
                row.status.as_str().to_owned(),
                row.since,
            )
        })
        .collect())
}

pub(super) fn deviation_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_analytical::open_deviations(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "deviation",
                row.deviation_id,
                format!("Deviation {}", row.deviation_id),
                row.state.as_str().to_owned(),
                row.opened_at,
            )
        })
        .collect())
}

pub(super) fn item(
    definition: QueueDefinition,
    item_kind: &str,
    item_id: impl ToString,
    title: String,
    state: String,
    since: lis_core::UtcTimestamp,
) -> WorkQueueItem {
    let state_label = super::catalog::state_label(&state);
    WorkQueueItem {
        queue: definition.code.to_owned(),
        item_kind: item_kind.to_owned(),
        item_id: item_id.to_string(),
        title,
        state,
        state_label: Some(state_label),
        since,
        next_action: definition.next_action.to_owned(),
        has_more: Some(false),
    }
}

/// Pending collection quality decisions are grouped by accession, oldest first.
pub(super) fn collection_review_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_specimens::pending_collection_reviews(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "collection_review",
                row.accession_id,
                row.accession_number,
                "pending".to_owned(),
                row.recorded_at,
            )
        })
        .collect())
}
