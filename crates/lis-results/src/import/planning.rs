//! Resolves extracted source rows into patient, QC, set-aside or quarantine
//! plans (M2 brief §3.2; result files brief §5, in its order).

use super::file_rows;
use super::patient::{mapped_analyte, patient_details};
use super::*;

type CurrentRowKey = (lis_analytical::PositionId, AnalyteId, String);
type CurrentRows = BTreeMap<CurrentRowKey, (MeasurementId, SourceRow)>;

/// What planning needs about the source, and what it has read so far.
pub(super) struct Planner<'a> {
    pub(super) receipt_id: ReceiptId,
    pub(super) registration: &'a lis_analytical::SourceRegistrationView,
    pub(super) format: ImportFormat,
    /// The approved template of a file source; `None` for ASTM.
    pub(super) layout: Option<&'a FileLayout>,
    /// Whether the dilution cross-check runs (F11). A conflicting row
    /// accepted as reprocessing passed it at import, the execution's factor
    /// never changes, and a quarantine item keeps no dilution text.
    pub(super) check_dilution: bool,
    current_rows: CurrentRows,
    /// Lot sample labels, read once when a file row is not placed (F9).
    pub(super) lot_labels: Option<BTreeSet<String>>,
}

impl<'a> Planner<'a> {
    /// Planning of an import attempt: every check runs.
    pub(super) fn new(
        receipt_id: ReceiptId,
        registration: &'a lis_analytical::SourceRegistrationView,
        format: ImportFormat,
    ) -> Self {
        Self {
            receipt_id,
            registration,
            format,
            layout: registration.file_layout.as_ref(),
            check_dilution: true,
            current_rows: BTreeMap::new(),
            lot_labels: None,
        }
    }

    /// Re-planning of a conflicting row a reviewer accepts as reprocessing:
    /// its dilution factor was checked at import (see `check_dilution`).
    pub(super) fn for_accepted_conflict(
        receipt_id: ReceiptId,
        registration: &'a lis_analytical::SourceRegistrationView,
        format: ImportFormat,
    ) -> Self {
        Self {
            check_dilution: false,
            ..Self::new(receipt_id, registration, format)
        }
    }
}

pub(super) fn plan_rows(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    rows: Vec<ExtractedRow>,
) -> Result<Vec<Plan>, ResultError> {
    rows.into_iter()
        .map(|row| plan_row(ctx, planner, row))
        .collect()
}

/// A file record whose field count differs from the header (brief §5).
pub(super) fn malformed_row(record_path: String) -> Plan {
    let row = ExtractedRow {
        record_path,
        sample_label: None,
        test_code: None,
        value_text: None,
        units_text: None,
        flags_text: None,
        status_text: None,
        operator_text: None,
        completed_text: None,
        sample_type_text: None,
        dilution_text: None,
    };
    quarantine(row, None, None, QuarantineReason::MalformedRow)
}

fn plan_row(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    mut row: ExtractedRow,
) -> Result<Plan, ResultError> {
    if let Some(layout) = planner.layout {
        if file_rows::not_result(layout, &row) {
            return Ok(Plan::NotResult);
        }
        file_rows::apply_declared_unit(layout, &mut row);
    }
    let found = match matching_label(planner.format, &row) {
        Some(label) => lis_analytical::position_by_sample_label(
            ctx.tx(),
            planner.registration.instrument_id,
            label,
        )?,
        None => PositionMatch::None,
    };
    let position = match found {
        PositionMatch::None => return file_rows::unplaced(ctx, planner, row),
        PositionMatch::Ambiguous => {
            return Ok(quarantine(
                row,
                None,
                None,
                QuarantineReason::AmbiguousSample,
            ));
        }
        PositionMatch::One(position) => position,
    };
    if planner.format == ImportFormat::Au && crate::au::role_mismatch(&row, position.role) {
        let analyte = mapped_analyte(planner.registration, &row);
        return Ok(quarantine(
            row,
            Some(position.position_id),
            analyte,
            QuarantineReason::RoleMismatch,
        ));
    }
    if planner
        .layout
        .is_some_and(|layout| file_rows::role_mismatch(layout, &row, position.role))
    {
        let analyte = mapped_analyte(planner.registration, &row);
        return Ok(quarantine(
            row,
            Some(position.position_id),
            analyte,
            QuarantineReason::RoleMismatch,
        ));
    }
    let source_row = source_row(&row);
    if position.role != PositionRole::Patient {
        return Ok(Plan::Qc(Box::new(QcPlan {
            observation: QcRow {
                position_id: position.position_id,
                receipt_id: planner.receipt_id,
                record_path: row.record_path.clone(),
                source_test_code: row.test_code.clone().unwrap_or_default(),
                row: source_row,
            },
        })));
    }
    if let Err(gates) = lis_analytical::patient_intake_gate(ctx.tx(), position.position_id)? {
        return Ok(quarantine(
            row,
            Some(position.position_id),
            None,
            intake_quarantine_reason(&gates)?,
        ));
    }
    let target = PatientTarget {
        position,
        source_row,
        specimen_id: None,
    };
    plan_patient(ctx, planner, row, target)
}

pub(super) fn matching_label(format: ImportFormat, row: &ExtractedRow) -> Option<&str> {
    row.sample_label
        .as_deref()
        .map(|label| match format {
            ImportFormat::Au => label.trim_end_matches(' '),
            ImportFormat::Astm | ImportFormat::File => label,
        })
        .filter(|label| !label.is_empty())
}

