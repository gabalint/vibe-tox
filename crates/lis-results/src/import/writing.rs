//! Persists planned patient evidence, revisions, quarantine and set-aside rows.

use std::collections::btree_map::Entry;

use lis_core::Comparator;

use super::*;
use crate::commands::{NewRevisionRecord, insert_result_line, insert_revision_record, point_to};
use crate::model::MeasurementForm;

pub(super) fn write_plans(
    ctx: &mut CommandContext<'_>,
    attempt_id: ImportAttemptId,
    plans: Vec<Plan>,
) -> Result<(), ResultError> {
    let correction_runs: BTreeSet<RunId> = plans
        .iter()
        .filter_map(|plan| match plan {
            Plan::Patient(patient) if patient.decision == RowDecision::Correction => {
                Some(patient.position.run_id)
            }
            Plan::Qc(_)
            | Plan::Patient(_)
            | Plan::Quarantine { .. }
            | Plan::NotResult
            | Plan::SetAside { .. } => None,
        })
        .collect();
    let mut processing = BTreeMap::new();
    for plan in plans {
        match plan {
            Plan::Qc(_) | Plan::NotResult => {}
            Plan::Quarantine { row, context } => {
                insert_quarantine(ctx, attempt_id, &row, context)?;
            }
            Plan::SetAside { row, kind, reason } => {
                insert_set_aside(ctx, attempt_id, &row, kind, reason)?;
            }
            Plan::Patient(patient) if patient.decision == RowDecision::Duplicate => {}
            Plan::Patient(patient) if patient.decision == RowDecision::Conflict => {
                insert_quarantine(
                    ctx,
                    attempt_id,
                    &patient.row,
                    QuarantineContext {
                        position: Some(patient.position.position_id),
                        analyte: Some(patient.analyte_id),
                        conflicting_measurement: patient.current_measurement,
                        reason: QuarantineReason::ConflictingDuplicate,
                    },
                )?;
            }
            Plan::Patient(patient) => {
                let processing_id = match processing.entry(patient.position.run_id) {
                    Entry::Occupied(entry) => *entry.get(),
                    Entry::Vacant(entry) => *entry.insert(processing_revision(
                        ctx,
                        patient.position.run_id,
                        attempt_id,
                        correction_runs.contains(&patient.position.run_id),
                    )?),
                };
                write_patient(ctx, attempt_id, processing_id, *patient)?;
            }
        }
    }
    Ok(())
}

pub(super) fn processing_revision(
    ctx: &CommandContext<'_>,
    run_id: RunId,
    attempt_id: ImportAttemptId,
    correction: bool,
) -> Result<ProcessingRevisionId, ResultError> {
    let current: Option<(ProcessingRevisionId, i64)> = ctx
        .tx()
        .query_row(
            "SELECT id, seq FROM res_processing_revisions WHERE run_id = ?1 ORDER BY seq DESC LIMIT 1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if !correction && let Some((id, _)) = current {
        return Ok(id);
    }
    let id = ProcessingRevisionId::generate();
    let (predecessor, seq) = current.map_or((None, 1), |(id, seq)| (Some(id), seq + 1));
    ctx.tx().execute(
        "INSERT INTO res_processing_revisions \
         (id, run_id, seq, import_attempt_id, predecessor_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, run_id, seq, attempt_id, predecessor, ctx.now()],
    )?;
    if correction {
        ctx.audit(
            AuditEvent::new(Stream::Clinical, "result.reprocess", Outcome::Succeeded)
                .record("processing_revision", id)
                .detail("run", AuditDetail::id(run_id))
                .detail("import_attempt", AuditDetail::id(attempt_id)),
        )?;
    }
    Ok(id)
}

/// Writes the measurement (a word as received, F7) and, for each ordered
/// component, its result line; a measurement kept outside the ordered scope
/// gets none (F17).
pub(super) fn write_patient(
    ctx: &mut CommandContext<'_>,
    attempt_id: ImportAttemptId,
    processing_id: ProcessingRevisionId,
    patient: PatientPlan,
) -> Result<(), ResultError> {
    let measurement_id = patient
        .measurement_id
        .ok_or(ResultError::WrongState("measurement plan"))?;
    let (form, comparator, magnitude, code) = measurement_columns(&patient.value);
    ctx.tx().execute(
        "INSERT INTO res_measurements (id, import_attempt_id, processing_revision_id, position_id, \
         analyte_id, source_test_code, value_text, units_text, flags_text, status_text, operator_text, \
         completed_text, record_path, form, comparator, magnitude, code_text, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![measurement_id, attempt_id, processing_id, patient.position.position_id,
            patient.analyte_id, patient.row.test_code, patient.row.value_text,
            patient.row.units_text, patient.row.flags_text, patient.row.status_text,
            patient.row.operator_text, patient.row.completed_text, patient.row.record_path,
            form, comparator, magnitude, code, ctx.now()],
    )?;
    if patient.decision == RowDecision::Correction {
        let previous = patient
            .current_measurement
            .ok_or(ResultError::WrongState("measurement"))?;
        ctx.tx().execute(
            "INSERT INTO res_measurement_supersessions \
             (measurement_id, superseded_by_id, processing_revision_id) VALUES (?1, ?2, ?3)",
            params![previous, measurement_id, processing_id],
        )?;
    }
    for component in &patient.components {
        update_result_line(ctx, &patient, component, measurement_id)?;
    }
    Ok(())
}

