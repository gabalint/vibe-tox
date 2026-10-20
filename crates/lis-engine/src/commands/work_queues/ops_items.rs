//! Operations queue adapters (REQ-OPS-002, REQ-OPS-006, REQ-OPS-008, REQ-OPS-013; M4
//! design brief §6.1 "Work items"): health checks in warning or alert, listening gaps,
//! the open recovery reconciliation, the running version awaiting acceptance, and patient
//! positions whose results are expected.

use lis_core::Principal;
use lis_ops::health::{ALL_CHECKS, CheckCode, HealthState};
use lis_ops::updates::VersionStatus;

use super::owner_items::item;
use super::{QueueDefinition, WorkQueueError, WorkQueueItem};

/// Checks with their own queue, so `ops_alerts` does not list them twice.
const OWN_QUEUE: [CheckCode; 3] = [
    CheckCode::ListeningGaps,
    CheckCode::RecoveryReconciliation,
    CheckCode::Version,
];

pub(super) fn ops_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    principal: &Principal,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    match definition.code {
        "ops_alerts" => alert_items(conn, definition, principal),
        "listening_gaps" => gap_items(conn, definition),
        "recovery_reconciliation" => recovery_items(conn, definition),
        "version_acceptance" => version_items(conn, definition),
        "results_expected" => expected_items(conn, definition, limit),
        _ => Ok(Vec::new()),
    }
}

/// The last recorded state of each check whose owner capability the caller holds, when it
/// is a warning or an alert.
fn alert_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    principal: &Principal,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let mut items = Vec::new();
    for check in ALL_CHECKS {
        if OWN_QUEUE.contains(&check) || principal.require(check.owner()).is_err() {
            continue;
        }
        let Some(last) = lis_ops::health::current_health_event(conn, check)? else {
            continue;
        };
        if matches!(last.state, HealthState::Warning | HealthState::Alert) {
            items.push(item(
                definition,
                "health_check",
                check.as_str(),
                format!("Health check: {}", check.as_str().replace('_', " ")),
                last.state.as_str().to_owned(),
                last.observed_at,
            ));
        }
    }
    Ok(items)
}

fn gap_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_ops::runs::pending_listening_gaps(conn)?
        .into_iter()
        .map(|run| {
            item(
                definition,
                "app_run",
                run.run_id,
                "Listening gap requiring reconciliation".to_owned(),
                "unacknowledged".to_owned(),
                run.started_at,
            )
        })
        .collect())
}

fn recovery_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_ops::recovery::get_recovery_reconciliation(conn)?
        .filter(|view| view.closed.is_none())
        .map(|view| {
            item(
                definition,
                "recovery_reconciliation",
                view.restore_id,
                "Recovery reconciliation after a restore".to_owned(),
                "open".to_owned(),
                view.applied_at,
            )
        })
        .into_iter()
        .collect())
}

fn version_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_ops::updates::list_installed_versions(conn)?
        .into_iter()
        .filter(|version| {
            version.running
                && matches!(
                    version.status,
                    VersionStatus::AwaitingAcceptance | VersionStatus::Unapproved
                )
        })
        .map(|version| {
            let state = match version.status {
                VersionStatus::Unapproved => "unapproved",
                VersionStatus::AwaitingAcceptance
                | VersionStatus::Accepted
                | VersionStatus::Previous => "awaiting_acceptance",
            };
            item(
                definition,
                "app_version",
                &version.version,
                format!("Version {}", version.version),
                state.to_owned(),
                version.first_started_at,
            )
        })
        .collect())
}

fn expected_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    Ok(lis_results::awaiting_results(conn, limit)?
        .into_iter()
        .map(|row| {
            item(
                definition,
                "run_position",
                // `<run>:<position>`, so the screen can open the run (as `run_analyte_qc`).
                format!("{}:{}", row.run_id, row.position_id),
                row.accession_number,
                "awaiting_result".to_owned(),
                row.since,
            )
        })
        .collect())
}

#[cfg(test)]
mod tests;