fn intake_quarantine_reason(gates: &[GateReason]) -> Result<QuarantineReason, ResultError> {
    if gates.iter().any(|gate| gate.code == "execution_failed") {
        return Ok(QuarantineReason::ExecutionFailed);
    }
    if gates.iter().any(|gate| gate.code == "run_closed") {
        return Ok(QuarantineReason::RunClosed);
    }
    Err(ResultError::WrongState("patient intake gate"))
}

pub(super) fn plan_patient(
    ctx: &CommandContext<'_>,
    planner: &mut Planner<'_>,
    row: ExtractedRow,
    target: PatientTarget,
) -> Result<Plan, ResultError> {
    let details = match patient_details(ctx, planner, &row, &target)? {
        Ok(details) => details,
        Err(refusal) => return Ok(refusal.plan(row, target.position.position_id)),
    };
    let key = (
        target.position.position_id,
        details.analyte_id,
        row.test_code.clone().unwrap_or_default(),
    );
    let current = match planner.current_rows.get(&key) {
        Some(current) => Some(current.clone()),
        None => {
            let current =
                current_measurement(ctx, target.position.position_id, details.analyte_id, &row)?;
            if let Some(current) = &current {
                planner.current_rows.insert(key.clone(), current.clone());
            }
            current
        }
    };
    let decision = lis_analytical::reexport::decide(
        current.as_ref().map(|(_, row)| row),
        &target.source_row,
        planner.registration.revision_indicator,
    );
    let measurement_id = matches!(decision, RowDecision::New | RowDecision::Correction)
        .then(MeasurementId::generate);
    if let Some(measurement_id) = measurement_id {
        planner
            .current_rows
            .insert(key, (measurement_id, target.source_row));
    }
    Ok(Plan::Patient(Box::new(PatientPlan {
        row,
        position: target.position,
        specimen_id: details.specimen_id,
        analyte_id: details.analyte_id,
        method: details.method,
        analyte: details.analyte,
        components: details.components,
        value: details.value,
        decision,
        current_measurement: current.map(|(id, _)| id),
        measurement_id,
    })))
}

pub(super) fn source_row(row: &ExtractedRow) -> SourceRow {
    SourceRow {
        value_text: row.value_text.clone(),
        units_text: row.units_text.clone(),
        flags_text: row.flags_text.clone(),
        status_text: row.status_text.clone(),
    }
}

fn current_measurement(
    ctx: &CommandContext<'_>,
    position_id: lis_analytical::PositionId,
    analyte_id: AnalyteId,
    row: &ExtractedRow,
) -> Result<Option<(MeasurementId, SourceRow)>, ResultError> {
    Ok(ctx
        .tx()
        .query_row(
            "SELECT m.id, m.value_text, m.units_text, m.flags_text, m.status_text \
             FROM res_measurements m LEFT JOIN res_measurement_supersessions s \
               ON s.measurement_id = m.id \
             WHERE m.position_id = ?1 AND m.analyte_id = ?2 AND m.source_test_code = ?3 \
               AND s.measurement_id IS NULL ORDER BY m.recorded_at DESC, m.id DESC LIMIT 1",
            params![
                position_id,
                analyte_id,
                row.test_code.as_deref().unwrap_or_default()
            ],
            |found| {
                Ok((
                    found.get(0)?,
                    SourceRow {
                        value_text: found.get(1)?,
                        units_text: found.get(2)?,
                        flags_text: found.get(3)?,
                        status_text: found.get(4)?,
                    },
                ))
            },
        )
        .optional()?)
}

pub(super) fn quarantine(
    row: ExtractedRow,
    position: Option<lis_analytical::PositionId>,
    analyte: Option<AnalyteId>,
    reason: QuarantineReason,
) -> Plan {
    Plan::Quarantine {
        row: Box::new(row),
        context: QuarantineContext {
            position,
            analyte,
            conflicting_measurement: None,
            reason,
        },
    }
}

pub(super) fn record_qc(
    ctx: &mut CommandContext<'_>,
    plans: &mut [Plan],
    counts: &mut Counts,
) -> Result<(), ResultError> {
    for plan in plans {
        let Plan::Qc(qc) = plan else { continue };
        let decision =
            lis_analytical::record_qc_observations(ctx, std::slice::from_ref(&qc.observation))?
                .into_iter()
                .next()
                .ok_or(ResultError::WrongState("QC observation"))?;
        match decision {
            RowDecision::New
            | RowDecision::Correction
            | RowDecision::Duplicate
            | RowDecision::Conflict => counts.qc += 1,
        }
    }
    Ok(())
}

/// Counts every non-QC row once; `record_qc` counts the QC rows.
pub(super) fn count_plans(plans: &[Plan], counts: &mut Counts) {
    for plan in plans {
        let count = match plan {
            Plan::Patient(patient) => match patient.decision {
                RowDecision::New if patient.kept() => &mut counts.retained,
                RowDecision::New => &mut counts.new,
                RowDecision::Duplicate => &mut counts.duplicate,
                RowDecision::Correction => &mut counts.reprocessed,
                RowDecision::Conflict => &mut counts.quarantined,
            },
            Plan::Quarantine { .. } => &mut counts.quarantined,
            Plan::NotResult => &mut counts.not_result,
            Plan::SetAside { .. } => &mut counts.set_aside,
            Plan::Qc(_) => continue,
        };
        *count += 1;
    }
}
