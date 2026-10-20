//! Acceptance of a quarantined conflict against its stored import context.

use super::writing::{processing_revision, write_patient};
use super::*;
use crate::result_value::WordRefusal;

/// Applies a human-accepted conflicting row as explicit reprocessing. The
/// quarantine command records the resolution and reason around this change.
pub(crate) fn accept_quarantined_as_reprocessing(
    ctx: &mut CommandContext<'_>,
    item_id: QuarantineItemId,
) -> Result<(), ResultError> {
    let stored = ctx
        .tx()
        .query_row(
            "SELECT q.import_attempt_id, a.receipt_id, a.instrument_id, a.registration_id, \
             q.position_id, q.analyte_id, q.conflicting_measurement_id, q.record_path, \
             q.sample_label, q.source_test_code, q.value_text, q.units_text, q.flags_text, \
             q.status_text, q.operator_text, q.completed_text, a.parser_version \
             FROM res_quarantine_items q JOIN res_import_attempts a ON a.id = q.import_attempt_id \
             WHERE q.id = ?1 AND q.state = 'open' AND q.reason = 'conflicting_duplicate'",
            [item_id],
            |row| {
                Ok((
                    row.get::<_, ImportAttemptId>(0)?,
                    row.get::<_, ReceiptId>(1)?,
                    row.get::<_, InstrumentId>(2)?,
                    row.get::<_, lis_analytical::SourceRegistrationId>(3)?,
                    row.get::<_, lis_analytical::PositionId>(4)?,
                    row.get::<_, AnalyteId>(5)?,
                    row.get::<_, Option<MeasurementId>>(6)?,
                    ExtractedRow {
                        record_path: row.get(7)?,
                        sample_label: row.get(8)?,
                        test_code: row.get(9)?,
                        value_text: row.get(10)?,
                        units_text: row.get(11)?,
                        flags_text: row.get(12)?,
                        status_text: row.get(13)?,
                        operator_text: row.get(14)?,
                        completed_text: row.get(15)?,
                        sample_type_text: None,
                        dilution_text: None,
                    },
                    row.get::<_, String>(16)?,
                ))
            },
        )
        .optional()?
        .ok_or(ResultError::WrongState("quarantine item"))?;
    let conflicting_measurement = stored.6.ok_or(ResultError::RevisionConflict)?;
    lis_analytical::patient_intake_gate(ctx.tx(), stored.4)?.map_err(ResultError::GateBlocked)?;
    let registration = lis_analytical::list_source_registrations(ctx.tx(), stored.2)?
        .into_iter()
        .find(|item| item.registration_id == stored.3)
        .ok_or(ResultError::RevisionConflict)?;
    let position = stored_position(ctx.tx(), stored.4)?;
    let specimen_id = current_measurement_specimen(ctx.tx(), conflicting_measurement)?;
    let source = source_row(&stored.7);
    let format = match stored.8.as_str() {
        AU_PARSER_VERSION => ImportFormat::Au,
        FILE_PARSER_VERSION => ImportFormat::File,
        ASTM_PARSER_VERSION => ImportFormat::Astm,
        _ => return Err(ResultError::RevisionConflict),
    };
    let mut planner = Planner::for_accepted_conflict(stored.1, &registration, format);
    let target = PatientTarget {
        position,
        source_row: source,
        specimen_id,
    };
    let mut patient = match plan_patient(ctx, &mut planner, stored.7, target)? {
        Plan::Patient(patient) => *patient,
        Plan::Quarantine { context, .. } => return Err(refused(Some(context.reason))),
        Plan::SetAside { reason, .. } => return Err(refused(reason)),
        Plan::Qc(_) | Plan::NotResult => return Err(ResultError::RevisionConflict),
    };
    if patient.analyte_id != stored.5
        || patient.current_measurement != Some(conflicting_measurement)
    {
        return Err(ResultError::RevisionConflict);
    }
    patient.decision = RowDecision::Correction;
    patient.measurement_id = Some(MeasurementId::generate());
    let processing = processing_revision(ctx, patient.position.run_id, stored.0, true)?;
    write_patient(ctx, stored.0, processing, patient)
}

/// Why an accepted row no longer becomes a measurement. A word on a dilution
/// (F7) is refused as at import; any other refusal means its context changed.
fn refused(reason: Option<QuarantineReason>) -> ResultError {
    if reason == Some(QuarantineReason::DilutionNotSupported) {
        return WordRefusal::Diluted.error();
    }
    ResultError::RevisionConflict
}

/// The specimen of the current revisions the measurement backs; `None` for a
/// measurement no revision ever cited, kept outside the ordered scope (F17),
/// whose specimen follows from its sample label as at import.
fn current_measurement_specimen(
    conn: &rusqlite::Connection,
    measurement_id: MeasurementId,
) -> Result<Option<SpecimenId>, ResultError> {
    let specimens: Vec<SpecimenId> = conn
        .prepare_cached(
            "SELECT DISTINCT rr.specimen_id FROM res_revision_evidence e \
             JOIN res_result_revisions rr ON rr.id = e.revision_id \
             JOIN res_results r ON r.id = rr.result_id \
             WHERE e.measurement_id = ?1 AND r.current_revision_id = rr.id",
        )?
        .query_map([measurement_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let cited: bool = conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM res_revision_evidence WHERE measurement_id = ?1)",
        )?
        .query_row([measurement_id], |row| row.get(0))?;
    match specimens.as_slice() {
        [specimen_id] => Ok(Some(*specimen_id)),
        [] if !cited => Ok(None),
        [] | [_, _, ..] => Err(ResultError::RevisionConflict),
    }
}

fn stored_position(
    conn: &rusqlite::Connection,
    position_id: lis_analytical::PositionId,
) -> Result<PositionRef, ResultError> {
    conn.query_row(
        "SELECT run_id, role, execution_id, method_version_id FROM ana_v_positions \
         WHERE position_id = ?1 AND run_state = 'open'",
        [position_id],
        |row| {
            Ok(PositionRef {
                position_id,
                run_id: row.get(0)?,
                role: row.get(1)?,
                execution_id: row.get(2)?,
                method_version_id: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or(ResultError::RevisionConflict)
}
