//! Printed wording, in one place (brief §0, §3.3; L7, L8). Values are shown
//! exactly as stored: lexical decimals are never reformatted, rounded or
//! converted, and no wording says "Positive" or "Negative" (REQ-RPT-008).

use jiff::Timestamp;
use jiff::tz::TimeZone;
use lis_catalog::{ComponentPurpose, MethodCertainty, ValidityAction};
use lis_core::{Comparator, DecimalText, UtcTimestamp};
use lis_orders::{MedicationSource, SequenceRole};
use lis_report::tox_v2::ToxComparisonCode;
use lis_results::ResultOutcome;
use lis_specimens::{CollectionPrecision, ReceiptDiscrepancyCode};

use crate::error::ReportingError;
use crate::model::LineDisposition;

/// The analyte value fields a result row prints.
#[derive(Debug, Clone, Copy)]
pub struct ResultValue<'a> {
    /// The outcome form.
    pub outcome: ResultOutcome,
    /// The censoring comparator, for censored values.
    pub comparator: Option<Comparator>,
    /// The exact lexical magnitude.
    pub magnitude: Option<&'a DecimalText>,
    /// The qualitative code text as received.
    pub code_text: Option<&'a str>,
}

/// The printed result: the exact lexical value, a censored value such as
/// `<0.50`, the qualitative code as received, or a fixed phrase.
#[must_use]
pub fn result_text(value: ResultValue<'_>) -> String {
    let magnitude = value.magnitude.map_or("", DecimalText::as_str);
    match value.outcome {
        ResultOutcome::Quantity => magnitude.to_owned(),
        ResultOutcome::Censored => {
            let comparator = value.comparator.map_or("", comparator_text);
            format!("{comparator}{magnitude}")
        }
        ResultOutcome::Qualitative => value.code_text.unwrap_or("").to_owned(),
        ResultOutcome::NotPerformed => "Not performed".to_owned(),
        ResultOutcome::Invalid => "Invalid".to_owned(),
        ResultOutcome::Indeterminate => "Indeterminate".to_owned(),
        ResultOutcome::Missing => "No result".to_owned(),
    }
}

fn comparator_text(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Lt => "<",
        Comparator::Le => "<=",
        Comparator::Gt => ">",
        Comparator::Ge => ">=",
    }
}

/// The limits an analyte row prints.
#[derive(Debug, Clone, Copy)]
pub struct Limits<'a> {
    /// Unit text as configured.
    pub unit: Option<&'a str>,
    /// Cutoff.
    pub cutoff: Option<&'a DecimalText>,
    /// Limit of detection.
    pub limit_of_detection: Option<&'a DecimalText>,
    /// Limit of quantitation.
    pub limit_of_quantitation: Option<&'a DecimalText>,
}

/// "Cutoff 50 ng/mL; LOD 2 ng/mL; LOQ 5 ng/mL", or `None` when the analyte
/// has no limit at all (the `limit_missing` gate).
#[must_use]
pub fn limits_text(limits: Limits<'_>) -> Option<String> {
    let unit = limits
        .unit
        .map(|unit| format!(" {unit}"))
        .unwrap_or_default();
    let parts: Vec<String> = [
        ("Cutoff", limits.cutoff),
        ("LOD", limits.limit_of_detection),
        ("LOQ", limits.limit_of_quantitation),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| format!("{label} {}{unit}", value.as_str())))
    .collect();
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The limits column of a validity row whose method version defines no
/// limit: nothing is invented (coordinator decision 2026-10-13).
pub const NO_METHOD_LIMITS: &str = "None defined";

/// The comparison column of a validity row: the current assessment outcome,
/// with its qualifier for the qualifier action.
#[must_use]
pub fn validity_row_text(assessed: Option<(&str, ValidityAction, Option<&str>)>) -> String {
    let (outcome, notes) = validity_text(assessed);
    match notes.first() {
        Some(qualifier) => format!("{outcome}: {qualifier}"),
        None => outcome,
    }
}

/// The comparison wording for a stored classification code (L7).
#[must_use]
pub fn comparison_text(classification: &str) -> &'static str {
    match classification {
        "meets_cutoff" => "Meets cutoff",
        "does_not_meet_cutoff" => "Does not meet cutoff",
        "not_classifiable" => "Not classifiable",
        _ => "—",
    }
}

