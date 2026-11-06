//! What one audit event changed, for the viewer (REQ-OPS-001).
//!
//! Events hold no field values (ADR 0008); owner modules read the old and new
//! values from the record's own immutable history when an event is viewed.
//! This module reads the event's revisions and details, and holds the shared
//! shapes owner modules answer with. Values in a [`FieldChange`] are record
//! content, possibly PHI: the engine authorizes and audits their read and they
//! are never written back into the audit trail.

use lis_core::Capability;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::{AuditError, Stream};

/// The facts of one stored event that say what it changed.
#[derive(Debug, Clone, PartialEq)]
pub struct EventFacts {
    /// `<module>.<verb>`.
    pub action: String,
    /// `succeeded`, `denied` or `failed`.
    pub outcome: String,
    /// Type of the affected record.
    pub record_type: Option<String>,
    /// ID of the affected record.
    pub record_id: Option<String>,
    /// The record revision before the change.
    pub old_revision: Option<i64>,
    /// The record revision after the change.
    pub new_revision: Option<i64>,
    /// Typed, non-PHI references recorded with the event.
    pub details: Map<String, Value>,
}

/// The event at `seq` of `stream`, if there is one.
pub fn event_facts(
    conn: &Connection,
    stream: Stream,
    seq: i64,
) -> Result<Option<EventFacts>, AuditError> {
    let row = conn
        .prepare_cached(
            "SELECT action, outcome, record_type, record_id, old_revision, new_revision, details \
             FROM audit_events WHERE stream = ?1 AND seq = ?2",
        )?
        .query_row(params![stream.as_str(), seq], |row| {
            Ok((
                EventFacts {
                    action: row.get(0)?,
                    outcome: row.get(1)?,
                    record_type: row.get(2)?,
                    record_id: row.get(3)?,
                    old_revision: row.get(4)?,
                    new_revision: row.get(5)?,
                    details: Map::new(),
                },
                row.get::<_, String>(6)?,
            ))
        })
        .optional()?;
    row.map(|(mut facts, details)| {
        facts.details = details_map(&details)?;
        Ok(facts)
    })
    .transpose()
}

/// The details of the succeeded event of `stream` that brought the record to
/// `revision`, if one did: the state an event recorded before the next change.
pub fn revision_details(
    conn: &Connection,
    stream: Stream,
    record: (&str, &str),
    revision: i64,
) -> Result<Option<Map<String, Value>>, AuditError> {
    let details: Option<String> = conn
        .prepare_cached(
            "SELECT details FROM audit_events WHERE stream = ?1 AND record_type = ?2 \
             AND record_id = ?3 AND new_revision = ?4 AND outcome = 'succeeded' \
             ORDER BY seq DESC LIMIT 1",
        )?
        .query_row(
            params![stream.as_str(), record.0, record.1, revision],
            |row| row.get(0),
        )
        .optional()?;
    details.as_deref().map(details_map).transpose()
}

fn details_map(text: &str) -> Result<Map<String, Value>, AuditError> {
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(AuditError::InvalidDetails),
    }
}

/// An event an owner module is asked about.
#[derive(Debug, Clone, Copy)]
pub struct ChangeQuery<'a> {
    /// `<module>.<verb>`.
    pub action: &'a str,
    /// Type of the affected record.
    pub record_type: &'a str,
    /// ID of the affected record.
    pub record_id: &'a str,
    /// The record revision before the change.
    pub old_revision: Option<i64>,
    /// The record revision after the change.
    pub new_revision: Option<i64>,
    /// The event's details.
    pub details: &'a Map<String, Value>,
}

impl ChangeQuery<'_> {
    /// The revision the event produced; a create event that recorded none made revision 1.
    #[must_use]
    pub fn produced_revision(&self) -> Option<i64> {
        let verb = self.action.rsplit_once('.').map_or("", |(_, verb)| verb);
        self.new_revision
            .or_else(|| (verb.starts_with("create") || verb.starts_with("register")).then_some(1))
    }

    /// An integer detail, such as `assertion_sequence`.
    #[must_use]
    pub fn integer(&self, key: &str) -> Option<i64> {
        self.details.get(key).and_then(Value::as_i64)
    }

    /// A text detail, such as an `identifier_id`.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        self.details.get(key).and_then(Value::as_str)
    }
}

/// One changed field: its label as the record's form shows it, and its value
/// before and after. `None` means empty or not yet recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    /// The field's label.
    pub field: String,
    /// The value before the change.
    pub old: Option<String>,
    /// The value after the change.
    pub new: Option<String>,
}

/// What an owner module read for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordChanges {
    /// The fields that differ, in the record's form order. Empty when the
    /// revision changed no value.
    pub fields: Vec<FieldChange>,
    /// The patient whose record this is, when the values are patient data.
    pub subject_id: Option<String>,
    /// Either capability lets a user read these values; `None` when `view_audit` suffices.
    pub read_access: Option<(Capability, Capability)>,
}

impl RecordChanges {
    /// Configuration or directory values that `view_audit` may read.
    #[must_use]
    pub fn configuration(fields: Vec<FieldChange>) -> Self {
        Self {
            fields,
            subject_id: None,
            read_access: None,
        }
    }

    /// A patient's values, readable with either capability.
    #[must_use]
    pub fn patient(
        fields: Vec<FieldChange>,
        subject_id: impl Into<String>,
        read_access: (Capability, Capability),
    ) -> Self {
        Self {
            fields,
            subject_id: Some(subject_id.into()),
            read_access: Some(read_access),
        }
    }
}

/// The fields whose old and new values differ, keeping their order.
#[must_use]
pub fn differing(
    fields: impl IntoIterator<Item = (String, Option<String>, Option<String>)>,
) -> Vec<FieldChange> {
    fields
        .into_iter()
        .filter(|(_, old, new)| old != new)
        .map(|(field, old, new)| FieldChange { field, old, new })
        .collect()
}

/// A detail value as text: Yes or No, a number, a code or ID, or a list of them.
#[must_use]
pub fn detail_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => "Yes".to_owned(),
        Value::Bool(false) => "No".to_owned(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(detail_text).collect::<Vec<_>>().join(", "),
        Value::Null => String::new(),
        Value::Number(_) | Value::Object(_) => value.to_string(),
    }
}
