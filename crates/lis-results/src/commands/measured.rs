//! Using a measurement kept outside the ordered scope for an accepted add-on
//! (result files brief F17, Q-41; REQ-RES-001, REQ-ANA-001).

use lis_analytical::ExecutionId;
use lis_catalog::ComponentPurpose;

use super::revision_support::*;
use super::*;
use crate::queries::{get_specimen_results, scoped_components};
use crate::views::SpecimenResults;

/// Pulls a kept measurement into an accepted add-on component.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UseMeasuredResultInput {
    /// The accepted add-on component to fill.
    pub order_component_id: OrderComponentId,
    /// The execution that measured the analyte.
    pub execution_id: ExecutionId,
    /// Why the measurement is used, as entered.
    pub reason: String,
}

/// Fills an accepted add-on component from the current measurements its
/// execution kept outside the ordered scope (F17), in one transaction:
///
/// 1. joins the add-on's order item to the accession whose material was
///    measured, unless it already belongs to it
///    (`lis_specimens::join_accession` refuses another order's item, an item
///    not accepted and an item accessioned elsewhere);
/// 2. makes the execution serve the component
///    (`lis_analytical::attach_served_component` refuses a component its
///    method version does not serve, an inactive one and one already served);
/// 3. creates a result line per analyte with a kept measurement, and its
///    first revision (cause `selected`, with the reason) from it;
/// 4. records the audit event.
///
/// The result then goes through technical review and every release gate,
/// including QC for that run and analyte.
pub fn use_measured_result(
    ctx: &mut CommandContext<'_>,
    input: UseMeasuredResultInput,
) -> Result<SpecimenResults, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    let execution = executed_on(ctx.tx(), input.execution_id)?;
    let component = add_on(ctx.tx(), input.order_component_id)?;
    // Read before the component joins the execution's ordered scope.
    let kept = kept_measurements(ctx, input.execution_id, &execution, &component.analytes)?;
    let accession = lis_specimens::get_accession(ctx.tx(), execution.accession_id)?;
    let joined = !accession.order_item_ids.contains(&component.item_id);
    if joined {
        lis_specimens::join_accession(ctx, execution.accession_id, component.item_id)?;
    }
    lis_analytical::attach_served_component(ctx, input.execution_id, input.order_component_id)?;
    if kept.is_empty() {
        return Err(crate::error::invalid(
            "executionId",
            "nothing_kept",
            "The execution kept no measurement of this test's analytes.",
        ));
    }
    for kept in &kept {
        fill_line(ctx, &input, &component, &actor, kept)?;
    }
    ctx.audit(
        clinical("result.use_measured_result")
            .record("order_component", input.order_component_id)
            .reason(input.reason.clone())
            .detail("execution", AuditDetail::id(input.execution_id))
            .detail("accession", AuditDetail::id(execution.accession_id))
            .detail("accession_joined", AuditDetail::boolean(joined))
            .detail("result_lines", AuditDetail::count(kept.len()))
            .detail(
                "measurements",
                AuditDetail::count(kept.iter().map(|(_, _, ids)| ids.len()).sum()),
            ),
    )?;
    get_specimen_results(ctx.tx(), execution.specimen_id)
}