/// The measurement's form and value columns: a word keeps only its text.
fn measurement_columns(
    value: &ResultValue,
) -> (
    MeasurementForm,
    Option<Comparator>,
    Option<&str>,
    Option<&str>,
) {
    if value.rule.is_some() {
        return (MeasurementForm::Word, None, None, None);
    }
    let (_, comparator, magnitude, code) = value.columns();
    let form = match &value.value {
        None => MeasurementForm::NotPerformed,
        Some(crate::value::SourceValue::Quantity(_)) => MeasurementForm::Quantity,
        Some(crate::value::SourceValue::Censored { .. }) => MeasurementForm::Censored,
        Some(crate::value::SourceValue::Qualitative(_)) => MeasurementForm::Qualitative,
    };
    (form, comparator, magnitude, code)
}

fn update_result_line(
    ctx: &mut CommandContext<'_>,
    patient: &PatientPlan,
    component: &ScopeComponent,
    measurement_id: MeasurementId,
) -> Result<(), ResultError> {
    let existing: Option<(ResultId, Option<ResultRevisionId>)> = ctx
        .tx()
        .query_row(
            "SELECT id, current_revision_id FROM res_results \
             WHERE order_component_id = ?1 AND analyte_id = ?2",
            params![component.id, patient.analyte_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (result_id, current) = match existing {
        Some(existing) => existing,
        None => (
            insert_result_line(
                ctx.tx(),
                component.id,
                patient.analyte_id,
                component.purpose,
            )?,
            None,
        ),
    };
    let cause = if current.is_none() {
        Some(ResultCause::Initial)
    } else if patient.decision == RowDecision::Correction
        && current_revision_uses(ctx, current, patient.current_measurement)?
    {
        Some(ResultCause::Reprocessed)
    } else {
        None
    };
    let Some(cause) = cause else { return Ok(()) };
    let revision_no: i64 = ctx.tx().query_row(
        "SELECT coalesce(max(revision_no), 0) + 1 FROM res_result_revisions WHERE result_id = ?1",
        [result_id],
        |row| row.get(0),
    )?;
    let revision_id = ResultRevisionId::generate();
    let actor = ctx.principal().actor().id_text();
    insert_revision_record(
        ctx,
        NewRevisionRecord {
            id: revision_id,
            result_id,
            revision_no,
            cause,
            value: &patient.value,
            method: &patient.method,
            analyte: &patient.analyte,
            sequence_role: component.sequence_role,
            specimen_id: patient.specimen_id,
            reason: None,
            created_by: &actor,
        },
        [measurement_id],
    )?;
    point_to(ctx.tx(), result_id, revision_id)
}

fn current_revision_uses(
    ctx: &CommandContext<'_>,
    revision: Option<ResultRevisionId>,
    measurement: Option<MeasurementId>,
) -> Result<bool, ResultError> {
    let (Some(revision), Some(measurement)) = (revision, measurement) else {
        return Ok(false);
    };
    Ok(ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM res_revision_evidence \
         WHERE revision_id = ?1 AND measurement_id = ?2)",
        params![revision, measurement],
        |row| row.get(0),
    )?)
}

fn insert_quarantine(
    ctx: &CommandContext<'_>,
    attempt_id: ImportAttemptId,
    row: &ExtractedRow,
    context: QuarantineContext,
) -> Result<(), ResultError> {
    ctx.tx().execute(
        "INSERT INTO res_quarantine_items (id, import_attempt_id, record_path, sample_label, \
         source_test_code, value_text, units_text, flags_text, status_text, operator_text, \
         completed_text, position_id, analyte_id, conflicting_measurement_id, reason, state, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'open', 1)",
        params![
            QuarantineItemId::generate(),
            attempt_id,
            row.record_path,
            row.sample_label,
            row.test_code,
            row.value_text,
            row.units_text,
            row.flags_text,
            row.status_text,
            row.operator_text,
            row.completed_text,
            context.position,
            context.analyte,
            context.conflicting_measurement,
            context.reason
        ],
    )?;
    Ok(())
}

fn insert_set_aside(
    ctx: &CommandContext<'_>,
    attempt_id: ImportAttemptId,
    row: &ExtractedRow,
    kind: SetAsideKind,
    reason: Option<QuarantineReason>,
) -> Result<(), ResultError> {
    ctx.tx().execute(
        "INSERT INTO res_set_aside_rows (id, import_attempt_id, record_path, sample_label, \
         source_test_code, value_text, type_word, kind, reason) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            crate::model::SetAsideRowId::generate(),
            attempt_id,
            row.record_path,
            row.sample_label,
            row.test_code,
            row.value_text,
            row.sample_type_text,
            kind,
            reason
        ],
    )?;
    Ok(())
}
