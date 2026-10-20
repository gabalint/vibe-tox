//! Measurements kept outside the ordered scope, and the accepted order
//! components they can fill (result files brief F17, Q-41).

use lis_analytical::{ExecutionId, ExecutionState, RunId};
use lis_catalog::MethodVersionId;
use lis_orders::ComponentForTesting;

use super::*;
use crate::model::{MeasurementForm, MeasurementId};
use crate::views::{AddOnComponentView, MeasuredNotOrderedView};

/// An accepted, active component of the specimen's order, with no hold and
/// no active execution, whose item is the accession's or in no accession:
/// one a kept measurement may fill (F17).
struct OpenComponent {
    component: ComponentForTesting,
    orderable_code: String,
    /// The method version of its frozen catalog component.
    method_version_id: MethodVersionId,
    /// The analytes its frozen catalog component expects.
    analytes: Vec<AnalyteId>,
}

impl OpenComponent {
    /// Whether a measurement of `analyte_id` made with `method_version_id` can fill it.
    fn takes(&self, method_version_id: MethodVersionId, analyte_id: AnalyteId) -> bool {
        self.method_version_id == method_version_id && self.analytes.contains(&analyte_id)
    }
}

/// The open components of the specimen's order (see [`OpenComponent`]).
/// `use_measured_result` rechecks every gate through the owners.
fn open_components(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<OpenComponent>, ResultError> {
    let (accession, specimen_type) = super::specimen::specimen_accession(conn, specimen_id)?;
    let mut open = Vec::new();
    for component in lis_orders::components_for_order(conn, accession.order_id)? {
        let joinable = accession.order_item_ids.contains(&component.item_id)
            || !lis_specimens::order_item_accessioned(conn, component.item_id)?;
        if component.specimen_type_code != specimen_type
            || !joinable
            || lis_orders::testing_gate(conn, component.component_id)?.is_err()
            || lis_analytical::component_executions(conn, component.component_id)?
                .iter()
                .any(|execution| execution.state == ExecutionState::Active)
        {
            continue;
        }
        let version = lis_catalog::get_orderable_version(conn, component.orderable_version_id)?;
        let catalog = version
            .components
            .iter()
            .find(|candidate| candidate.position == component.catalog_component_position)
            .ok_or(ResultError::NotFound("frozen catalog component"))?;
        open.push(OpenComponent {
            orderable_code: version.code.clone(),
            method_version_id: catalog.method_version_id,
            analytes: catalog
                .analytes
                .iter()
                .map(|item| item.analyte_id)
                .collect(),
            component,
        });
    }
    Ok(open)
}

/// A current measurement no revision cites, as stored.
struct KeptRow {
    measurement_id: MeasurementId,
    execution_id: ExecutionId,
    run_id: RunId,
    instrument_id: InstrumentId,
    analyte_id: AnalyteId,
    value_text: Option<String>,
    units_text: Option<String>,
    form: MeasurementForm,
    recorded_at: UtcTimestamp,
    method_version_id: MethodVersionId,
    execution_active: bool,
}

/// How the run list identifies a run: its instrument's name and its creation
/// time, each read once.
#[derive(Default)]
struct RunLabels {
    instruments: Option<Vec<(InstrumentId, String)>>,
    runs: BTreeMap<RunId, UtcTimestamp>,
}

impl RunLabels {
    fn of(
        &mut self,
        conn: &Connection,
        row: &KeptRow,
    ) -> Result<(String, UtcTimestamp), ResultError> {
        let instruments = match &mut self.instruments {
            Some(instruments) => instruments,
            None => self
                .instruments
                .insert(lis_interfaces::instrument_names(conn)?),
        };
        let name = instruments
            .iter()
            .find(|(id, _)| *id == row.instrument_id)
            .map(|(_, name)| name.clone())
            .ok_or(ResultError::NotFound("instrument"))?;
        let created_at = match self.runs.get(&row.run_id) {
            Some(created_at) => *created_at,
            None => *self
                .runs
                .entry(row.run_id)
                .or_insert(lis_analytical::run_created_at(conn, row.run_id)?),
        };
        Ok((name, created_at))
    }
}

/// The specimen's current measurements that no revision cites and whose
/// analyte is outside their execution's ordered scope, oldest first, each
/// with the open components it can fill while its execution is active.
pub(crate) fn measured_not_ordered(
    conn: &Connection,
    specimen_id: SpecimenId,
    analytes: &BTreeMap<AnalyteId, (String, String)>,
) -> Result<Vec<MeasuredNotOrderedView>, ResultError> {
    let rows: Vec<KeptRow> = conn
        .prepare_cached(
            "SELECT m.id, p.execution_id, p.run_id, m.analyte_id, m.value_text, m.units_text, \
             m.form, m.recorded_at, p.method_version_id, \
             coalesce(p.execution_state = 'active', 0), p.instrument_id \
             FROM res_measurements m JOIN ana_v_positions p ON p.position_id = m.position_id \
             JOIN spc_v_containers c ON c.container_id = p.container_id \
             LEFT JOIN res_measurement_supersessions s ON s.measurement_id = m.id \
             WHERE c.specimen_id = ?1 AND s.measurement_id IS NULL \
               AND NOT EXISTS (SELECT 1 FROM res_revision_evidence e \
                               WHERE e.measurement_id = m.id) \
             ORDER BY m.recorded_at, m.id",
        )?
        .query_map([specimen_id], |row| {
            Ok(KeptRow {
                measurement_id: row.get(0)?,
                execution_id: row.get(1)?,
                run_id: row.get(2)?,
                analyte_id: row.get(3)?,
                value_text: row.get(4)?,
                units_text: row.get(5)?,
                form: row.get(6)?,
                recorded_at: row.get(7)?,
                method_version_id: row.get(8)?,
                execution_active: row.get(9)?,
                instrument_id: row.get(10)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let mut open = None;
    let mut labels = RunLabels::default();
    let mut kept = Vec::new();
    for row in rows {
        if !scoped_components(conn, row.execution_id, row.analyte_id)?.is_empty() {
            continue;
        }
        let (instrument_name, run_created_at) = labels.of(conn, &row)?;
        let (analyte_code, analyte_name) = analytes
            .get(&row.analyte_id)
            .cloned()
            .unwrap_or_else(|| ("unknown".to_owned(), "Unknown analyte".to_owned()));
        let mut add_on_components = Vec::new();
        if row.execution_active {
            let open = match &mut open {
                Some(open) => open,
                None => open.insert(open_components(conn, specimen_id)?),
            };
            add_on_components = open
                .iter()
                .filter(|item| item.takes(row.method_version_id, row.analyte_id))
                .map(|item| AddOnComponentView {
                    component_id: item.component.component_id,
                    orderable_code: item.orderable_code.clone(),
                })
                .collect();
        }
        kept.push(MeasuredNotOrderedView {
            measurement_id: row.measurement_id,
            execution_id: row.execution_id,
            run_id: row.run_id,
            instrument_name,
            run_created_at,
            analyte_id: row.analyte_id,
            analyte_code,
            analyte_name,
            value_text: row.value_text,
            units_text: row.units_text,
            form: row.form,
            recorded_at: row.recorded_at,
            add_on_components,
        });
    }
    Ok(kept)
}