/// The code of [`comparison_text`]'s wording, which the layout templates
/// colour; `None` for a code without a comparison.
#[must_use]
pub fn comparison_code(classification: &str) -> Option<ToxComparisonCode> {
    match classification {
        "meets_cutoff" => Some(ToxComparisonCode::MeetsCutoff),
        "does_not_meet_cutoff" => Some(ToxComparisonCode::DoesNotMeetCutoff),
        "not_classifiable" => Some(ToxComparisonCode::NotClassifiable),
        _ => None,
    }
}

/// An instant in the laboratory's zone, for example `2026-10-12 09:30 CDT`.
pub fn lab_time(at: UtcTimestamp, time_zone: &str) -> Result<String, ReportingError> {
    let zone = TimeZone::get(time_zone).map_err(|_| ReportingError::LaboratoryTimeZone)?;
    Ok(instant(at)?
        .to_zoned(zone)
        .strftime("%Y-%m-%d %H:%M %Z")
        .to_string())
}

/// The collection time as recorded: date and time at the source's offset,
/// a date only, or `None` when unknown.
pub fn collected_text(
    precision: CollectionPrecision,
    collected_at: Option<UtcTimestamp>,
    utc_offset_minutes: Option<i32>,
    collected_date: Option<&str>,
) -> Result<Option<String>, ReportingError> {
    Ok(match (precision, collected_at, utc_offset_minutes) {
        (CollectionPrecision::Datetime, Some(at), Some(minutes)) => {
            let offset = jiff::tz::Offset::from_seconds(minutes.saturating_mul(60))
                .map_err(|_| ReportingError::Inconsistent("collection offset"))?;
            let local = instant(at)?.to_zoned(TimeZone::fixed(offset));
            Some(local.strftime("%Y-%m-%d %H:%M (UTC%:z)").to_string())
        }
        (CollectionPrecision::Date, _, _) => {
            collected_date.map(|date| format!("{date} (time not recorded)"))
        }
        (CollectionPrecision::Datetime | CollectionPrecision::Unknown, _, _) => None,
    })
}

/// A stored instant as a `jiff` timestamp.
pub(crate) fn instant(at: UtcTimestamp) -> Result<Timestamp, ReportingError> {
    at.to_string()
        .parse()
        .map_err(|_| ReportingError::Inconsistent("timestamp"))
}

/// "FAMILY, Given" or "FAMILY".
#[must_use]
pub fn person_name(family: &str, given: Option<&str>) -> String {
    match given {
        Some(given) if !given.is_empty() => format!("{family}, {given}"),
        _ => family.to_owned(),
    }
}

/// "LC-MS/MS method v3".
#[must_use]
pub fn method_text(name: &str, version: i64) -> String {
    format!("{name} v{version}")
}

/// Method certainty as printed.
#[must_use]
pub fn certainty_text(certainty: MethodCertainty) -> &'static str {
    match certainty {
        MethodCertainty::Presumptive => "Presumptive",
        MethodCertainty::Definitive => "Definitive",
    }
}

/// The component's role: its sequence role, or specimen validity.
#[must_use]
pub fn role_text(purpose: ComponentPurpose, role: SequenceRole) -> &'static str {
    match (purpose, role) {
        (ComponentPurpose::Validity, _) => "Specimen validity",
        (ComponentPurpose::Analytical, SequenceRole::FirstLine) => "First line",
        (ComponentPurpose::Analytical, SequenceRole::Confirmation) => "Confirmation",
    }
}

/// Where a medication list came from.
#[must_use]
pub fn medication_source_text(source: MedicationSource) -> &'static str {
    match source {
        MedicationSource::OrderingClinic => "Ordering clinic",
        MedicationSource::Patient => "Patient",
        MedicationSource::Other => "Other source",
    }
}

/// The fixed note of a test not reported; free-text reasons are never printed (L7).
#[must_use]
pub fn other_test_note(disposition: LineDisposition) -> &'static str {
    match disposition {
        LineDisposition::Reported => "",
        LineDisposition::Pending => "Results not yet available",
        LineDisposition::Held => "Held for laboratory review",
        LineDisposition::Cancelled => "Cancelled",
        LineDisposition::NotActivated => "Reflex condition not met",
        LineDisposition::TriggerPending => "Awaiting reflex decision",
    }
}

