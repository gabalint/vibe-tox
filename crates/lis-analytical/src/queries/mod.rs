//! Read-side use cases. They take a connection (a read connection, or the
//! caller's transaction for gate rechecks) and perform no authorization: the
//! engine checks the capability before calling them. No result carries
//! demographics; the engine audits the reads that disclose container
//! sample labels and accession numbers (worklist, run detail).

pub(crate) mod preparation;
pub(crate) mod qc;
pub(crate) mod queues;
pub(crate) mod runs;

pub(crate) use qc::qc_assessment;
pub(crate) use runs::get_run;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use lis_catalog::MethodVersionId;
use lis_core::{InstrumentId, UtcTimestamp};
use lis_orders::OrderComponentId;
use lis_specimens::{AccessionDetail, AccessionId, AccessionSpecimenView};
use rusqlite::Connection;

use crate::error::AnalyticalError;
use crate::model::{
    CivilDate, ExecutionId, LotId, PrepBatchId, RegistrationState, SourceRegistrationId, is_expired,
};
use crate::repo::lots::{self, LotRow};
use crate::repo::published::{self, ContainerRef};
use crate::repo::{readiness, runs as run_repo};
use crate::served::{CatalogCache, ServedComponent, served_components};
use crate::views::{
    InstrumentReadinessView, LotView, PositionMatch, PositionRef, PrepBatchView,
    RegistrationSummary, SourceRegistrationView, WorklistEntry,
};

