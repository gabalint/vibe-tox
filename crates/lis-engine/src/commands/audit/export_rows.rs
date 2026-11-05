//! The text of an audit export: one row per event with what its Changes
//! section says, as CSV or as the `audit-export` PDF document. The screen and
//! the file read the same engine values, so they agree.

use jiff::tz::TimeZone;
use lis_core::UtcTimestamp;
use lis_report::audit_export_v1 as doc;

use super::changes::{AuditChangeStatus, AuditEventChanges};
use super::{AuditEventView, AuditStream};
use crate::pipeline::Failure;
use crate::report_rendering::csv::cell;

const CSV_HEADER: &str = "stream,seq,occurred,occurred_utc,recorded,recorded_utc,user,user_id,\
     action,action_code,outcome,record_type,record,record_id,patient,reason,authority,\
     old_revision,new_revision,changes,details,correlation_id";

/// Printed for an empty value in the PDF's front block.
const NONE: &str = "—";

/// One exported event with its Changes section.
pub(super) struct ExportRow {
    pub(super) event: AuditEventView,
    pub(super) changes: AuditEventChanges,
}

/// The laboratory's name and time zone for formatting.
pub(super) struct Laboratory {
    pub(super) name: String,
    pub(super) time_zone: String,
}

/// Who exported, when, and the bound and filters of the export.
pub(super) struct Provenance {
    pub(super) exported_at: UtcTimestamp,
    pub(super) exported_by: String,
    pub(super) recorded_before: UtcTimestamp,
    pub(super) filters: Vec<(String, String)>,
}

/// An instant to the second in the laboratory's time zone.
pub(super) fn lab_time(at: UtcTimestamp, time_zone: &str) -> Result<String, Failure> {
    let zone = TimeZone::get(time_zone)
        .map_err(|_| Failure::Internal("laboratory time zone".to_owned()))?;
    let instant: jiff::Timestamp = at
        .to_string()
        .parse()
        .map_err(|_| Failure::Internal("audit timestamp".to_owned()))?;
    Ok(instant
        .to_zoned(zone)
        .strftime("%Y-%m-%d %H:%M:%S %Z")
        .to_string())
}

/// The rows as CSV: lab-time and UTC times, labels beside codes, formula-safe cells.
pub(super) fn csv(rows: &[ExportRow], lab: &Laboratory) -> Result<String, Failure> {
    let mut text = String::from(CSV_HEADER);
    text.push_str("\r\n");
    for ExportRow { event, changes } in rows {
        let cells = [
            stream_label(event.stream).to_owned(),
            event.seq.to_string(),
            lab_time(event.occurred_at, &lab.time_zone)?,
            event.occurred_at.to_string(),
            lab_time(event.recorded_at, &lab.time_zone)?,
            event.recorded_at.to_string(),
            event.principal_display.clone().unwrap_or_default(),
            event.principal_id.clone(),
            event.action_label.clone().unwrap_or_default(),
            event.action.clone(),
            outcome_label(&event.outcome).to_owned(),
            event.record_type.clone().unwrap_or_default(),
            event.record_label.clone().unwrap_or_default(),
            event.record_id.clone().unwrap_or_default(),
            patient_text(changes),
            event.reason.clone().unwrap_or_default(),
            event.authority.clone().unwrap_or_default(),
            changes
                .old_revision
                .map(|n| n.to_string())
                .unwrap_or_default(),
            changes
                .new_revision
                .map(|n| n.to_string())
                .unwrap_or_default(),
            changes_text(changes),
            details_text(changes),
            event.correlation_id.clone(),
        ];
        let line: Vec<String> = cells.iter().map(|value| cell(value)).collect();
        text.push_str(&line.join(","));
        text.push_str("\r\n");
    }
    Ok(text)
}

/// The rows as the `audit-export` v1 document.
pub(super) fn document(
    rows: &[ExportRow],
    lab: &Laboratory,
    provenance: &Provenance,
) -> Result<doc::AuditExportDocument, Failure> {
    let events = rows
        .iter()
        .map(|row| event_text(row, lab))
        .collect::<Result<Vec<_>, _>>()?;
    let patient_data = rows.iter().any(|row| row.changes.patient.is_some());
    Ok(doc::AuditExportDocument {
        laboratory_name: lab.name.clone(),
        export: doc::AuditExportHeader {
            exported_at: lab_time(provenance.exported_at, &lab.time_zone)?,
            exported_by: provenance.exported_by.clone(),
            recorded_before: lab_time(provenance.recorded_before, &lab.time_zone)?,
            event_count: rows.len().to_string(),
            patient_data: if patient_data { "yes" } else { "no" }.to_owned(),
        },
        filters: provenance
            .filters
            .iter()
            .map(|(name, value)| doc::AuditExportFilter {
                name: name.clone(),
                value: if value.is_empty() {
                    NONE
                } else {
                    value.as_str()
                }
                .to_owned(),
            })
            .collect(),
        events,
    })
}

