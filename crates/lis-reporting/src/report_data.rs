//! Maps a snapshot to the exact printed strings of `tox-report` v2 (brief
//! §3.3). Pure: the same snapshot and header always give the same data, so
//! the stored `data_json` explains the issued bytes (NFR-07).

use lis_catalog::{ComponentPurpose, ValidityAction};
use lis_core::{GateReason, UtcTimestamp};
use lis_report::ReportLaboratory;
use lis_report::tox_v2::{
    ToxCollectionObservation, ToxCompleteness, ToxIdentifier, ToxMedication, ToxOrder,
    ToxOtherTest, ToxOtherTestStatus, ToxPractitioner, ToxRelationship, ToxReleasedBy,
    ToxReportData, ToxReportSection, ToxResultGroup, ToxResultRow, ToxSpecimen, ToxSubject,
    ToxSupersedes, ToxValidity,
};

use crate::display::{self, Limits, ResultValue};
use crate::error::{ReportingError, gate};
use crate::model::{LineDisposition, ReportCompleteness, ReportRelationship};
use crate::snapshot::{AnalyteLine, ComponentLine, ReportSnapshot};

/// What a revision prints besides its content.
#[derive(Debug, Clone)]
pub struct Header {
    /// The report number.
    pub report_number: String,
    /// The proposed revision number.
    pub revision_no: i64,
    /// Relationship to the current revision.
    pub relationship: ReportRelationship,
    /// The revision it follows: number and release time.
    pub supersedes: Option<(i64, UtcTimestamp)>,
    /// The correction reason.
    pub amendment_reason: Option<String>,
    /// The issue time printed (the preparation time, L5).
    pub prepared_at: UtcTimestamp,
    /// The signatory's name.
    pub released_by: String,
    /// Changed analytes and context sections compared with the preceding report.
    pub change_summary: Vec<String>,
}

/// The printed data of `snapshot` under `header`.
pub fn tox_report_data(
    snapshot: &ReportSnapshot,
    header: &Header,
) -> Result<ToxReportData, ReportingError> {
    let zone = &snapshot.laboratory.time_zone;
    let completeness = crate::dispositions::completeness(dispositions(snapshot));
    let supersedes = header
        .supersedes
        .map(|(revision, released_at)| -> Result<_, ReportingError> {
            Ok(ToxSupersedes {
                revision: revision.to_string(),
                released_at: display::lab_time(released_at, zone)?,
            })
        })
        .transpose()?;
    let lab = &snapshot.laboratory;
    Ok(ToxReportData {
        report: ToxReportSection {
            number: header.report_number.clone(),
            revision: header.revision_no.to_string(),
            completeness: match completeness {
                ReportCompleteness::Partial => ToxCompleteness::Partial,
                ReportCompleteness::Final => ToxCompleteness::Final,
            },
            relationship: match header.relationship {
                ReportRelationship::Initial => ToxRelationship::Initial,
                ReportRelationship::Supplement => ToxRelationship::Supplement,
                ReportRelationship::Correction => ToxRelationship::Correction,
            },
            supersedes,
            amendment_reason: header.amendment_reason.clone(),
            issued_at: display::lab_time(header.prepared_at, zone)?,
            generated_at: Some(display::lab_time(header.prepared_at, zone)?),
            change_summary: header.change_summary.clone(),
        },
        laboratory: ReportLaboratory {
            name: lab.name.clone(),
            address_lines: lab.address_lines.clone(),
            clia_id: lab.clia_id.clone(),
            director: lab.director.clone(),
            phone: lab.phone.clone(),
        },
        subject: subject(snapshot),
        order: order(snapshot),
        specimen: specimen(snapshot)?,
        medications: snapshot
            .order
            .medications
            .iter()
            .map(|m| ToxMedication {
                name: m.name.clone(),
                source: display::medication_source_text(m.source).to_owned(),
                as_of: m.as_of.clone(),
            })
            .collect(),
        result_groups: result_groups(snapshot)?,
        other_tests: other_tests(snapshot),
        validity: validity(snapshot),
        released_by: ToxReleasedBy {
            name: header.released_by.clone(),
        },
    })
}

/// Every line's disposition: components, then trigger lines.
pub(crate) fn dispositions(snapshot: &ReportSnapshot) -> impl Iterator<Item = LineDisposition> {
    snapshot
        .lines
        .iter()
        .map(|line| line.disposition)
        .chain(snapshot.triggers.iter().map(|t| t.disposition))
}

