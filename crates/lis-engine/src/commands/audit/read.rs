//! Reading one page of audit events with the display labels the viewer shows: user names,
//! action and record-type labels, and record labels from their owner modules.

use std::collections::{BTreeMap, BTreeSet};

use lis_audit::EventFilter;
use lis_core::UserId;
use lis_store::StoreError;

use super::{AuditEventView, AuditReadError, AuditStream, FilterOptions, ListAuditEventsInput};

pub(super) fn read_page(
    conn: &rusqlite::Connection,
    input: &ListAuditEventsInput,
) -> Result<Vec<AuditEventView>, AuditReadError> {
    let filter = EventFilter {
        stream: input.stream.stream(),
        before_seq: input.before_seq,
        limit: input.limit,
        action: input.action.as_deref(),
        record_type: input.record_type.as_deref(),
        record_id: input.record_id.as_deref(),
        self_approvals_only: input.self_approvals_only == Some(true),
    };
    let events = lis_audit::list(conn, &filter).map_err(StoreError::from)?;
    let users: BTreeSet<UserId> = events
        .iter()
        .flat_map(|event| {
            let principal = (event.principal_kind == "user")
                .then(|| event.principal_id.parse().ok())
                .flatten();
            let record = (event.record_type.as_deref() == Some("user"))
                .then(|| event.record_id.as_deref()?.parse().ok())
                .flatten();
            [principal, record]
        })
        .flatten()
        .collect();
    let names = lis_auth::display_names(conn, &users)?;
    let record_labels = record_labels(conn, &events, &names)?;
    let (available_actions, available_record_types) = available_filters(conn, input.stream)?;
    Ok(events
        .into_iter()
        .map(|event| {
            let principal_display = (event.principal_kind == "user")
                .then(|| event.principal_id.parse::<UserId>().ok())
                .flatten()
                .and_then(|id| names.get(&id).cloned());
            let record_label = event
                .record_type
                .as_ref()
                .zip(event.record_id.as_ref())
                .and_then(|(record_type, record_id)| {
                    record_labels
                        .get(&(record_type.clone(), record_id.clone()))
                        .cloned()
                });
            AuditEventView {
                stream: input.stream,
                principal_display,
                action_label: Some(lis_audit::action_label(&event.action)),
                record_type_label: event
                    .record_type
                    .as_deref()
                    .map(lis_audit::record_type_label),
                record_label,
                available_actions: Some(available_actions.clone()),
                available_record_types: Some(available_record_types.clone()),
                seq: event.seq,
                occurred_at: event.occurred_at,
                recorded_at: event.recorded_at,
                principal_kind: event.principal_kind,
                principal_id: event.principal_id,
                action: event.action,
                outcome: event.outcome,
                record_type: event.record_type,
                record_id: event.record_id,
                reason: event.reason,
                authority: event.authority,
                correlation_id: event.correlation_id,
            }
        })
        .collect())
}

fn available_filters(
    conn: &rusqlite::Connection,
    stream: AuditStream,
) -> Result<(FilterOptions, FilterOptions), AuditReadError> {
    let actions = lis_audit::actions(conn, stream.stream())
        .map_err(StoreError::from)?
        .into_iter()
        .map(|code| {
            let label = lis_audit::action_label(&code);
            (code, label)
        })
        .collect();
    let record_types = lis_audit::record_types(conn, stream.stream())
        .map_err(StoreError::from)?
        .into_iter()
        .map(|code| {
            let label = lis_audit::record_type_label(&code);
            (code, label)
        })
        .collect();
    Ok((actions, record_types))
}

fn record_labels(
    conn: &rusqlite::Connection,
    events: &[lis_audit::StoredEvent],
    users: &std::collections::HashMap<UserId, String>,
) -> Result<BTreeMap<(String, String), String>, AuditReadError> {
    let records: BTreeSet<_> = events
        .iter()
        .filter_map(|event| event.record_type.clone().zip(event.record_id.clone()))
        .collect();
    let mut labels = BTreeMap::new();
    for (record_type, record_id) in records {
        let label = if record_type == "user" {
            record_id
                .parse::<UserId>()
                .ok()
                .and_then(|id| users.get(&id).cloned())
        } else {
            let mut label = lis_catalog::audit_record_label(conn, &record_type, &record_id)?;
            if label.is_none() {
                label = lis_identity::audit_record_label(conn, &record_type, &record_id)?;
            }
            if label.is_none() {
                label = lis_reporting::audit_record_label(conn, &record_type, &record_id)?;
            }
            if label.is_none() {
                label = lis_specimens::audit_record_label(conn, &record_type, &record_id)?;
            }
            if label.is_none() {
                label = lis_billing::audit_record_label(conn, &record_type, &record_id)?;
            }
            label
        };
        if let Some(label) = label {
            labels.insert((record_type, record_id), label);
        }
    }
    Ok(labels)
}
