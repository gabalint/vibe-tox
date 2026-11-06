//! Find in the audit trail (REQ-OPS-001): the records whose readable values start with the
//! typed text, or whose record ID it is, for the user to pick one and filter the trail by it.
//! Each owner module matches its own values; patients only for users who may read them.
//! The text may be a patient name: it is held as [`Phi`] and never logged or recorded.

use std::collections::{BTreeSet, HashMap};

use lis_audit::{AuditDetail, AuditEvent, AuditRecord, RecordRef, RecordSearch};
use lis_core::{Phi, UserId, UtcTimestamp, required_text};
use lis_store::StoreError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::input::refused;
use super::labels::record_labels;
use super::{AuditReadError, AuditStream, access_event};
use crate::pipeline::Failure;
use crate::registry::FIND_AUDIT_RECORDS;

/// Most records of one type matched by their readable values.
const PER_TYPE: u32 = 20;
/// Most records Find offers.
const MOST_FOUND: usize = 50;
/// Longest text Find searches, in characters.
const TEXT_MAX: usize = 100;

/// One laboratory day, as the instants it starts and ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AuditDay {
    /// The day's start.
    pub start: UtcTimestamp,
    /// The next day's start.
    pub end: UtcTimestamp,
}

/// Input of `find_audit_records`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FindAuditRecordsInput {
    /// The start of a readable value, or a whole record ID; at least two characters, or
    /// one digit. Never logged or recorded.
    pub text: Phi<String>,
    /// Only this record type.
    #[ts(optional)]
    pub record_type: Option<String>,
    /// Only records with events in this stream; every stream when absent.
    #[ts(optional)]
    pub stream: Option<AuditStream>,
    /// The laboratory day the text names, when it is a date: backup runs requested that day.
    #[ts(optional)]
    pub day: Option<AuditDay>,
}

/// One record Find offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FoundAuditRecord {
    /// Its record type code, the filter value.
    pub record_type: String,
    /// Plain wording for `record_type`.
    pub record_type_label: String,
    /// Its record ID, the filter value.
    pub record_id: String,
    /// Its readable label, when its owner has one and the user may see it.
    #[ts(optional)]
    pub label: Option<String>,
    /// The streams with events about it.
    pub streams: Vec<AuditStream>,
    /// When its latest event was appended.
    pub last_recorded_at: UtcTimestamp,
}

/// The answer of `find_audit_records`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FoundAuditRecords {
    /// The records, by type and label.
    pub records: Vec<FoundAuditRecord>,
    /// More records match than are listed.
    pub more: bool,
}

/// Whether `text`, trimmed, is long enough to search: two characters, or one digit.
fn searchable(text: &str) -> bool {
    text.chars().count() >= 2
        || text
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit())
}

/// The details of the search's `access` event: the record type, whether patients and a day
/// were searched, never the text. Refuses text that is too long or not plain, a record type
/// that is not a code, and a day that ends before it starts.
pub(super) fn find_details(
    input: &FindAuditRecordsInput,
    patients: bool,
) -> Result<Vec<(&'static str, AuditDetail)>, Failure> {
    let text = input.text.expose().trim();
    if !text.is_empty() {
        required_text("text", text, TEXT_MAX)
            .map_err(|error| refused("text", &error.code, error.message))?;
    }
    if input.day.is_some_and(|day| day.start >= day.end) {
        return Err(refused(
            "day",
            "out_of_range",
            "Choose a day that ends after it starts.".to_owned(),
        ));
    }
    let mut details = vec![
        (
            "patients_searched",
            AuditDetail::boolean(
                patients && input.record_type.as_deref().is_none_or(|t| t == "subject"),
            ),
        ),
        ("by_day", AuditDetail::boolean(input.day.is_some())),
    ];
    if let Some(record_type) = input.record_type.as_deref() {
        let detail = AuditDetail::machine_token(record_type).map_err(|_| {
            refused(
                "recordType",
                "invalid_machine_token",
                "Choose a record type from the list.".to_owned(),
            )
        })?;
        details.push(("record_type", detail));
    }
    Ok(details)
}