/// The printed limits of an analyte, or `None` when it has none.
pub(crate) fn limits(analyte: &AnalyteLine) -> Option<String> {
    display::limits_text(Limits {
        unit: analyte.unit.as_deref(),
        cutoff: analyte.cutoff.as_ref(),
        limit_of_detection: analyte.limit_of_detection.as_ref(),
        limit_of_quantitation: analyte.limit_of_quantitation.as_ref(),
    })
}

fn subject(snapshot: &ReportSnapshot) -> ToxSubject {
    let subject = &snapshot.subject;
    ToxSubject {
        name: display::person_name(&subject.family_name, subject.given_names.as_deref()),
        birth_date: subject.birth_date.clone(),
        sex: subject.sex.clone(),
        identifiers: subject
            .identifiers
            .iter()
            .map(|i| ToxIdentifier {
                identifier_type: i.identifier_type.clone(),
                value: i.value.clone(),
                authority: i.authority.clone(),
            })
            .collect(),
    }
}

fn order(snapshot: &ReportSnapshot) -> ToxOrder {
    let order = &snapshot.order;
    ToxOrder {
        practitioner: ToxPractitioner {
            name: order.practitioner_name.clone(),
            npi: order.practitioner_npi.clone(),
        },
        client: order.client_name.clone(),
        location: order.location_name.clone(),
        placer_order: order.placer_order_id.clone(),
    }
}

fn specimen(snapshot: &ReportSnapshot) -> Result<ToxSpecimen, ReportingError> {
    let specimen = &snapshot.specimen;
    let mut condition_notes: Vec<String> = specimen
        .discrepancy_codes
        .iter()
        .map(|code| display::discrepancy_text(*code).to_owned())
        .collect();
    let reported_conditions = crate::condition_review::projected(specimen);
    if !reported_conditions.is_empty() {
        condition_notes.clear();
        for condition in &reported_conditions {
            let description = condition
                .description
                .as_deref()
                .map(|text| format!(": {text}"))
                .unwrap_or_default();
            condition_notes.push(format!(
                "{}{} (receipt decision: {}; recorded {})",
                display::discrepancy_text(condition.code),
                description,
                match condition.decision {
                    lis_specimens::ReceiptDecision::Held => "held",
                    lis_specimens::ReceiptDecision::Accepted => "accepted",
                    lis_specimens::ReceiptDecision::AcceptedConditionally =>
                        "accepted conditionally",
                    lis_specimens::ReceiptDecision::Rejected => "rejected",
                },
                display::lab_time(condition.recorded_at, &snapshot.laboratory.time_zone)?
            ));
        }
    }
    if let (Some(policy), Some(scope)) = (&specimen.conditional_policy, &specimen.conditional_scope)
    {
        condition_notes.push(display::conditional_acceptance_text(policy, scope));
    }
    Ok(ToxSpecimen {
        accession: specimen.accession_number.clone(),
        matrix: specimen.specimen_type_name.clone(),
        container_labels: specimen.container_labels.clone(),
        collected: display::collected_text(
            specimen.collection_precision,
            specimen.collected_at,
            specimen.utc_offset_minutes,
            specimen.collected_date.as_deref(),
        )?,
        received: Some(display::lab_time(
            specimen.received_at,
            &snapshot.laboratory.time_zone,
        )?),
        condition_notes,
        collection_observations: specimen
            .observations
            .iter()
            .map(|o| ToxCollectionObservation {
                code: o.code.clone(),
                value: o.value.clone(),
                observed_by: o.observed_by.clone(),
            })
            .collect(),
    })
}

fn result_groups(snapshot: &ReportSnapshot) -> Result<Vec<ToxResultGroup>, ReportingError> {
    snapshot
        .lines
        .iter()
        .filter(|line| line.disposition == LineDisposition::Reported)
        .map(|line| {
            Ok(ToxResultGroup {
                name: line.orderable_name.clone(),
                method: display::method_text(&line.method_name, line.method_version),
                certainty: display::certainty_text(line.certainty).to_owned(),
                role: display::role_text(line.purpose, line.sequence_role).to_owned(),
                report_notes: line.report_notes.clone(),
                rows: rows(line, snapshot)?,
            })
        })
        .collect()
}

