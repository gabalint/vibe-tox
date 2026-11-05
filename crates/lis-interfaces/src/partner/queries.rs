//! Connections as configured and as working, with their history.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

use super::connection_views::{
    ConnectionChangeView, ConnectionStatus, LastAttempt, PartnerConnection,
    PartnerConnectionDetail, PartnerVerification,
};
use super::error::PartnerError;
use super::model::{PartnerConnectionId, PartnerMessageType};
use super::received_queries::intake_counts;
use super::repo::{self, ConnectionRow};
use crate::model::ChangeStamp;

/// Every connection by name (`manage_configuration`, checked by the engine).
pub fn list_partner_connections(conn: &Connection) -> Result<Vec<PartnerConnection>, PartnerError> {
    repo::connections(conn, None)?
        .into_iter()
        .map(|row| connection_view(conn, row))
        .collect()
}

/// Whether any connection, started or stopped, carries `message_type`.
pub fn carries(conn: &Connection, message_type: PartnerMessageType) -> Result<bool, PartnerError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_partner_connections WHERE message_type = ?1)",
        [message_type],
        |row| row.get(0),
    )?)
}

/// One connection with its history, newest first.
pub fn get_partner_connection(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<PartnerConnectionDetail, PartnerError> {
    let row = repo::load_connection(conn, id)?.ok_or(PartnerError::NotFound("connection"))?;
    let changes = repo::changes(conn, id)?;
    let snapshots: Vec<Value> = changes
        .iter()
        .map(|change| serde_json::from_str(&change.settings))
        .collect::<Result<_, _>>()?;
    let history = changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| ConnectionChangeView {
            revision: change.revision,
            change: change.change,
            changed: ChangeStamp {
                by: change.changed_by,
                by_display: None,
                at: change.changed_at,
            },
            reason: change.reason,
            fields: snapshots
                .get(index + 1)
                .map(|before| changed_fields(before, &snapshots[index]))
                .unwrap_or_default(),
        })
        .collect();
    Ok(PartnerConnectionDetail {
        connection: connection_view(conn, row)?,
        history,
    })
}

/// The settings whose value differs between two snapshots, by name.
fn changed_fields(before: &Value, after: &Value) -> Vec<String> {
    let (Some(before), Some(after)) = (before.as_object(), after.as_object()) else {
        return Vec::new();
    };
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}

/// An outgoing connection's last write attempt, its messages not written yet and its
/// failing ones; synthetic tests are not counted.
fn delivery_counts(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<(Option<LastAttempt>, u32, u32), PartnerError> {
    let last_attempt = conn
        .prepare_cached(
            "SELECT last_attempt_at, last_error_code FROM ifc_partner_deliveries \
             WHERE connection_id = ?1 AND last_attempt_at IS NOT NULL \
             ORDER BY last_attempt_at DESC LIMIT 1",
        )?
        .query_row([connection_id], |found| {
            Ok(LastAttempt {
                at: found.get(0)?,
                error_code: found.get(1)?,
            })
        })
        .optional()?;
    let (waiting, failing): (u32, u32) = conn
        .prepare_cached(
            "SELECT COALESCE(SUM(state = 'waiting'), 0), \
             COALESCE(SUM(state = 'failed' OR (state = 'waiting' AND attempts > 0)), 0) \
             FROM ifc_partner_deliveries WHERE connection_id = ?1 AND reason <> 'test'",
        )?
        .query_row([connection_id], |found| Ok((found.get(0)?, found.get(1)?)))?;
    Ok((last_attempt, waiting, failing))
}

fn connection_view(
    conn: &Connection,
    row: ConnectionRow,
) -> Result<PartnerConnection, PartnerError> {
    let verification =
        repo::verification(conn, row.id, &row.destination)?.map(|check| PartnerVerification {
            method: check.method,
            verified: ChangeStamp {
                by: check.verified_by,
                by_display: None,
                at: check.verified_at,
            },
            self_approval: check.self_approval,
        });
    let (last_attempt, waiting, failing) = if row.settings.message_type.incoming() {
        intake_counts(conn, row.id)?
    } else {
        delivery_counts(conn, row.id)?
    };
    let status = if !row.active {
        ConnectionStatus::Stopped
    } else if last_attempt
        .as_ref()
        .is_some_and(|attempt| attempt.error_code.is_some())
    {
        ConnectionStatus::Error
    } else {
        ConnectionStatus::Active
    };
    let (by, at) = repo::last_change(conn, row.id)?;
    let mut allowed_actions = vec![
        "update_partner_connection".to_owned(),
        "set_partner_connection_active".to_owned(),
        "test_partner_folder".to_owned(),
    ];
    // An incoming connection only reads: the LIS sends it nothing.
    if !row.settings.message_type.incoming() {
        allowed_actions.push("send_partner_test_message".to_owned());
    }
    if verification.is_none() {
        allowed_actions.push("record_partner_destination_check".to_owned());
    }
    let (processed_folder, failed_folder) = row.moved_to.map_or((None, None), |moved_to| {
        (Some(moved_to.processed), Some(moved_to.failed))
    });
    Ok(PartnerConnection {
        id: row.id,
        settings: row.settings,
        folder: row.folder,
        processed_folder,
        failed_folder,
        has_password: row.sealed_password.is_some(),
        active: row.active,
        revision: row.revision,
        status,
        verification,
        last_attempt,
        waiting,
        failing,
        last_change: ChangeStamp {
            by,
            by_display: None,
            at,
        },
        allowed_actions,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn history_names_the_settings_that_changed() {
        let before = json!({"name": "A", "folder": "/x", "rules": [], "active": true});
        let after = json!({"name": "A", "folder": "/y", "rules": [1], "active": true});
        assert_eq!(changed_fields(&before, &after), ["folder", "rules"]);
    }
}
