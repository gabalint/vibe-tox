//! How new content relates to the current revision, the one home of this
//! rule (brief §4.1, REQ-RPT-002). Every revision is cumulative and complete.

use crate::error::ReportingError;
use crate::model::{LineDisposition, ReportRelationship};
use crate::snapshot::ReportSnapshot;

/// Compares what the two snapshots print; frozen provenance that prints
/// nothing (subject revision, order evidence version, latest receipt
/// decision) is ignored here, though it still makes a candidate stale:
/// - no current revision → `initial`;
/// - the same printed content → [`ReportingError::NothingChanged`];
/// - a previously reported line changed or is no longer reported, or the
///   specimen, subject, order (with medications), validity or laboratory
///   section changed → `correction`, which needs a reason;
/// - otherwise (lines not reported before became reported or changed
///   status) → `supplement`.
pub fn relationship(
    current: Option<&ReportSnapshot>,
    next: &ReportSnapshot,
) -> Result<ReportRelationship, ReportingError> {
    let Some(current) = current else {
        return Ok(ReportRelationship::Initial);
    };
    let (current, next) = (printed(current), printed(next));
    if current == next {
        return Err(ReportingError::NothingChanged);
    }
    let context_changed = current.specimen != next.specimen
        || current.subject != next.subject
        || current.order != next.order
        || current.validity != next.validity
        || current.laboratory != next.laboratory;
    let reported_changed = current
        .lines
        .iter()
        .filter(|line| line.disposition == LineDisposition::Reported)
        .any(|line| {
            next.lines
                .iter()
                .find(|candidate| candidate.component_id == line.component_id)
                != Some(line)
        });
    Ok(if context_changed || reported_changed {
        ReportRelationship::Correction
    } else {
        ReportRelationship::Supplement
    })
}

/// The snapshot without its unprinted provenance.
fn printed(snapshot: &ReportSnapshot) -> ReportSnapshot {
    let mut printed = snapshot.clone();
    printed.subject.revision = 0;
    printed.order.version = 0;
    printed.specimen.condition_history = crate::condition_review::projected(&printed.specimen);
    printed.specimen.condition_report_review = None;
    printed.specimen.latest_decision_id = None;
    // Historical release dates become known after release; that alone is not
    // changed clinical content requiring a second report.
    for result in printed
        .lines
        .iter_mut()
        .flat_map(|line| &mut line.analytes)
        .filter_map(|a| a.result.as_mut())
    {
        result.first_reported_at = None;
    }
    printed
}

/// Human-readable identification of changed report content, frozen at preparation.
pub fn change_summary(current: Option<&ReportSnapshot>, next: &ReportSnapshot) -> Vec<String> {
    let Some(current) = current else {
        return Vec::new();
    };
    let (current, next) = (printed(current), printed(next));
    let mut changes = Vec::new();
    for (changed, label) in [
        (current.subject != next.subject, "Patient identity"),
        (
            current.specimen != next.specimen,
            "Specimen and collection context",
        ),
        (
            current.order != next.order,
            "Ordering source or supplied medication context",
        ),
        (
            current.validity != next.validity,
            "Specimen validity assessment",
        ),
        (
            current.laboratory != next.laboratory,
            "Laboratory contact/profile information",
        ),
    ] {
        if changed {
            changes.push(label.to_owned());
        }
    }
    for old in current
        .lines
        .iter()
        .filter(|line| line.disposition == LineDisposition::Reported)
    {
        let new = next
            .lines
            .iter()
            .find(|line| line.component_id == old.component_id);
        if new.is_none_or(|line| line.disposition != LineDisposition::Reported) {
            changes.push(format!(
                "{}: previously reported results removed",
                old.orderable_name
            ));
            continue;
        }
        let Some(new) = new else {
            continue;
        };
        let mut old_context = old.clone();
        let mut new_context = new.clone();
        old_context.analytes.clear();
        new_context.analytes.clear();
        if old_context != new_context {
            changes.push(format!(
                "{}: reported method or test context changed",
                old.orderable_name
            ));
        }
        for analyte in &old.analytes {
            if new
                .analytes
                .iter()
                .find(|a| a.analyte_id == analyte.analyte_id)
                != Some(analyte)
            {
                changes.push(format!(
                    "{} — {}: result or reported method information changed",
                    old.orderable_name, analyte.name
                ));
            }
        }
    }
    changes
}