/// Analytical rows print limits and the comparison with the cutoff. A
/// validity row prints the method limits only when its method version
/// defines them, and the current validity assessment instead of a
/// comparison (coordinator decision 2026-10-13).
fn rows(
    line: &ComponentLine,
    snapshot: &ReportSnapshot,
) -> Result<Vec<ToxResultRow>, ReportingError> {
    line.analytes
        .iter()
        .map(|analyte| {
            let result = analyte
                .result
                .as_ref()
                .ok_or(ReportingError::Inconsistent("reported line without result"))?;
            let (limits, comparison, comparison_code) = match line.purpose {
                ComponentPurpose::Analytical => (
                    limits(analyte)
                        .ok_or_else(|| ReportingError::GateBlocked(vec![limit_missing()]))?,
                    display::comparison_text(&result.classification).to_owned(),
                    display::comparison_code(&result.classification),
                ),
                ComponentPurpose::Validity => (
                    limits(analyte).unwrap_or_else(|| display::NO_METHOD_LIMITS.to_owned()),
                    display::validity_row_text(validity_assessed(snapshot)),
                    None,
                ),
            };
            Ok(ToxResultRow {
                analyte: analyte.name.clone(),
                result: display::result_text(ResultValue {
                    outcome: result.outcome,
                    comparator: result.comparator,
                    magnitude: result.magnitude.as_ref(),
                    code_text: result.code_text.as_deref(),
                }),
                units: result.units_text.clone().unwrap_or_default(),
                limits,
                comparison,
                comparison_code,
                recorded_at: result
                    .recorded_at
                    .map(|at| display::lab_time(at, &snapshot.laboratory.time_zone))
                    .transpose()?,
                first_reported_at: result
                    .first_reported_at
                    .map(|at| display::lab_time(at, &snapshot.laboratory.time_zone))
                    .transpose()?,
                reference_interval: analyte.reference_interval.clone(),
                performing_sites: performing_sites(result),
            })
        })
        .collect()
}

/// REQ-RPT-008: an analytical analyte needs a cutoff or limit to be reported.
pub(crate) fn limit_missing() -> GateReason {
    gate(
        "limit_missing",
        "A reported analyte has no cutoff or limit in its method version.",
    )
}

fn other_tests(snapshot: &ReportSnapshot) -> Vec<ToxOtherTest> {
    let components = snapshot
        .lines
        .iter()
        .map(|line| (line.orderable_name.as_str(), line.disposition));
    let triggers = snapshot
        .triggers
        .iter()
        .map(|line| (line.orderable_name.as_str(), line.disposition));
    components
        .chain(triggers)
        .filter_map(|(name, disposition)| {
            Some(ToxOtherTest {
                name: name.to_owned(),
                status: other_status(disposition)?,
                note: display::other_test_note(disposition).to_owned(),
            })
        })
        .collect()
}

fn other_status(disposition: LineDisposition) -> Option<ToxOtherTestStatus> {
    match disposition {
        LineDisposition::Reported => None,
        LineDisposition::Pending => Some(ToxOtherTestStatus::Pending),
        LineDisposition::Held => Some(ToxOtherTestStatus::Held),
        LineDisposition::Cancelled => Some(ToxOtherTestStatus::Cancelled),
        LineDisposition::NotActivated => Some(ToxOtherTestStatus::NotActivated),
        LineDisposition::TriggerPending => Some(ToxOtherTestStatus::TriggerPending),
    }
}

fn validity(snapshot: &ReportSnapshot) -> ToxValidity {
    let (outcome, notes) = display::validity_text(validity_assessed(snapshot));
    ToxValidity { outcome, notes }
}

fn validity_assessed(snapshot: &ReportSnapshot) -> Option<(&str, ValidityAction, Option<&str>)> {
    snapshot
        .validity
        .as_ref()
        .map(|v| (v.label.as_str(), v.action, v.qualifier_text.as_deref()))
}

fn performing_sites(result: &crate::snapshot::ResultSection) -> Vec<String> {
    if result.outcome == lis_results::ResultOutcome::NotPerformed {
        return vec!["Not performed".to_owned()];
    }
    let mut sites: Vec<String> = result
        .performing_sites
        .iter()
        .map(|site| {
            format!(
                "{}; {}; CLIA {} (profile version {})",
                site.name,
                site.address_lines.join(", "),
                site.clia_id,
                site.version
            )
        })
        .collect();
    if result.performing_site_unknown || sites.is_empty() {
        sites.push("Performing location not established in legacy run evidence".to_owned());
    }
    sites
}