/// The `access` event of a search: its details, how many records it offered and whether
/// there were more.
pub(super) fn find_event(
    stream: Option<AuditStream>,
    found: &FoundAuditRecords,
    details: Vec<(&'static str, AuditDetail)>,
) -> AuditEvent {
    let mut event = access_event(FIND_AUDIT_RECORDS.action, stream)
        .detail("returned", AuditDetail::count(found.records.len()))
        .detail("more", AuditDetail::boolean(found.more));
    for (key, value) in details {
        event = event.detail(key, value);
    }
    event
}

/// The records that match `input`, labeled; `patients` is whether the caller may read them.
pub(super) fn find_records(
    conn: &rusqlite::Connection,
    input: &FindAuditRecordsInput,
    patients: bool,
) -> Result<FoundAuditRecords, AuditReadError> {
    let text = input.text.expose().trim();
    if !searchable(text) {
        return Ok(FoundAuditRecords {
            records: Vec::new(),
            more: false,
        });
    }
    let search = RecordSearch {
        record_type: input.record_type.as_deref(),
        text,
        limit: PER_TYPE,
    };
    let candidates = candidates(conn, &search, input.day, patients)?;
    let mut per_type: HashMap<&str, u32> = HashMap::new();
    for &(record_type, _) in &candidates {
        *per_type.entry(record_type).or_default() += 1;
    }
    let stream = input.stream.map(AuditStream::stream);
    let limit = u32::try_from(MOST_FOUND + 1).unwrap_or(u32::MAX);
    let mut found = lis_audit::audit_records(conn, stream, &search, &candidates, limit)
        .map_err(StoreError::from)?;
    let more = found.len() > MOST_FOUND || per_type.values().any(|count| *count >= PER_TYPE);
    found.truncate(MOST_FOUND);
    Ok(FoundAuditRecords {
        records: labeled(conn, found, patients)?,
        more,
    })
}

/// Every owner module's matches for `search`.
fn candidates(
    conn: &rusqlite::Connection,
    search: &RecordSearch<'_>,
    day: Option<AuditDay>,
    patients: bool,
) -> Result<Vec<RecordRef>, AuditReadError> {
    let mut records = lis_catalog::audit_record_search(conn, search)?;
    records.extend(lis_identity::audit_record_search(conn, search)?);
    records.extend(lis_reporting::audit_record_search(conn, search)?);
    records.extend(lis_specimens::audit_record_search(conn, search)?);
    records.extend(lis_orders::audit_record_search(conn, search)?);
    records.extend(lis_billing::audit_record_search(conn, search)?);
    records.extend(lis_interfaces::audit_record_search(conn, search)?);
    let day = day.map(|day| (day.start, day.end));
    records.extend(lis_ops::audit_records::audit_record_search(
        conn, search, day,
    )?);
    records.extend(lis_auth::audit_record_search(conn, search)?);
    if patients && search.covers("subject") {
        let subjects = lis_identity::search_subjects(
            conn,
            &lis_identity::SubjectSearch {
                query: Some(Phi::new(search.text.to_owned())),
                offset: None,
                identifier: None,
                family_name: None,
                given_names: None,
                birth_date: None,
                limit: PER_TYPE,
            },
        )?;
        records.extend(
            subjects
                .into_iter()
                .map(|subject| ("subject", subject.subject_id.to_string())),
        );
    }
    Ok(records)
}

/// The found records with their labels, by type label, then label.
fn labeled(
    conn: &rusqlite::Connection,
    found: Vec<AuditRecord>,
    patients: bool,
) -> Result<Vec<FoundAuditRecord>, AuditReadError> {
    let records: BTreeSet<_> = found
        .iter()
        .map(|record| (record.record_type.clone(), record.record_id.clone()))
        .collect();
    let users: BTreeSet<UserId> = found
        .iter()
        .filter(|record| record.record_type == "user")
        .filter_map(|record| record.record_id.parse().ok())
        .collect();
    let names = lis_auth::display_names(conn, &users)?;
    let mut labels = record_labels(conn, &records, &names, patients)?;
    let mut records: Vec<_> = found
        .into_iter()
        .map(|record| FoundAuditRecord {
            record_type_label: lis_audit::record_type_label(&record.record_type),
            label: labels.remove(&(record.record_type.clone(), record.record_id.clone())),
            streams: record.streams.into_iter().map(AuditStream::from).collect(),
            last_recorded_at: record.last_recorded_at,
            record_type: record.record_type,
            record_id: record.record_id,
        })
        .collect();
    records.sort_by(|a, b| {
        (&a.record_type_label, &a.label, &a.record_id).cmp(&(
            &b.record_type_label,
            &b.label,
            &b.record_id,
        ))
    });
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> FindAuditRecordsInput {
        FindAuditRecordsInput {
            text: Phi::new(text.to_owned()),
            record_type: None,
            stream: None,
            day: None,
        }
    }

    #[test]
    fn text_needs_two_characters_or_a_digit() {
        assert!(searchable("A2"));
        assert!(searchable("7"));
        assert!(!searchable("A"));
        assert!(!searchable(""));
    }

    #[test]
    fn find_details_never_hold_the_text() {
        let details = find_details(&input("Synthetic Patient"), true).unwrap();
        let keys: Vec<_> = details.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, ["patients_searched", "by_day"]);
    }

    #[test]
    fn find_details_refuse_long_text_and_free_text_record_types() {
        let long = "A".repeat(TEXT_MAX + 1);
        let Err(Failure::Refused { field_errors, .. }) = find_details(&input(&long), false) else {
            panic!("long text was not refused");
        };
        assert_eq!(field_errors[0].field, "text");
        let typed = FindAuditRecordsInput {
            record_type: Some("Patient Name".to_owned()),
            ..input("A26")
        };
        let Err(Failure::Refused { field_errors, .. }) = find_details(&typed, false) else {
            panic!("a free-text record type was not refused");
        };
        assert_eq!(field_errors[0].field, "recordType");
    }
}
