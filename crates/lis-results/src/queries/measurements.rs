//! Measurement hydration and component-scope lookups.

use super::*;

/// Runs supporting exactly this immutable result revision; no unrelated result history.
pub fn result_revision_run_ids(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Vec<lis_analytical::RunId>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT p.run_id FROM res_revision_evidence e \
         JOIN res_measurements m ON m.id = e.measurement_id \
         JOIN ana_v_positions p ON p.position_id = m.position_id \
         WHERE e.revision_id = ?1 ORDER BY p.run_id",
        )?
        .query_map([revision_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

pub(crate) fn measurements_for_revision(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Vec<MeasurementView>, ResultError> {
    measurement_query(
        conn,
        "WHERE e.revision_id = ?1 ORDER BY m.recorded_at, m.id",
        [revision_id],
    )
}

pub(crate) fn all_measurements(
    conn: &Connection,
    specimen_id: SpecimenId,
    component_id: OrderComponentId,
    analyte_id: AnalyteId,
) -> Result<Vec<MeasurementView>, ResultError> {
    let mut statement = conn.prepare_cached(
        "SELECT m.id, p.run_id, m.position_id, p.attempt_kind, pr.seq, \
         m.value_text, m.units_text, m.status_text, (s.measurement_id IS NOT NULL), m.recorded_at \
         FROM res_measurements m JOIN ana_v_positions p ON p.position_id = m.position_id \
         JOIN res_processing_revisions pr ON pr.id = m.processing_revision_id \
         LEFT JOIN res_measurement_supersessions s ON s.measurement_id = m.id \
         JOIN spc_v_containers c ON c.container_id = p.container_id \
         WHERE c.specimen_id = ?1 AND m.analyte_id = ?2 ORDER BY m.recorded_at, m.id",
    )?;
    let rows = statement
        .query_map(params![specimen_id, analyte_id], raw_measurement_row)?
        .collect::<Result<_, _>>()?;
    let measurements = materialize_measurements(conn, rows)?;
    let mut in_scope = Vec::new();
    for measurement in measurements {
        if measurement_component_ids(conn, measurement.position_id)?.contains(&component_id) {
            in_scope.push(measurement);
        }
    }
    Ok(in_scope)
}

pub(crate) fn measurement_component_ids(
    conn: &Connection,
    position_id: lis_analytical::PositionId,
) -> Result<Vec<OrderComponentId>, ResultError> {
    let execution_id: lis_analytical::ExecutionId = conn
        .query_row(
            "SELECT execution_id FROM ana_v_positions WHERE position_id = ?1",
            [position_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ResultError::WrongState("patient position"))?;
    Ok(lis_analytical::execution_components(conn, execution_id)?)
}

/// An order component an execution serves whose frozen catalog component
/// expects a given analyte: the ordered scope of a measurement (REQ-ANA-003).
#[derive(Debug, Clone)]
pub(crate) struct ScopeComponent {
    pub(crate) id: OrderComponentId,
    pub(crate) purpose: ComponentPurpose,
    pub(crate) sequence_role: lis_orders::SequenceRole,
}

/// The components of `execution_id` that expect `analyte_id`. Empty means
/// the analyte is outside the ordered scope (M2 §3.2 step 5.2; result files
/// brief F17).
pub(crate) fn scoped_components(
    conn: &Connection,
    execution_id: lis_analytical::ExecutionId,
    analyte_id: AnalyteId,
) -> Result<Vec<ScopeComponent>, ResultError> {
    let ids = lis_analytical::execution_components(conn, execution_id)?;
    let mut components = Vec::new();
    for id in ids {
        let row = conn.query_row(
            "SELECT catalog_orderable_version_id, catalog_component_position, purpose, sequence_role \
             FROM ord_v_components WHERE component_id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, lis_catalog::OrderableVersionId>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, ComponentPurpose>(2)?,
                    row.get::<_, lis_orders::SequenceRole>(3)?,
                ))
            },
        )?;
        let version = lis_catalog::get_orderable_version(conn, row.0)?;
        let in_scope = version
            .components
            .iter()
            .find(|component| component.position == row.1)
            .is_some_and(|component| {
                component
                    .analytes
                    .iter()
                    .any(|analyte| analyte.analyte_id == analyte_id)
            });
        if in_scope {
            components.push(ScopeComponent {
                id,
                purpose: row.2,
                sequence_role: row.3,
            });
        }
    }
    Ok(components)
}

fn measurement_query<P: rusqlite::Params>(
    conn: &Connection,
    suffix: &str,
    params: P,
) -> Result<Vec<MeasurementView>, ResultError> {
    let sql = format!(
        "SELECT m.id, p.run_id, m.position_id, p.attempt_kind, pr.seq, \
         m.value_text, m.units_text, m.status_text, (s.measurement_id IS NOT NULL), m.recorded_at \
         FROM res_revision_evidence e JOIN res_measurements m ON m.id = e.measurement_id \
         JOIN ana_v_positions p ON p.position_id = m.position_id \
         JOIN res_processing_revisions pr ON pr.id = m.processing_revision_id \
         LEFT JOIN res_measurement_supersessions s ON s.measurement_id = m.id {suffix}"
    );
    let rows = conn
        .prepare(&sql)?
        .query_map(params, raw_measurement_row)?
        .collect::<Result<_, _>>()?;
    materialize_measurements(conn, rows)
}

struct RawMeasurement {
    measurement_id: MeasurementId,
    run_id: lis_analytical::RunId,
    position_id: lis_analytical::PositionId,
    attempt_kind: lis_analytical::AttemptKind,
    processing_revision_seq: i64,
    value_text: Option<String>,
    units_text: Option<String>,
    status_text: Option<String>,
    superseded: bool,
    recorded_at: UtcTimestamp,
}

fn raw_measurement_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawMeasurement> {
    Ok(RawMeasurement {
        measurement_id: row.get(0)?,
        run_id: row.get(1)?,
        position_id: row.get(2)?,
        attempt_kind: row.get(3)?,
        processing_revision_seq: row.get(4)?,
        value_text: row.get(5)?,
        units_text: row.get(6)?,
        status_text: row.get(7)?,
        superseded: row.get(8)?,
        recorded_at: row.get(9)?,
    })
}

fn materialize_measurements(
    conn: &Connection,
    rows: Vec<RawMeasurement>,
) -> Result<Vec<MeasurementView>, ResultError> {
    let mut labels: BTreeMap<lis_analytical::RunId, BTreeMap<lis_analytical::PositionId, String>> =
        BTreeMap::new();
    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        if let std::collections::btree_map::Entry::Vacant(slot) = labels.entry(row.run_id) {
            let positions = lis_analytical::get_run(conn, row.run_id, row.recorded_at)?
                .positions
                .into_iter()
                .map(|position| (position.position_id, position.label))
                .collect();
            slot.insert(positions);
        }
        let position_label = labels
            .get(&row.run_id)
            .and_then(|positions| positions.get(&row.position_id))
            .cloned()
            .ok_or(ResultError::NotFound("run position"))?;
        views.push(MeasurementView {
            measurement_id: row.measurement_id,
            run_id: row.run_id,
            position_id: row.position_id,
            position_label,
            attempt_kind: row.attempt_kind,
            processing_revision_seq: row.processing_revision_seq,
            value_text: row.value_text,
            units_text: row.units_text,
            status_text: row.status_text,
            superseded: row.superseded,
        });
    }
    Ok(views)
}
