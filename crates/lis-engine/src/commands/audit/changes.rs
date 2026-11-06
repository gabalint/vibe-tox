//! What one audit event changed (REQ-OPS-001): the old and new value of each
//! changed field, read from the record's own history when the event is opened.
//!
//! Events hold no values (ADR 0008). Owner modules read them: identity
//! (patients, laboratory profile, directory), catalog (analytes), and the
//! instrument state its events record (`enabled`, `transport`). Patient values
//! are returned only to a user who also holds the record's read capability;
//! otherwise only the names of the changed fields are. The engine records the
//! view as an `access` event naming the patient before the values leave it.

use lis_audit::{
    AuditDetail, AuditEvent, ChangeQuery, EventFacts, FieldChange, Outcome, RecordChanges, Stream,
};
use lis_core::{Capability, Principal};
use lis_identity::SubjectId;
use lis_store::StoreError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AuditReadError, AuditStream};

/// Input of `get_audit_event_changes`: one event.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetAuditEventChangesInput {
    /// The event's stream.
    pub stream: AuditStream,
    /// Its sequence number in the stream.
    #[ts(type = "number")]
    pub seq: i64,
}

/// What the Changes section says about the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuditChangeStatus {
    /// At least one field changed; `fields` lists them.
    Changed,
    /// The record's history was read and this revision changed no value.
    UnchangedRevision,
    /// Patient values the user's roles cannot read; `fields` has names only.
    Hidden,
    /// A revision was recorded, but this record type keeps no readable history.
    ValuesNotKept,
    /// The event changed no record data (a sign-in, a list, a check).
    NoChange,
}

/// One changed field: its label, and its value before and after (`None`: empty).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditFieldChange {
    /// The field's label, as the record's form shows it.
    pub field: String,
    /// The value before; `None` when empty, new or hidden.
    pub old: Option<String>,
    /// The value after; `None` when empty, removed or hidden.
    pub new: Option<String>,
}

impl From<FieldChange> for AuditFieldChange {
    fn from(change: FieldChange) -> Self {
        Self {
            field: change.field,
            old: change.old,
            new: change.new,
        }
    }
}

/// The patient a change belongs to, as currently identified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditPatient {
    /// The patient record's ID.
    pub subject_id: String,
    /// `Family, Given`.
    pub name: String,
    /// `YYYY-MM-DD`.
    pub birth_date: String,
    /// Active identifiers, as `type (authority): value`.
    pub identifiers: Vec<String>,
}

/// The Changes section of one audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditEventChanges {
    /// What the section says.
    pub status: AuditChangeStatus,
    /// The record revision before the change.
    #[ts(type = "number | null")]
    pub old_revision: Option<i64>,
    /// The record revision after the change.
    #[ts(type = "number | null")]
    pub new_revision: Option<i64>,
    /// The patient, when the record is a patient's and the user may read it.
    pub patient: Option<AuditPatient>,
    /// The changed fields, in the record's form order.
    pub fields: Vec<AuditFieldChange>,
    /// The other facts recorded with the event (codes, counts, IDs), labeled.
    #[ts(type = "Array<[string, string]>")]
    pub facts: Vec<(String, String)>,
    /// When hidden: capability codes, either of which shows the values.
    pub read_capabilities: Vec<String>,
}

/// The changes, and the patient and capability under which values were read.
pub(super) struct Viewed {
    pub(super) changes: AuditEventChanges,
    pub(super) patient_read: Option<(String, Capability)>,
}

/// The Changes section of `input`'s event, for `principal`.
pub(super) fn read_changes(
    conn: &rusqlite::Connection,
    input: GetAuditEventChangesInput,
    principal: &Principal,
) -> Result<Viewed, AuditReadError> {
    let stream = input.stream.stream();
    let facts = lis_audit::event_facts(conn, stream, input.seq)
        .map_err(StoreError::from)?
        .ok_or(AuditReadError::EventNotFound)?;
    let record = record_changes(conn, stream, &facts)?;
    let mut changes = AuditEventChanges {
        status: AuditChangeStatus::NoChange,
        old_revision: facts.old_revision,
        new_revision: facts.new_revision,
        patient: None,
        fields: Vec::new(),
        facts: facts
            .details
            .iter()
            .map(|(key, value)| (lis_audit::detail_label(key), lis_audit::detail_text(value)))
            .collect(),
        read_capabilities: Vec::new(),
    };
    let Some(record) = record else {
        if facts.old_revision.is_some() || facts.new_revision.is_some() {
            changes.status = AuditChangeStatus::ValuesNotKept;
        }
        return Ok(Viewed {
            changes,
            patient_read: None,
        });
    };
    let granted = match record.read_access {
        None => Ok(None),
        Some((first, second)) => [first, second]
            .into_iter()
            .find(|capability| principal.require(*capability).is_ok())
            .map(Some)
            .ok_or([first, second]),
    };
    let capability = match granted {
        Ok(capability) => capability,
        Err(needed) => {
            changes.status = AuditChangeStatus::Hidden;
            changes.fields = record.fields.into_iter().map(names_only).collect();
            changes.read_capabilities = needed.iter().map(|c| c.as_str().to_owned()).collect();
            return Ok(Viewed {
                changes,
                patient_read: None,
            });
        }
    };
    changes.status = if record.fields.is_empty() {
        AuditChangeStatus::UnchangedRevision
    } else {
        AuditChangeStatus::Changed
    };
    changes.fields = record.fields.into_iter().map(Into::into).collect();
    let patient_read = record.subject_id.zip(capability);
    if let Some((subject_id, _)) = &patient_read {
        changes.patient = patient(conn, subject_id)?;
    }
    Ok(Viewed {
        changes,
        patient_read,
    })
}

