//! Opt-in approved analytical range gates; no conversion, censoring or inferred interpretation.

use super::read_context::ReviewRead;
use lis_catalog::{MethodAnalyte, RangePolicy};
use lis_core::{DecimalText, GateReason};

use crate::{ResultError, ResultOutcome, ResultRevisionId};

pub(super) fn range_gate(
    read: &mut ReviewRead<'_>,
    id: ResultRevisionId,
) -> Result<Option<GateReason>, ResultError> {
    let (method_id, analyte_id, outcome, magnitude): (
        lis_catalog::MethodVersionId,
        lis_catalog::AnalyteId,
        ResultOutcome,
        Option<DecimalText>,
    ) = read
        .conn
        .prepare_cached(
            "SELECT rr.method_version_id, r.analyte_id, rr.outcome, rr.magnitude \
         FROM res_result_revisions rr JOIN res_results r ON r.id = rr.result_id WHERE rr.id = ?1",
        )?
        .query_row([id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
    let method = read.method(method_id)?;
    let analyte = method
        .analytes
        .iter()
        .find(|analyte| analyte.limits.analyte_id == analyte_id)
        .ok_or(ResultError::NotFound("method analyte"))?;
    Ok(blocked(&analyte.limits, outcome, magnitude.as_ref()).then(|| crate::error::gate(
        "reportable_range_disposition_required",
        "The approved method requires a quantitative result within its reportable bounds. Select suitable repeat evidence or record an explicit nonnumeric disposition; the LIS does not convert or censor the observation.",
    )))
}

fn blocked(
    analyte: &MethodAnalyte,
    outcome: ResultOutcome,
    magnitude: Option<&DecimalText>,
) -> bool {
    if analyte.range_policy == RangePolicy::ReportOnly {
        return false;
    }
    match outcome {
        ResultOutcome::NotPerformed | ResultOutcome::Invalid | ResultOutcome::Indeterminate => {
            false
        }
        ResultOutcome::Censored | ResultOutcome::Qualitative | ResultOutcome::Missing => true,
        ResultOutcome::Quantity => magnitude.is_none_or(|value| {
            let value = value.to_decimal();
            analyte
                .reportable_low
                .as_ref()
                .is_some_and(|low| value < low.to_decimal())
                || analyte
                    .reportable_high
                    .as_ref()
                    .is_some_and(|high| value > high.to_decimal())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_range_is_inclusive_and_never_assumes_censor_or_dilution_semantics() {
        let mut analyte = MethodAnalyte {
            analyte_id: lis_catalog::AnalyteId::generate(),
            unit: Some("ng/mL".into()),
            cutoff: None,
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: Some(DecimalText::parse("5.00").unwrap()),
            reportable_high: Some(DecimalText::parse("1000.00").unwrap()),
            range_policy: RangePolicy::RequireWithinRange,
            reference_interval: None,
            display_scale: None,
        };
        for (text, expected) in [
            ("4.99", true),
            ("5.000", false),
            ("1000", false),
            ("1000.01", true),
        ] {
            assert_eq!(
                blocked(
                    &analyte,
                    ResultOutcome::Quantity,
                    Some(&DecimalText::parse(text).unwrap())
                ),
                expected
            );
        }
        assert!(blocked(
            &analyte,
            ResultOutcome::Censored,
            Some(&DecimalText::parse("5").unwrap())
        ));
        for outcome in [
            ResultOutcome::NotPerformed,
            ResultOutcome::Invalid,
            ResultOutcome::Indeterminate,
        ] {
            assert!(!blocked(&analyte, outcome, None));
        }
        analyte.range_policy = RangePolicy::ReportOnly;
        assert!(!blocked(
            &analyte,
            ResultOutcome::Quantity,
            Some(&DecimalText::parse("1500").unwrap())
        ));
    }
}
