//! Re-export semantics on the ASTM route (brief §3.5, REQ-ANA-003): how a
//! received row relates to the row already stored for the same position and
//! test code. Pure; `lis-results` reuses it for patient rows.

use crate::model::RevisionIndicator;

/// ASTM E1394 result status `C`: a correction of a previously transmitted result.
const CORRECTION_STATUS: &str = "C";

/// The compared content of one received row, exactly as received.
///
/// Metadata such as the operator or completion time is not compared: a
/// re-export that changes only those is a duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    /// The value text.
    pub value_text: Option<String>,
    /// The units text.
    pub units_text: Option<String>,
    /// The abnormal flags text.
    pub flags_text: Option<String>,
    /// The result status text, such as `F` or `C`.
    pub status_text: Option<String>,
}

/// What to do with a received row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowDecision {
    /// Nothing is stored for this position and test code: store it.
    New,
    /// Identical to the stored row: count it, store nothing.
    Duplicate,
    /// A correction the registration declares: store it superseding the old row.
    Correction,
    /// A changed row the registration cannot classify: keep the old row and
    /// quarantine the new one for a person to decide (R5).
    Conflict,
}

/// Decides how `incoming` relates to the current stored row `existing`.
///
/// A changed row is a correction only when the registration declares
/// `result_status_correction` and the row carries status `C`; otherwise it
/// is a conflict. Nothing is trimmed or normalized before comparing.
#[must_use]
pub fn decide(
    existing: Option<&SourceRow>,
    incoming: &SourceRow,
    indicator: RevisionIndicator,
) -> RowDecision {
    let Some(existing) = existing else {
        return RowDecision::New;
    };
    if existing == incoming {
        return RowDecision::Duplicate;
    }
    let marked_correction = incoming.status_text.as_deref() == Some(CORRECTION_STATUS);
    match indicator {
        RevisionIndicator::ResultStatusCorrection if marked_correction => RowDecision::Correction,
        RevisionIndicator::ResultStatusCorrection | RevisionIndicator::None => {
            RowDecision::Conflict
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(value: &str, status: &str) -> SourceRow {
        SourceRow {
            value_text: Some(value.to_owned()),
            units_text: Some("ng/mL".to_owned()),
            flags_text: None,
            status_text: Some(status.to_owned()),
        }
    }

    const NONE: RevisionIndicator = RevisionIndicator::None;
    const CORRECTION: RevisionIndicator = RevisionIndicator::ResultStatusCorrection;

    /// The truth table of brief §3.5, written from the brief.
    #[test]
    fn decide_follows_the_re_export_table() {
        let stored = row("120", "F");
        let cases = [
            // No stored row: new, whatever the indicator or status.
            (None, row("120", "F"), NONE, RowDecision::New),
            (None, row("120", "C"), CORRECTION, RowDecision::New),
            // Identical row: duplicate.
            (Some(&stored), row("120", "F"), NONE, RowDecision::Duplicate),
            (
                Some(&stored),
                row("120", "F"),
                CORRECTION,
                RowDecision::Duplicate,
            ),
            // Changed row with status C and the correction indicator: correction.
            (
                Some(&stored),
                row("95", "C"),
                CORRECTION,
                RowDecision::Correction,
            ),
            // Changed row without the indicator: conflict, even with status C.
            (Some(&stored), row("95", "C"), NONE, RowDecision::Conflict),
            (Some(&stored), row("95", "F"), NONE, RowDecision::Conflict),
            // Changed row with the indicator but no correction status: conflict.
            (
                Some(&stored),
                row("95", "F"),
                CORRECTION,
                RowDecision::Conflict,
            ),
        ];
        for (existing, incoming, indicator, expected) in cases {
            assert_eq!(
                decide(existing, &incoming, indicator),
                expected,
                "{existing:?} {incoming:?} {indicator:?}"
            );
        }
    }

    #[test]
    fn nothing_is_trimmed_or_normalized_before_comparing() {
        let stored = row("120", "F");
        for changed in [row("120 ", "F"), row("120.0", "F"), row("120", "f")] {
            assert_eq!(decide(Some(&stored), &changed, NONE), RowDecision::Conflict);
        }
        let mut units = row("120", "F");
        units.units_text = Some("ng/ml".to_owned());
        assert_eq!(decide(Some(&stored), &units, NONE), RowDecision::Conflict);
        let mut flags = row("120", "F");
        flags.flags_text = Some("H".to_owned());
        assert_eq!(decide(Some(&stored), &flags, NONE), RowDecision::Conflict);
    }

    #[test]
    fn an_absent_value_differs_from_an_empty_or_present_one() {
        let mut absent = row("120", "X");
        absent.value_text = None;
        assert_eq!(
            decide(Some(&absent), &absent.clone(), NONE),
            RowDecision::Duplicate
        );
        assert_eq!(
            decide(Some(&absent), &row("120", "X"), NONE),
            RowDecision::Conflict
        );
    }
}