fn names_only(change: FieldChange) -> AuditFieldChange {
    AuditFieldChange {
        field: change.field,
        old: None,
        new: None,
    }
}

/// The owner module's reading of a succeeded event, when one keeps the values.
fn record_changes(
    conn: &rusqlite::Connection,
    stream: Stream,
    facts: &EventFacts,
) -> Result<Option<RecordChanges>, AuditReadError> {
    let (Some(record_type), Some(record_id)) = (&facts.record_type, &facts.record_id) else {
        return Ok(None);
    };
    if facts.outcome != "succeeded" {
        return Ok(None);
    }
    let query = ChangeQuery {
        action: &facts.action,
        record_type,
        record_id,
        old_revision: facts.old_revision,
        new_revision: facts.new_revision,
        details: &facts.details,
    };
    if let Some(changes) = lis_identity::audit_record_changes(conn, &query)? {
        return Ok(Some(changes));
    }
    if let Some(changes) = lis_catalog::audit_record_changes(conn, &query)? {
        return Ok(Some(changes));
    }
    Ok(instrument_changes(conn, stream, &query)?)
}

/// Instruments keep no history of their settings, but their events record the
/// `enabled` and `transport` state; the earlier value is the one the most
/// recent earlier event of the instrument recorded.
fn instrument_changes(
    conn: &rusqlite::Connection,
    stream: Stream,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, StoreError> {
    let keys: &[&str] = match query.action {
        "interface.set_enabled" => &["enabled"],
        "interface.register_instrument" => &["enabled", "transport"],
        _ => return Ok(None),
    };
    if query.record_type != "instrument" {
        return Ok(None);
    }
    let mut rows = Vec::new();
    for key in keys {
        let new = query.details.get(*key).map(lis_audit::detail_text);
        let old = earlier_value(conn, stream, query, key)?;
        rows.push((lis_audit::detail_label(key), old, new));
    }
    Ok(Some(RecordChanges::configuration(lis_audit::differing(
        rows,
    ))))
}

fn earlier_value(
    conn: &rusqlite::Connection,
    stream: Stream,
    query: &ChangeQuery<'_>,
    key: &str,
) -> Result<Option<String>, StoreError> {
    let record = (query.record_type, query.record_id);
    let mut revision = query.old_revision.unwrap_or(0);
    while revision >= 1 {
        let details = lis_audit::revision_details(conn, stream, record, revision)?;
        if let Some(value) = details.as_ref().and_then(|details| details.get(key)) {
            return Ok(Some(lis_audit::detail_text(value)));
        }
        revision -= 1;
    }
    Ok(None)
}

fn patient(
    conn: &rusqlite::Connection,
    subject_id: &str,
) -> Result<Option<AuditPatient>, AuditReadError> {
    let Ok(id) = subject_id.parse::<SubjectId>() else {
        return Ok(None);
    };
    let summary = match lis_identity::subject_summary(conn, id) {
        Ok(summary) => summary,
        Err(lis_identity::IdentityError::NotFound(_)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let family = summary.family_name.expose();
    Ok(Some(AuditPatient {
        subject_id: subject_id.to_owned(),
        name: summary.given_names.as_ref().map_or_else(
            || family.clone(),
            |given| format!("{family}, {}", given.expose()),
        ),
        birth_date: summary.birth_date.expose().clone(),
        identifiers: summary
            .identifiers
            .iter()
            .map(|identifier| {
                format!(
                    "{} ({}): {}",
                    identifier.identifier_type,
                    identifier.assigning_authority,
                    identifier.value.expose()
                )
            })
            .collect(),
    }))
}

/// The `access` event for opening the changes: the patient when patient
/// values were shown, else the event's stream; the event by stream and number.
pub(super) fn changes_event(
    action: &'static str,
    input: GetAuditEventChangesInput,
    viewed: &Viewed,
) -> AuditEvent {
    let stream = input.stream.stream().as_str();
    let event = AuditEvent::new(Stream::Access, action, Outcome::Succeeded);
    let event = match &viewed.patient_read {
        Some((subject_id, _)) => event.record("subject", subject_id),
        None => event.record("audit_stream", stream),
    };
    event
        .detail("audit_stream", AuditDetail::code(stream))
        .detail("audit_seq", AuditDetail::integer(input.seq))
        .detail(
            "values_shown",
            AuditDetail::boolean(viewed.changes.status != AuditChangeStatus::Hidden),
        )
}