fn event_text(row: &ExportRow, lab: &Laboratory) -> Result<doc::AuditExportEvent, Failure> {
    let ExportRow { event, changes } = row;
    let by = match &event.principal_display {
        Some(name) => format!("{name} ({})", event.principal_id),
        None => format!("{} ({})", event.principal_id, event.principal_kind),
    };
    let action = match &event.action_label {
        Some(label) if *label != event.action => format!("{label} ({})", event.action),
        _ => event.action.clone(),
    };
    let record = [
        event.record_type_label.as_deref(),
        event.record_label.as_deref(),
        event.record_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let patient = patient_text(changes);
    let changes_cell = if patient.is_empty() {
        changes_text(changes)
    } else {
        format!("Patient: {patient}. {}", changes_text(changes))
    };
    Ok(doc::AuditExportEvent {
        stream: stream_label(event.stream).to_owned(),
        seq: event.seq.to_string(),
        occurred: lab_time(event.occurred_at, &lab.time_zone)?,
        recorded: lab_time(event.recorded_at, &lab.time_zone)?,
        by,
        action,
        outcome: outcome_label(&event.outcome).to_owned(),
        record,
        reason: event.reason.clone().unwrap_or_default(),
        authority: event.authority.clone().unwrap_or_default(),
        revision: revision_text(changes),
        changes: changes_cell,
        details: details_text(changes),
        correlation_id: event.correlation_id.clone(),
    })
}

fn stream_label(stream: AuditStream) -> &'static str {
    match stream {
        AuditStream::Clinical => "Clinical",
        AuditStream::Security => "Security",
        AuditStream::Access => "Access",
    }
}

fn outcome_label(outcome: &str) -> &str {
    match outcome {
        "succeeded" => "Succeeded",
        "denied" => "Denied",
        "failed" => "Failed",
        other => other,
    }
}

fn revision_text(changes: &AuditEventChanges) -> String {
    changes.new_revision.map_or_else(String::new, |new| {
        let old = changes
            .old_revision
            .map_or(NONE.to_owned(), |old| old.to_string());
        format!("{old} → {new}")
    })
}

/// `Family, Given, born YYYY-MM-DD; identifiers`, or empty.
fn patient_text(changes: &AuditEventChanges) -> String {
    changes
        .patient
        .as_ref()
        .map_or_else(String::new, |patient| {
            let mut text = format!("{}, born {}", patient.name, patient.birth_date);
            if !patient.identifiers.is_empty() {
                text.push_str("; ");
                text.push_str(&patient.identifiers.join("; "));
            }
            text
        })
}

/// `Field: old → new; …`, or what the Changes section says instead.
pub(super) fn changes_text(changes: &AuditEventChanges) -> String {
    let fields = changes
        .fields
        .iter()
        .map(|field| match changes.status {
            AuditChangeStatus::Hidden => format!("{}: hidden", field.field),
            _ => format!(
                "{}: {} → {}",
                field.field,
                field.old.as_deref().unwrap_or(NONE),
                field.new.as_deref().unwrap_or(NONE)
            ),
        })
        .collect::<Vec<_>>()
        .join("; ");
    let status = match changes.status {
        AuditChangeStatus::Changed => "",
        AuditChangeStatus::UnchangedRevision => "This revision changed no field value.",
        AuditChangeStatus::Hidden => {
            "Patient values hidden: the exporter's roles cannot read them."
        }
        AuditChangeStatus::ValuesNotKept => "Values of this record type are not kept in a history.",
        AuditChangeStatus::NoChange => "No data changed.",
    };
    [status, fields.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The recorded details, without one that the changes already show, as the panel lists them.
fn details_text(changes: &AuditEventChanges) -> String {
    changes
        .facts
        .iter()
        .filter(|(label, _)| {
            !changes
                .fields
                .iter()
                .any(|change| change.field.eq_ignore_ascii_case(label))
        })
        .map(|(label, value)| format!("{label}: {value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::audit::AuditFieldChange;

    fn changes(status: AuditChangeStatus) -> AuditEventChanges {
        AuditEventChanges {
            status,
            old_revision: Some(1),
            new_revision: Some(2),
            patient: None,
            fields: vec![AuditFieldChange {
                field: "Phone".to_owned(),
                old: Some("555-0100".to_owned()),
                new: None,
            }],
            facts: vec![("Transport".to_owned(), "tcp".to_owned())],
            read_capabilities: Vec::new(),
        }
    }

    #[test]
    fn changes_read_as_the_panel_shows_them() {
        assert_eq!(
            changes_text(&changes(AuditChangeStatus::Changed)),
            "Phone: 555-0100 → —"
        );
        assert_eq!(
            changes_text(&changes(AuditChangeStatus::Hidden)),
            "Patient values hidden: the exporter's roles cannot read them. Phone: hidden"
        );
        assert_eq!(revision_text(&changes(AuditChangeStatus::Changed)), "1 → 2");
        assert_eq!(
            details_text(&changes(AuditChangeStatus::Changed)),
            "Transport: tcp"
        );
    }

    #[test]
    fn details_skip_what_the_changes_already_show() {
        let mut shown = changes(AuditChangeStatus::Changed);
        shown.facts.push(("phone".to_owned(), "—".to_owned()));
        assert_eq!(details_text(&shown), "Transport: tcp");
    }

    #[test]
    fn times_are_shown_to_the_second_in_the_laboratory_zone() {
        let at = UtcTimestamp::parse("2026-11-05T16:00:05.000000000Z").unwrap();
        assert_eq!(
            lab_time(at, "America/Chicago").unwrap(),
            "2026-11-05 10:00:05 CST"
        );
    }
}