/// The configured instrument's name, or `NotFound`.
pub(crate) fn instrument_name(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<String, AnalyticalError> {
    lis_interfaces::instrument_names(conn)?
        .into_iter()
        .find(|(id, _)| *id == instrument_id)
        .map(|(_, name)| name)
        .ok_or(AnalyticalError::NotFound("instrument"))
}

/// Every configured instrument with its latest status and active
/// registration (REQ-ANA-007), by name.
pub fn list_instrument_readiness(
    conn: &Connection,
) -> Result<Vec<InstrumentReadinessView>, AnalyticalError> {
    lis_interfaces::instrument_names(conn)?
        .into_iter()
        .map(|(id, name)| instrument_readiness(conn, id, name))
        .collect()
}

pub(crate) fn instrument_readiness(
    conn: &Connection,
    instrument_id: InstrumentId,
    name: String,
) -> Result<InstrumentReadinessView, AnalyticalError> {
    let latest = readiness::latest_status(conn, instrument_id)?;
    let active = active_registration(conn, instrument_id)?;
    Ok(InstrumentReadinessView {
        instrument_id,
        name,
        status: latest.map(|(status, _)| status),
        since: latest.map(|(_, at)| at),
        transport: lis_interfaces::instrument_transport(conn, instrument_id)?,
        active_registration: active.map(|registration| RegistrationSummary {
            registration_id: registration.registration_id,
            sender: registration.sender_name,
            version: registration.sender_version,
            file_template: registration.file_layout.is_some(),
        }),
    })
}

/// The instrument's active source registration with its test-code map,
/// or `None`: an importer quarantines a message without one (REQ-ANA-003).
pub fn active_registration(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<Option<SourceRegistrationView>, AnalyticalError> {
    readiness::registrations(conn, instrument_id, Some(RegistrationState::Active))?
        .into_iter()
        .next()
        .map(|registration| complete_registration(conn, registration))
        .transpose()
}

/// Every registration of an instrument, newest first.
pub fn list_source_registrations(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<Vec<SourceRegistrationView>, AnalyticalError> {
    readiness::registrations(conn, instrument_id, None)?
        .into_iter()
        .map(|registration| complete_registration(conn, registration))
        .collect()
}

/// An exact registration version with its immutable file or AU source profile.
pub fn source_registration(
    conn: &Connection,
    id: SourceRegistrationId,
) -> Result<SourceRegistrationView, AnalyticalError> {
    let registration = readiness::registration(conn, id)?
        .ok_or(AnalyticalError::NotFound("source registration"))?;
    complete_registration(conn, registration)
}

fn complete_registration(
    conn: &Connection,
    mut registration: SourceRegistrationView,
) -> Result<SourceRegistrationView, AnalyticalError> {
    registration.test_codes = readiness::test_codes(conn, registration.registration_id)?;
    registration.file_layout = readiness::file_layout(conn, registration.registration_id)?;
    registration.au_profile = crate::repo::au_profiles::get(conn, registration.registration_id)?;
    if registration.state == RegistrationState::Draft {
        registration.allowed_actions = vec!["approve_source".to_owned()];
    }
    Ok(registration)
}

/// Every lot, or the active ones, with expiry derived at `now`.
pub fn list_lots(
    conn: &Connection,
    active_only: bool,
    now: UtcTimestamp,
) -> Result<Vec<LotView>, AnalyticalError> {
    lots::lots(conn, active_only)?
        .into_iter()
        .map(|row| lot_view_of(row, now))
        .collect()
}

pub(crate) fn lot_view(
    conn: &Connection,
    id: LotId,
    now: UtcTimestamp,
) -> Result<LotView, AnalyticalError> {
    lot_view_of(
        lots::lot(conn, id)?.ok_or(AnalyticalError::NotFound("lot"))?,
        now,
    )
}

fn lot_view_of(row: LotRow, now: UtcTimestamp) -> Result<LotView, AnalyticalError> {
    let expired = is_expired(CivilDate::parse("expiresOn", &row.expires_on)?, now);
    Ok(LotView {
        lot_id: row.id,
        material_kind: row.material_kind,
        product_name: row.product_name,
        lot_number: row.lot_number,
        level_text: row.level_text,
        sample_label: row.sample_label,
        expires_on: row.expires_on,
        expired,
        state: row.state,
        recorded_at: row.recorded_at,
        revision: row.revision,
    })
}

pub fn prep_batch(conn: &Connection, id: PrepBatchId) -> Result<PrepBatchView, AnalyticalError> {
    let mut batch =
        lots::prep_batch(conn, id)?.ok_or(AnalyticalError::NotFound("preparation batch"))?;
    batch.steps = lots::prep_steps(conn, id)?;
    Ok(batch)
}

/// The order components an execution serves (REQ-ANA-001).
pub fn execution_components(
    conn: &Connection,
    execution_id: ExecutionId,
) -> Result<Vec<OrderComponentId>, AnalyticalError> {
    run_repo::execution(conn, execution_id)?.ok_or(AnalyticalError::NotFound("execution"))?;
    Ok(run_repo::execution_components(conn, execution_id)?)
}

/// The performing laboratory profile pinned at run creation (RPT-05).
/// Legacy runs or runs opened without a configured profile return None; callers
/// must identify unknown provenance rather than substitute the current site.
pub fn execution_laboratory_profile(
    conn: &Connection,
    execution_id: ExecutionId,
) -> Result<Option<lis_identity::LaboratoryProfileId>, AnalyticalError> {
    run_repo::execution(conn, execution_id)?.ok_or(AnalyticalError::NotFound("execution"))?;
    let run =
        run_repo::execution_run(conn, execution_id)?.ok_or(AnalyticalError::NotFound("run"))?;
    run_laboratory_profile(conn, run)
}

/// The performing profile pinned when this run was opened; None explicitly
/// means unknown provenance, including legacy runs (RPT-05).
pub fn run_laboratory_profile(
    conn: &Connection,
    run_id: crate::model::RunId,
) -> Result<Option<lis_identity::LaboratoryProfileId>, AnalyticalError> {
    let run = run_repo::run(conn, run_id)?.ok_or(AnalyticalError::NotFound("run"))?;
    Ok(run
        .laboratory_profile_id
        .or(run_repo::site_resolution(conn, run_id)?.map(|r| r.0)))
}

/// The dilution factor recorded on an execution, for the result-file
/// dilution cross-check (result files brief F11); the LIS never multiplies.
pub fn execution_dilution_factor(
    conn: &Connection,
    execution_id: ExecutionId,
) -> Result<Option<lis_core::DecimalText>, AnalyticalError> {
    Ok(run_repo::execution(conn, execution_id)?
        .ok_or(AnalyticalError::NotFound("execution"))?
        .view
        .dilution_factor)
}

/// The position an imported sample label names, searching only open runs of
/// the sending instrument (brief §3.3). Matching is exact; several matches
/// are ambiguous, and row order is never used to choose.
pub fn position_by_sample_label(
    conn: &Connection,
    instrument_id: InstrumentId,
    sample_label: &str,
) -> Result<PositionMatch, AnalyticalError> {
    let mut found = run_repo::open_positions_by_sample_label(conn, instrument_id, sample_label)?;
    Ok(match (found.pop(), found.is_empty()) {
        (None, _) => PositionMatch::None,
        (Some(position), true) => PositionMatch::One(PositionRef {
            position_id: position.id,
            run_id: position.run_id,
            role: position.role,
            execution_id: position.execution_id,
            method_version_id: position.method_version_id,
        }),
        (Some(_), false) => PositionMatch::Ambiguous,
    })
}

/// Containers with tests pending for `method_version_id`: accepted
/// material, rechecked through the specimen and order gates, whose served
/// components no active execution covers yet. No demographics.
pub fn list_worklist(
    conn: &Connection,
    method_version_id: MethodVersionId,
) -> Result<Vec<WorklistEntry>, AnalyticalError> {
    let method = lis_catalog::get_method_version(conn, method_version_id)?;
    let mut cache = CatalogCache::default();
    let mut accessions: BTreeMap<AccessionId, (AccessionDetail, Vec<ServedComponent>)> =
        BTreeMap::new();
    let mut entries = Vec::new();
    for container in
        published::candidate_containers(conn, &method.specimen_type_code, method_version_id)?
    {
        if lis_specimens::testing_gate(conn, container.container_id, &method.specimen_type_code)?
            .is_err()
        {
            continue;
        }
        let (detail, pending) = match accessions.entry(container.accession_id) {
            Entry::Occupied(found) => found.into_mut(),
            Entry::Vacant(slot) => slot.insert(pending(
                conn,
                &mut cache,
                container.accession_id,
                method_version_id,
            )?),
        };
        let received = detail
            .specimens
            .iter()
            .find(|specimen| specimen.specimen_id == container.specimen_id);
        if let (Some(specimen), false) = (received, pending.is_empty()) {
            entries.push(worklist_entry(
                container,
                specimen,
                &method.specimen_type_code,
                pending,
            ));
        }
    }
    Ok(entries)
}

fn worklist_entry(
    container: ContainerRef,
    specimen: &AccessionSpecimenView,
    specimen_type_code: &str,
    pending: &[ServedComponent],
) -> WorklistEntry {
    let mut orderables: Vec<String> = pending.iter().map(|s| s.orderable_code.clone()).collect();
    orderables.sort();
    orderables.dedup();
    WorklistEntry {
        container_id: container.container_id,
        label_code: container.label_code,
        accession_number: container.accession_number,
        specimen_type_code: specimen_type_code.to_owned(),
        component_ids: pending.iter().map(|s| s.component.component_id).collect(),
        orderables,
        received_at: specimen.received_at,
    }
}

/// The served components of an accession that are active, pass the order
/// gate, and have no active execution.
fn pending(
    conn: &Connection,
    cache: &mut CatalogCache,
    accession_id: AccessionId,
    method_version_id: MethodVersionId,
) -> Result<(AccessionDetail, Vec<ServedComponent>), AnalyticalError> {
    let (detail, served) = served_components(conn, cache, accession_id, method_version_id)?;
    let mut pending = Vec::new();
    for served in served.into_iter().filter(|s| s.component.active) {
        let id = served.component.component_id;
        if lis_orders::testing_gate(conn, id)?.is_ok()
            && !run_repo::component_in_progress(conn, id)?
        {
            pending.push(served);
        }
    }
    Ok((detail, pending))
}
