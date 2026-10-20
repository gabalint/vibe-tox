//! The patient-row checks in the brief's order (M2 brief §3.2 step 5;
//! result files brief §5): analyte mapped, in the method, in the ordered
//! scope (a file row outside it is kept, F17), then the value, unit and
//! dilution checks of the source's format.

use super::file_rows;
use super::planning::{Planner, quarantine};
use super::*;

pub(super) struct PatientDetails {
    pub(super) specimen_id: SpecimenId,
    pub(super) analyte_id: AnalyteId,
    pub(super) method: MethodVersionView,
    pub(super) analyte: MethodAnalyte,
    pub(super) components: Vec<ScopeComponent>,
    pub(super) value: ResultValue,
}

/// Why a placed patient row is not imported.
pub(super) struct Refusal {
    analyte: Option<AnalyteId>,
    reason: QuarantineReason,
    /// The row is outside the ordered scope of a file source (F17): it is set
    /// aside with the reason instead of quarantined.
    kept: bool,
}

impl Refusal {
    const fn of(analyte: Option<AnalyteId>, reason: QuarantineReason) -> Self {
        Self {
            analyte,
            reason,
            kept: false,
        }
    }

    pub(super) fn plan(self, row: ExtractedRow, position: lis_analytical::PositionId) -> Plan {
        if self.kept {
            return Plan::SetAside {
                row: Box::new(row),
                kind: SetAsideKind::NotOrderedUnusable,
                reason: Some(self.reason),
            };
        }
        quarantine(row, Some(position), self.analyte, self.reason)
    }
}

/// The patient checks, in the brief's order.
pub(super) fn patient_details(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    row: &ExtractedRow,
    target: &PatientTarget,
) -> Result<Result<PatientDetails, Refusal>, ResultError> {
    let Some(analyte_id) = mapped_analyte(planner.registration, row) else {
        return Ok(Err(Refusal::of(None, QuarantineReason::UnknownAnalyte)));
    };
    let method = lis_catalog::get_method_version(ctx.tx(), target.position.method_version_id)?;
    let Some(analyte) = method
        .analytes
        .iter()
        .find(|candidate| candidate.limits.analyte_id == analyte_id)
        .map(|view| view.limits.clone())
    else {
        let refusal = Refusal::of(Some(analyte_id), QuarantineReason::UnknownAnalyte);
        return Ok(Err(refusal));
    };
    let execution_id = target
        .position
        .execution_id
        .ok_or(ResultError::WrongState("patient position"))?;
    let components = crate::queries::scoped_components(ctx.tx(), execution_id, analyte_id)?;
    let kept = components.is_empty();
    let checked = match planner.layout {
        None if kept => Err(QuarantineReason::NotInOrderedScope),
        None if planner.format == ImportFormat::Au => {
            au_value(ctx, planner, row, &method, &analyte, execution_id)?
        }
        None => astm_value(row, &method, &analyte),
        Some(_) => file_rows::file_value(ctx, planner, row, &method, &analyte, execution_id)?,
    };
    let value = match checked {
        Ok(value) => value,
        Err(reason) => {
            return Ok(Err(Refusal {
                analyte: Some(analyte_id),
                reason,
                kept: kept && planner.layout.is_some(),
            }));
        }
    };
    let specimen_id = match target.specimen_id {
        Some(specimen_id) => specimen_id,
        None => {
            lis_specimens::find_container(
                ctx.tx(),
                super::planning::matching_label(planner.format, row).unwrap_or_default(),
            )?
            .container
            .specimen_id
        }
    };
    Ok(Ok(PatientDetails {
        specimen_id,
        analyte_id,
        method,
        analyte,
        components,
        value,
    }))
}

fn au_value(
    ctx: &CommandContext<'_>,
    planner: &Planner<'_>,
    row: &ExtractedRow,
    method: &MethodVersionView,
    analyte: &MethodAnalyte,
    execution_id: lis_analytical::ExecutionId,
) -> Result<Result<ResultValue, QuarantineReason>, ResultError> {
    let factor = lis_analytical::execution_dilution_factor(ctx.tx(), execution_id)?;
    if crate::result_value::diluted(factor.as_ref()) {
        return Ok(Err(QuarantineReason::DilutionNotSupported));
    }
    let profile = planner
        .registration
        .au_profile
        .as_ref()
        .ok_or(ResultError::WrongState("AU source profile"))?;
    Ok(crate::au::patient_value(
        row,
        method.output_scale,
        analyte,
        profile,
    ))
}

pub(super) fn mapped_analyte(
    registration: &lis_analytical::SourceRegistrationView,
    row: &ExtractedRow,
) -> Option<AnalyteId> {
    let code = row.test_code.as_deref()?;
    registration
        .test_codes
        .iter()
        .find(|mapped| mapped.source_test_code == code)
        .map(|mapped| mapped.analyte_id)
}

/// The ASTM route (M2 §3.2 step 5): the unit must equal the method's unless
/// the source says the test was not performed; then the value is read.
fn astm_value(
    row: &ExtractedRow,
    method: &MethodVersionView,
    analyte: &MethodAnalyte,
) -> Result<ResultValue, QuarantineReason> {
    let not_performed = row.status_text.as_deref() == Some(NOT_PERFORMED_STATUS);
    if !not_performed && analyte.unit != row.units_text {
        return Err(QuarantineReason::IncompatibleUnit);
    }
    let value = if not_performed {
        None
    } else {
        let text = row
            .value_text
            .as_deref()
            .ok_or(QuarantineReason::MalformedValue)?;
        let value = crate::parse_value(text, method.output_scale)
            .map_err(|_| QuarantineReason::MalformedValue)?;
        Some(value)
    };
    Ok(ResultValue {
        value,
        units: row.units_text.clone(),
        rule: None,
    })
}