/// The component's line for the analyte, created when it has none yet,
/// advanced to a selected revision from the kept measurements, with the
/// reason. A line can remain from an execution that failed: it advances
/// under its current revision and revision number, and the change of that
/// line is audited with its previous revision (wave-10 review L8).
fn fill_line(
    ctx: &CommandContext<'_>,
    input: &UseMeasuredResultInput,
    component: &AddOn,
    actor: &str,
    (analyte_id, record, ids): &(AnalyteId, MeasurementRecord, Vec<MeasurementId>),
) -> Result<(), ResultError> {
    let existing: Option<(ResultId, Option<ResultRevisionId>, i64)> = ctx
        .tx()
        .query_row(
            "SELECT id, current_revision_id, revision FROM res_results \
             WHERE order_component_id = ?1 AND analyte_id = ?2",
            params![input.order_component_id, analyte_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let result_id = match existing {
        Some((result_id, ..)) => result_id,
        None => insert_result_line(
            ctx.tx(),
            input.order_component_id,
            *analyte_id,
            component.purpose,
        )?,
    };
    let revision = MeasurementRevision {
        result_id,
        cause: ResultCause::Selected,
        sequence_role: component.sequence_role,
        reason: Some(&input.reason),
        actor,
    };
    let appended = append_measurement_revision(ctx, revision, record, ids.iter().copied())?;
    let Some((_, Some(current), line_revision)) = existing else {
        return point_to(ctx.tx(), result_id, appended.revision_id);
    };
    advance_result(ctx, result_id, current, line_revision, appended.revision_id)?;
    ctx.audit(
        clinical("result.use_measured_result")
            .record("result", result_id)
            .old_revision(line_revision)
            .new_revision(line_revision + 1)
            .reason(input.reason.clone())
            .detail("previous_revision", AuditDetail::id(current)),
    )?;
    Ok(())
}

/// Where an execution's patient position is and what material it tested.
struct Executed {
    position_id: lis_analytical::PositionId,
    specimen_id: SpecimenId,
    accession_id: lis_specimens::AccessionId,
}

fn executed_on(
    conn: &rusqlite::Connection,
    execution_id: ExecutionId,
) -> Result<Executed, ResultError> {
    conn.query_row(
        "SELECT p.position_id, c.specimen_id, c.accession_id FROM ana_v_positions p \
         JOIN spc_v_containers c ON c.container_id = p.container_id \
         WHERE p.execution_id = ?1",
        [execution_id],
        |row| {
            Ok(Executed {
                position_id: row.get(0)?,
                specimen_id: row.get(1)?,
                accession_id: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or(ResultError::NotFound("execution"))
}

/// The add-on component as frozen on its order.
struct AddOn {
    item_id: lis_orders::OrderItemId,
    purpose: ComponentPurpose,
    sequence_role: SequenceRole,
    analytes: Vec<AnalyteId>,
}

fn add_on(
    conn: &rusqlite::Connection,
    component_id: OrderComponentId,
) -> Result<AddOn, ResultError> {
    let (item_id, version_id, position, purpose, sequence_role) = conn
        .query_row(
            "SELECT item_id, catalog_orderable_version_id, catalog_component_position, purpose, \
             sequence_role FROM ord_v_components WHERE component_id = ?1",
            [component_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, lis_catalog::OrderableVersionId>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(ResultError::NotFound("order component"))?;
    let version = lis_catalog::get_orderable_version(conn, version_id)?;
    let catalog = version
        .components
        .iter()
        .find(|candidate| candidate.position == position)
        .ok_or(ResultError::NotFound("frozen catalog component"))?;
    Ok(AddOn {
        item_id,
        purpose,
        sequence_role,
        analytes: catalog
            .analytes
            .iter()
            .map(|item| item.analyte_id)
            .collect(),
    })
}

type Kept = Vec<(AnalyteId, MeasurementRecord, Vec<MeasurementId>)>;

/// Per analyte of the component outside the execution's ordered scope, its
/// current measurements at the execution's position, which must assert the
/// same result.
fn kept_measurements(
    ctx: &CommandContext<'_>,
    execution_id: ExecutionId,
    execution: &Executed,
    analytes: &[AnalyteId],
) -> Result<Kept, ResultError> {
    let mut kept = Vec::new();
    for analyte_id in analytes {
        if !scoped_components(ctx.tx(), execution_id, *analyte_id)?.is_empty() {
            continue;
        }
        let ids: Vec<MeasurementId> = ctx
            .tx()
            .prepare_cached(
                "SELECT m.id FROM res_measurements m LEFT JOIN res_measurement_supersessions s \
                 ON s.measurement_id = m.id \
                 WHERE m.position_id = ?1 AND m.analyte_id = ?2 AND s.measurement_id IS NULL \
                 ORDER BY m.recorded_at, m.id",
            )?
            .query_map(params![execution.position_id, analyte_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let Some(first) = ids.first() else { continue };
        let record = load_measurement(ctx, *first)?;
        for id in ids.iter().skip(1) {
            if !record.equivalent(&load_measurement(ctx, *id)?) {
                return Err(crate::error::invalid(
                    "executionId",
                    "not_equivalent",
                    "The kept measurements of an analyte assert different results.",
                ));
            }
        }
        kept.push((*analyte_id, record, ids));
    }
    Ok(kept)
}
