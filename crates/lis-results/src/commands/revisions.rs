//! Result evidence selection and explicit disposition revisions.

use super::revision_support::*;
use super::*;

pub fn select_result_evidence(
    ctx: &mut CommandContext<'_>,
    input: SelectResultEvidenceInput,
) -> Result<ResultRevisionView, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    let (current, revision, component_id, analyte_id, specimen_id) =
        current_result(ctx.tx(), input.result_id)?;
    if revision != input.expected_revision {
        return Err(ResultError::RevisionConflict);
    }
    let ids: BTreeSet<_> = input.measurement_ids.into_iter().collect();
    if ids.is_empty() {
        return Err(crate::error::invalid(
            "measurementIds",
            "required",
            "Select at least one measurement.",
        ));
    }
    let first_id = ids
        .first()
        .copied()
        .ok_or_else(|| crate::error::invalid("measurementIds", "required", "Select evidence."))?;
    let first = load_measurement(ctx, first_id)?;
    if first.specimen_id != specimen_id
        || first.analyte_id != analyte_id
        || !measurement_serves_component(ctx.tx(), first.position_id, component_id)?
    {
        return Err(ResultError::WrongState("measurement"));
    }
    for id in ids.iter().skip(1) {
        let candidate = load_measurement(ctx, *id)?;
        if !first.equivalent(&candidate)
            || candidate.specimen_id != specimen_id
            || !measurement_serves_component(ctx.tx(), candidate.position_id, component_id)?
        {
            return Err(crate::error::invalid(
                "measurementIds",
                "not_equivalent",
                "Selected measurements must assert the same result.",
            ));
        }
    }
    let sequence_role: SequenceRole = ctx.tx().query_row(
        "SELECT sequence_role FROM ord_v_components WHERE component_id = ?1",
        [component_id],
        |row| row.get(0),
    )?;
    let new = append_measurement_revision(
        ctx,
        MeasurementRevision {
            result_id: input.result_id,
            cause: ResultCause::Selected,
            sequence_role,
            reason: Some(&input.reason),
            actor: &actor,
        },
        &first,
        ids.iter().copied(),
    )?;
    advance_result(ctx, input.result_id, current, revision, new.revision_id)?;
    ctx.audit(
        clinical("result.select_evidence")
            .record("result", input.result_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(input.reason)
            .detail("evidence_count", AuditDetail::count(ids.len())),
    )?;
    Ok(new)
}

/// Appends invalid, indeterminate, missing or not-performed as an explicit disposition.
pub fn record_result_disposition(
    ctx: &mut CommandContext<'_>,
    input: RecordResultDispositionInput,
) -> Result<ResultRevisionView, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    if !matches!(
        input.outcome,
        ResultOutcome::Invalid
            | ResultOutcome::Indeterminate
            | ResultOutcome::Missing
            | ResultOutcome::NotPerformed
    ) {
        return Err(crate::error::invalid(
            "outcome",
            "invalid_disposition",
            "Choose invalid, indeterminate, missing or not performed.",
        ));
    }
    let target = disposition_target(ctx.tx(), &input)?;
    let DispositionTarget::Existing {
        result_id,
        current,
        revision,
        component_id,
        specimen_id,
    } = target
    else {
        return record_pending_missing(ctx, input, actor, target);
    };
    if revision != input.expected_revision {
        return Err(ResultError::RevisionConflict);
    }
    let prior = load_revision_record(ctx.tx(), current)?;
    let sequence_role: SequenceRole = ctx.tx().query_row(
        "SELECT sequence_role FROM ord_v_components WHERE component_id = ?1",
        [component_id],
        |row| row.get(0),
    )?;
    let id = ResultRevisionId::generate();
    let revision_no = prior.revision_no + 1;
    ctx.tx().execute(
        "INSERT INTO res_result_revisions (id, result_id, revision_no, cause, outcome, comparator, \
         magnitude, units_text, code_text, classification, classification_reason, method_version_id, \
         method_certainty, output_scale, sequence_role, specimen_id, reason, created_by, created_at) \
         VALUES (?1, ?2, ?3, 'disposition', ?4, NULL, NULL, NULL, NULL, 'not_applicable', NULL, \
                 ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![id, result_id, revision_no, input.outcome, prior.method_version_id,
            prior.method_certainty, prior.output_scale, sequence_role, specimen_id,
            input.reason, actor, ctx.now()],
    )?;
    copy_revision_evidence(ctx, current, id)?;
    advance_result(ctx, result_id, current, revision, id)?;
    ctx.audit(
        clinical("result.record_disposition")
            .record("result", result_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(input.reason)
            .detail("outcome", AuditDetail::code(input.outcome.as_str())),
    )?;
    revision_view(ctx.tx(), id)
}

