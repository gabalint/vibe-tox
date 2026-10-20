//! Reporting and infrastructure queue adapters.

use lis_reporting::{DestinationState, IncidentQueueSubject, ReleaseQueue};

use super::{QueueDefinition, WorkQueueError, WorkQueueItem, billing_items};

pub(super) fn reporting_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    match definition.code {
        "release_ready" => release_items(conn, definition, limit, ReleaseQueue::Ready),
        "release_blocked" => release_items(conn, definition, limit, ReleaseQueue::Blocked),
        "partial_reports" => Ok(revision_items(
            definition,
            lis_reporting::partial_reports(conn, limit)?,
        )),
        "impact_review" => impact_items(conn, definition, limit, false),
        "correction_due" => impact_items(conn, definition, limit, true),
        "delivery_work" => delivery_items(conn, definition, limit),
        "reports_without_destination" => Ok(revision_items(
            definition,
            lis_reporting::revisions_without_destination(conn, limit)?,
        )),
        "unverified_destinations" => destination_items(conn, definition, limit),
        "preservation_incidents" => incident_items(conn, definition, limit),
        "billing_work" => billing_items::billing_items(conn, definition, limit),
        "outbox_dead_letters" => dead_letter_items(conn, definition, limit),
        _ => Ok(Vec::new()),
    }
}

fn release_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    queue: ReleaseQueue,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let state = match queue {
        ReleaseQueue::Ready => "ready",
        ReleaseQueue::Blocked => "blocked",
    };
    Ok(lis_reporting::release_queue(conn, queue, limit)?
        .into_iter()
        .map(|row| {
            let state = state.to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "specimen_report".to_owned(),
                item_id: row.specimen_id.to_string(),
                title: row.report_number.unwrap_or_else(|| {
                    row.accession_number
                        .unwrap_or_else(|| format!("Specimen {}", row.specimen_id))
                }),
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.since,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}

/// Destinations awaiting verification, or found wrong
/// by a delivery attempt and awaiting replacement or retirement.
fn destination_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_reporting::destinations_needing_attention(conn, limit)?
        .into_iter()
        .map(|row| {
            let state = row.state.as_str().to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "delivery_destination".to_owned(),
                item_id: row.destination_id.to_string(),
                title: row.label,
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.since,
                next_action: match row.state {
                    DestinationState::ReportedWrong => "Replace or retire",
                    DestinationState::Unverified
                    | DestinationState::Verified
                    | DestinationState::Retired => definition.next_action,
                }
                .to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}

/// Open incidents, and damaged stored content no incident covers yet
/// (NFR-07). Named incidents use `<report>:<incident>`; discovered damage uses
/// `<report>:revision:<revision>` until opening records the incident.
fn incident_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_reporting::preservation_incidents(conn, limit)?
        .into_iter()
        .map(|row| {
            let state = row.kind.as_str().to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "preservation_incident".to_owned(),
                item_id: match row.subject {
                    IncidentQueueSubject::Incident(id) => format!("{}:{id}", row.report_id),
                    IncidentQueueSubject::UnrecordedRevision(id) => {
                        format!("{}:revision:{id}", row.report_id)
                    }
                },
                title: row.report_number,
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.since,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}

fn revision_items(
    definition: QueueDefinition,
    rows: Vec<lis_reporting::RevisionQueueItem>,
) -> Vec<WorkQueueItem> {
    rows.into_iter()
        .map(|row| {
            let state = definition.code.to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "report_revision".to_owned(),
                item_id: row.revision_id.to_string(),
                title: row.report_number,
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.released_at,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect()
}

fn impact_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    correction_due: bool,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    // Select the requested impact kind before limiting; other kinds cannot hide its summary.
    let rows = lis_reporting::impact_queue(conn, u32::MAX)?
        .into_iter()
        .filter(|row| row.correction_due == correction_due)
        .take(limit as usize)
        .collect();
    Ok(revision_items(definition, rows))
}

fn delivery_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let input = lis_reporting::ListDeliveryObligationsInput {
        filter: lis_reporting::DeliveryQueueFilter::All,
        limit,
        after: None,
    };
    Ok(lis_reporting::delivery_queue(conn, input)?
        .into_iter()
        .map(|row| {
            let state = row.status.code.as_str().to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "obligation".to_owned(),
                item_id: row.obligation_id.to_string(),
                title: row.report_number,
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.created_at,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}

fn dead_letter_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_store::outbox::dead_letters(conn, limit)?
        .into_iter()
        .map(|row| {
            let state = "dead".to_owned();
            WorkQueueItem {
                queue: definition.code.to_owned(),
                item_kind: "outbox_item".to_owned(),
                item_id: row.id.to_string(),
                title: row.topic,
                state_label: Some(super::catalog::state_label(&state)),
                state,
                since: row.updated_at,
                next_action: definition.next_action.to_owned(),
                has_more: Some(false),
            }
        })
        .collect())
}
