//! The row rules of result files (result files brief F7–F11, §5): the
//! not-a-result list, calibrator and blank rows the LIS cannot place, the
//! sample-type cross-check, value words, units and the dilution factor.
//! Value texts are read only through [`read_value`], the one home of F7.

use lis_core::{Decimal, DecimalText};

use super::planning::{Planner, quarantine};
use super::*;
use crate::delimited::{FileValueReading, read_value};
use crate::result_value::ValueRule;

/// F10: an analyte name on the template's not-a-result list.
pub(super) fn not_result(layout: &FileLayout, row: &ExtractedRow) -> bool {
    row.test_code
        .as_ref()
        .is_some_and(|name| layout.not_results.contains(name))
}

/// F8: with no units column, every row carries the unit the template
/// declares, recorded as declared; the two are exclusive.
pub(super) fn apply_declared_unit(layout: &FileLayout, row: &mut ExtractedRow) {
    if let Some(unit) = &layout.declared_unit {
        row.units_text = Some(unit.clone());
    }
}

/// F9: the role the row's sample-type word maps to; an unmapped word or an
/// empty cell is untyped.
fn typed_role(layout: &FileLayout, row: &ExtractedRow) -> Option<PositionRole> {
    let word = row.sample_type_text.as_deref()?;
    layout
        .sample_types
        .iter()
        .find(|typed| typed.word == word)
        .map(|typed| typed.role)
}

/// F9: a typed row placed on a position of another role.
pub(super) fn role_mismatch(layout: &FileLayout, row: &ExtractedRow, role: PositionRole) -> bool {
    typed_role(layout, row).is_some_and(|typed| typed != role)
}

/// A row whose label matches no open position (F9): a calibrator or blank
/// row whose label is no container's and no lot's is a batch row not placed;
/// anything else is `unknown_sample`.
pub(super) fn unplaced(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    row: ExtractedRow,
) -> Result<Plan, ResultError> {
    let batch_row = planner.layout.is_some_and(|layout| {
        matches!(
            typed_role(layout, &row),
            Some(PositionRole::Calibrator | PositionRole::Blank)
        )
    });
    if batch_row && !known_label(ctx, planner, row.sample_label.as_deref())? {
        return Ok(Plan::SetAside {
            row: Box::new(row),
            kind: SetAsideKind::BatchRowNotPlaced,
            reason: None,
        });
    }
    Ok(quarantine(row, None, None, QuarantineReason::UnknownSample))
}

/// Whether `label` is a container label or a lot's sample label in the LIS.
fn known_label(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    label: Option<&str>,
) -> Result<bool, ResultError> {
    let Some(label) = label else {
        return Ok(false);
    };
    let container: bool = ctx
        .tx()
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM spc_v_containers WHERE label_code = ?1)")?
        .query_row([label], |row| row.get(0))?;
    if container {
        return Ok(true);
    }
    if planner.lot_labels.is_none() {
        let labels = lis_analytical::list_lots(ctx.tx(), false, ctx.now())?
            .into_iter()
            .filter_map(|lot| lot.sample_label)
            .collect();
        planner.lot_labels = Some(labels);
    }
    Ok(planner
        .lot_labels
        .as_ref()
        .is_some_and(|labels| labels.contains(label)))
}

/// The value checks of a file row, in the brief's order (§5 steps 4–7): an
/// approved word (never on a dilution, and the method version must have its
/// limit), the unit unless a word, the value with the zero rule, then the
/// dilution factor.
pub(super) fn file_value(
    ctx: &CommandContext<'_>,
    planner: &Planner<'_>,
    row: &ExtractedRow,
    method: &MethodVersionView,
    analyte: &MethodAnalyte,
    execution_id: lis_analytical::ExecutionId,
) -> Result<Result<ResultValue, QuarantineReason>, ResultError> {
    let layout = planner
        .layout
        .ok_or(ResultError::WrongState("file layout"))?;
    let factor = lis_analytical::execution_dilution_factor(ctx.tx(), execution_id)?;
    let text = row.value_text.as_deref().unwrap_or_default();
    let reading = read_value(text, &layout.value_words, method.output_scale);
    let value = if let FileValueReading::Word(meaning) = reading {
        let rule = ValueRule {
            registration_id: planner.registration.registration_id,
            word: text.to_owned(),
        };
        match ResultValue::from_word(meaning, analyte, rule, factor.as_ref()) {
            Ok(value) => value,
            Err(refusal) => return Ok(Err(refusal.quarantine_reason())),
        }
    } else {
        if analyte.unit != row.units_text {
            return Ok(Err(QuarantineReason::IncompatibleUnit));
        }
        let FileValueReading::Value(value) = reading else {
            return Ok(Err(QuarantineReason::MalformedValue));
        };
        ResultValue {
            value: Some(value),
            units: row.units_text.clone(),
            rule: None,
        }
    };
    if planner.check_dilution
        && layout.columns.dilution.is_some()
        && !dilution_matches(row.dilution_text.as_deref(), factor.as_ref())
    {
        return Ok(Err(QuarantineReason::DilutionMismatch));
    }
    Ok(Ok(value))
}

/// F11: the file's dilution factor equals the execution's numerically; an
/// execution without a factor matches an empty cell or 1. The LIS never
/// multiplies.
fn dilution_matches(text: Option<&str>, factor: Option<&DecimalText>) -> bool {
    let Some(text) = text else {
        return factor.is_none();
    };
    let expected = factor.map_or(Decimal::ONE, DecimalText::to_decimal);
    DecimalText::parse(text).is_ok_and(|found| found.to_decimal() == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factor(text: &str) -> DecimalText {
        DecimalText::parse(text).unwrap()
    }

    #[test]
    fn an_execution_without_a_factor_matches_an_empty_cell_or_one() {
        for text in [None, Some("1"), Some("1.0"), Some("01.000")] {
            assert!(dilution_matches(text, None), "{text:?}");
        }
        for text in ["10", "0", "2", "one", " 1", "1,0"] {
            assert!(!dilution_matches(Some(text), None), "{text}");
        }
    }

    #[test]
    fn a_dilution_factor_must_be_equal_numerically() {
        let ten = factor("10");
        assert!(dilution_matches(Some("10"), Some(&ten)));
        assert!(dilution_matches(Some("10.00"), Some(&ten)));
        assert!(!dilution_matches(None, Some(&ten)));
        assert!(!dilution_matches(Some("1"), Some(&ten)));
        assert!(!dilution_matches(Some("1E1"), Some(&ten)));
    }
}