#[derive(Debug, Clone, Copy)]
enum DispositionTarget {
    Existing {
        result_id: ResultId,
        current: ResultRevisionId,
        revision: i64,
        component_id: OrderComponentId,
        specimen_id: SpecimenId,
    },
    Pending {
        component_id: OrderComponentId,
        analyte_id: AnalyteId,
        specimen_id: SpecimenId,
    },
}

fn disposition_target(
    conn: &rusqlite::Connection,
    input: &RecordResultDispositionInput,
) -> Result<DispositionTarget, ResultError> {
    match (
        input.result_id,
        input.component_id,
        input.analyte_id,
        input.specimen_id,
    ) {
        (Some(result_id), None, None, None) => {
            let (current, revision, component_id, _, specimen_id) =
                current_result(conn, result_id)?;
            Ok(DispositionTarget::Existing {
                result_id,
                current,
                revision,
                component_id,
                specimen_id,
            })
        }
        (None, Some(component_id), Some(analyte_id), Some(specimen_id))
            if input.expected_revision == 0 && input.outcome == ResultOutcome::Missing =>
        {
            Ok(DispositionTarget::Pending {
                component_id,
                analyte_id,
                specimen_id,
            })
        }
        _ => Err(crate::error::invalid(
            "resultId",
            "invalid_target",
            "Choose an existing result or one pending component and analyte.",
        )),
    }
}

fn record_pending_missing(
    ctx: &mut CommandContext<'_>,
    input: RecordResultDispositionInput,
    actor: String,
    target: DispositionTarget,
) -> Result<ResultRevisionView, ResultError> {
    let DispositionTarget::Pending {
        component_id,
        analyte_id,
        specimen_id,
    } = target
    else {
        return Err(ResultError::WrongState("result disposition"));
    };
    let component = pending_component(ctx.tx(), specimen_id, component_id, analyte_id)?;
    let result_id = insert_result_line(ctx.tx(), component_id, analyte_id, component.purpose)?;
    let method = lis_catalog::get_method_version(ctx.tx(), component.method_version_id)?;
    let id = ResultRevisionId::generate();
    ctx.tx().execute(
        "INSERT INTO res_result_revisions (id, result_id, revision_no, cause, outcome, comparator, \
         magnitude, units_text, code_text, classification, classification_reason, method_version_id, \
         method_certainty, output_scale, sequence_role, specimen_id, reason, created_by, created_at) \
         VALUES (?1, ?2, 1, 'disposition', 'missing', NULL, NULL, NULL, NULL, 'not_applicable', \
                 NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, result_id, method.id, method.certainty, method.output_scale,
            component.sequence_role, specimen_id, input.reason, actor, ctx.now()],
    )?;
    ctx.tx().execute(
        "UPDATE res_results SET current_revision_id = ?2, revision = 2 WHERE id = ?1",
        params![result_id, id],
    )?;
    ctx.audit(
        clinical("result.record_disposition")
            .record("result", result_id)
            .new_revision(2)
            .reason(input.reason)
            .detail(
                "outcome",
                AuditDetail::code(ResultOutcome::Missing.as_str()),
            ),
    )?;
    revision_view(ctx.tx(), id)
}

struct PendingComponent {
    purpose: lis_catalog::ComponentPurpose,
    sequence_role: SequenceRole,
    method_version_id: MethodVersionId,
}

fn pending_component(
    conn: &rusqlite::Connection,
    specimen_id: SpecimenId,
    component_id: OrderComponentId,
    analyte_id: AnalyteId,
) -> Result<PendingComponent, ResultError> {
    let component = crate::queries::specimen_components(conn, specimen_id)?
        .into_iter()
        .find(|candidate| candidate.component_id == component_id)
        .ok_or(ResultError::WrongState("pending component"))?;
    let version = lis_catalog::get_orderable_version(conn, component.orderable_version_id)?;
    let frozen = version
        .components
        .into_iter()
        .find(|candidate| candidate.position == component.catalog_component_position)
        .ok_or(ResultError::NotFound("frozen catalog component"))?;
    if !frozen
        .analytes
        .iter()
        .any(|candidate| candidate.analyte_id == analyte_id)
    {
        return Err(ResultError::WrongState("pending analyte"));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM res_results \
         WHERE order_component_id = ?1 AND analyte_id = ?2)",
        params![component_id, analyte_id],
        |row| row.get(0),
    )?;
    if exists {
        return Err(ResultError::RevisionConflict);
    }
    Ok(PendingComponent {
        purpose: component.purpose,
        sequence_role: component.sequence_role,
        method_version_id: frozen.method_version_id,
    })
}