/// A receipt discrepancy as printed in the condition notes.
#[must_use]
pub fn discrepancy_text(code: ReceiptDiscrepancyCode) -> &'static str {
    match code {
        ReceiptDiscrepancyCode::Unlabeled => "Received unlabeled",
        ReceiptDiscrepancyCode::LabelMismatch => "Label mismatch at receipt",
        ReceiptDiscrepancyCode::Leaking => "Leaking at receipt",
        ReceiptDiscrepancyCode::InsufficientVolume => "Insufficient volume at receipt",
        ReceiptDiscrepancyCode::TemperatureOrStability => {
            "Temperature or stability concern at receipt"
        }
        ReceiptDiscrepancyCode::ContainerDamaged => "Container damaged at receipt",
        ReceiptDiscrepancyCode::Other => "Other discrepancy at receipt",
    }
}

/// The conditional-acceptance note (REQ-SPC-002).
#[must_use]
pub fn conditional_acceptance_text(policy: &str, scope: &str) -> String {
    format!("Accepted conditionally under {policy}: {scope}")
}

/// The validity outcome and notes (L8): the category label, plus the
/// qualifier text for the qualifier action; "Not assessed" without one.
#[must_use]
pub fn validity_text(
    assessed: Option<(&str, ValidityAction, Option<&str>)>,
) -> (String, Vec<String>) {
    match assessed {
        None => ("Not assessed".to_owned(), Vec::new()),
        Some((label, ValidityAction::Qualifier, Some(qualifier))) => {
            (label.to_owned(), vec![qualifier.to_owned()])
        }
        Some((label, _, _)) => (label.to_owned(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(text: &str) -> DecimalText {
        DecimalText::parse(text).unwrap()
    }

    #[test]
    fn results_keep_their_exact_lexical_form() {
        let value = |outcome, comparator, magnitude: Option<&DecimalText>, code_text| {
            result_text(ResultValue {
                outcome,
                comparator,
                magnitude,
                code_text,
            })
        };
        let trailing = dec("152.40");
        assert_eq!(
            value(ResultOutcome::Quantity, None, Some(&trailing), None),
            "152.40"
        );
        let censored = dec("0.50");
        assert_eq!(
            value(
                ResultOutcome::Censored,
                Some(Comparator::Lt),
                Some(&censored),
                None
            ),
            "<0.50"
        );
        assert_eq!(
            value(ResultOutcome::Qualitative, None, None, Some("DET")),
            "DET"
        );
        assert_eq!(
            value(ResultOutcome::NotPerformed, None, None, None),
            "Not performed"
        );
        assert_eq!(value(ResultOutcome::Missing, None, None, None), "No result");
    }

    #[test]
    fn limits_name_each_limit_and_are_none_without_any() {
        let (cutoff, loq) = (dec("50"), dec("5"));
        let limits = Limits {
            unit: Some("ng/mL"),
            cutoff: Some(&cutoff),
            limit_of_detection: None,
            limit_of_quantitation: Some(&loq),
        };
        assert_eq!(limits_text(limits).unwrap(), "Cutoff 50 ng/mL; LOQ 5 ng/mL");
        let none = Limits {
            unit: Some("ng/mL"),
            cutoff: None,
            limit_of_detection: None,
            limit_of_quantitation: None,
        };
        assert_eq!(limits_text(none), None);
    }

    // L7: never "Positive" or "Negative".
    #[test]
    fn comparison_wording_never_interprets() {
        for code in [
            "meets_cutoff",
            "does_not_meet_cutoff",
            "not_classifiable",
            "not_applicable",
        ] {
            let text = comparison_text(code).to_lowercase();
            assert!(
                !text.contains("positive") && !text.contains("negative"),
                "{text}"
            );
        }
        assert_eq!(comparison_text("not_applicable"), "—");
    }

    #[test]
    fn times_print_in_the_laboratory_zone_and_at_the_source_offset() {
        let at = UtcTimestamp::parse("2026-10-12T14:30:00.000000000Z").unwrap();
        assert_eq!(
            lab_time(at, "America/Chicago").unwrap(),
            "2026-10-12 09:30 CDT"
        );
        let collected =
            collected_text(CollectionPrecision::Datetime, Some(at), Some(-240), None).unwrap();
        assert_eq!(collected.as_deref(), Some("2026-10-12 10:30 (UTC-04:00)"));
        let date_only =
            collected_text(CollectionPrecision::Date, None, None, Some("2026-10-11")).unwrap();
        assert_eq!(date_only.as_deref(), Some("2026-10-11 (time not recorded)"));
        assert!(lab_time(at, "Not/AZone").is_err());
    }
}
