//! Reading one page of audit events, of one stream or of every stream, with the display
//! labels the viewer shows: user and instrument names, action and record-type labels, and
//! record labels from their owner modules ([`super::labels`]).

use std::collections::{BTreeSet, HashMap};

use lis_audit::{ByFilter, EventFilter};
use lis_core::UserId;
use lis_store::StoreError;

use super::labels::record_labels;
use super::{
    AuditActorKind, AuditEventView, AuditOutcome, AuditReadError, AuditStream, FilterOptions,
    ListAuditEventsInput,
};

/// One page of events; `patients` is whether the caller may see patient names.
pub(super) fn read_page(
    conn: &rusqlite::Connection,
    input: &ListAuditEventsInput,
    patients: bool,
) -> Result<Vec<AuditEventView>, AuditReadError> {
    let user_id = input.by_user.map(|id| id.to_string());
    let filter = event_filter(input, user_id.as_deref());
    let events = lis_audit::list(conn, &filter).map_err(StoreError::from)?;
    let names = lis_auth::display_names(conn, &user_ids(&events))?;
    let instruments = instrument_names(conn, &events)?;
    let records: BTreeSet<_> = events
        .iter()
        .filter_map(|event| event.record_type.clone().zip(event.record_id.clone()))
        .collect();
    let record_labels = record_labels(conn, &records, &names, patients)?;
    let (available_actions, available_record_types) = available_filters(conn, input.stream)?;
    let available_users: FilterOptions = lis_auth::user_names(conn)?
        .into_iter()
        .map(|(id, name)| (id.to_string(), name))
        .collect();
    Ok(events
        .into_iter()
        .map(|event| {
            let principal_display = principal_display(&event, &names, &instruments);
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
                stream: event.stream.into(),
                principal_display,
                action_label: Some(lis_audit::action_label(&event.action)),
                record_type_label: event
                    .record_type
                    .as_deref()
                    .map(lis_audit::record_type_label),
                record_label,
                available_actions: Some(available_actions.clone()),
                available_record_types: Some(available_record_types.clone()),
                available_users: Some(available_users.clone()),
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

/// Who acted, by name: a user's display name or an instrument's current name, as Bench setup
/// shows it (ticket #4743). Names are read at read time, so a renamed instrument shows its
/// current name; a principal not found has none, and the viewer shows its kind and ID.
fn principal_display(
    event: &lis_audit::StoredEvent,
    users: &HashMap<UserId, String>,
    instruments: &HashMap<String, String>,
) -> Option<String> {
    match event.principal_kind.as_str() {
        "user" => event
            .principal_id
            .parse::<UserId>()
            .ok()
            .and_then(|id| users.get(&id).cloned()),
        "instrument" => instruments.get(&event.principal_id).cloned(),
        _ => None,
    }
}

/// The names of the laboratory's instruments, by ID as audit stores it, read once for the
/// page and only when an instrument acted in `events`.
fn instrument_names(
    conn: &rusqlite::Connection,
    events: &[lis_audit::StoredEvent],
) -> Result<HashMap<String, String>, AuditReadError> {
    if !events
        .iter()
        .any(|event| event.principal_kind == "instrument")
    {
        return Ok(HashMap::new());
    }
    Ok(lis_interfaces::instrument_names(conn)?
        .into_iter()
        .map(|(id, name)| (id.to_string(), name))
        .collect())
}

/// The users who acted in `events` or are their records.
fn user_ids(events: &[lis_audit::StoredEvent]) -> BTreeSet<UserId> {
    events
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
        .collect()
}

/// The engine filter for `input`; `user_id` is its "By" user as stored.
fn event_filter<'a>(input: &'a ListAuditEventsInput, user_id: Option<&'a str>) -> EventFilter<'a> {
    let by = match (user_id, input.by_kind) {
        (Some(id), _) => Some(ByFilter::User(id)),
        (None, Some(AuditActorKind::People)) => Some(ByFilter::People),
        (None, Some(AuditActorKind::System)) => Some(ByFilter::System),
        (None, None) => None,
    };
    EventFilter {
        stream: input.stream.map(AuditStream::stream),
        before: input.before.map(Into::into),
        limit: input.limit,
        action: input.action.as_deref(),
        record_type: input.record_type.as_deref(),
        record_id: input.record_id.as_deref(),
        self_approvals_only: input.self_approvals_only == Some(true),
        recorded_from: input.recorded_from,
        recorded_before: input.recorded_before,
        by,
        outcome: input.outcome.map(AuditOutcome::outcome),
    }
}

fn available_filters(
    conn: &rusqlite::Connection,
    stream: Option<AuditStream>,
) -> Result<(FilterOptions, FilterOptions), AuditReadError> {
    let stream = stream.map(AuditStream::stream);
    let actions = lis_audit::actions(conn, stream)
        .map_err(StoreError::from)?
        .into_iter()
        .map(|code| {
            let label = lis_audit::action_label(&code);
            (code, label)
        })
        .collect();
    let record_types = lis_audit::record_types(conn, stream)
        .map_err(StoreError::from)?
        .into_iter()
        .map(|(code, streams)| {
            let label = record_type_option(&code, &streams, stream.is_none());
            (code, label)
        })
        .collect();
    Ok((actions, record_types))
}

/// A record type's label, followed by its streams when every stream is shown, so the
/// list says where each type is recorded ("Order — Clinical, Access").
fn record_type_option(code: &str, streams: &[lis_audit::Stream], every_stream: bool) -> String {
    let label = lis_audit::record_type_label(code);
    if !every_stream {
        return label;
    }
    let streams: Vec<_> = streams.iter().map(|stream| stream_label(*stream)).collect();
    format!("{label} — {}", streams.join(", "))
}

/// The stream's name in the viewer.
fn stream_label(stream: lis_audit::Stream) -> &'static str {
    match stream {
        lis_audit::Stream::Clinical => "Clinical",
        lis_audit::Stream::Security => "Security",
        lis_audit::Stream::Access => "Access",
    }
}
