//! Read-side use cases. They take a connection (a read connection, or the
//! caller's transaction for gate rechecks) and perform no authorization: the
//! engine checks the capability, and audits the read where the result is
//! sensitive, before calling them.

use std::collections::BTreeMap;

use lis_core::GateReason;
use rusqlite::Connection;

use crate::error::SpecimenError;
use lis_identity::SubjectId;

use crate::material::{container_actions, release_gates, specimen_actions, testing_gates};
use crate::model::{AccessionId, ContainerId, ExcursionId, SpecimenAcceptance, SpecimenId};
use crate::repo::{accessions, config, containers, holds, labels, receipt_history};
use crate::views::{
    AccessionDetail, AccessionSpecimenView, ContainerForTesting, ContainerHoldRecord,
    ContainerLookup, ContainerView, ExcursionView, LabelPrinterView, StorageLocationView,
    SubjectSpecimen,
};

/// An accession with its collection, specimens, containers, holds and label
/// jobs. Clinical record content: an audited read (capability `accession`).
pub fn get_accession(
    conn: &Connection,
    accession_id: AccessionId,
) -> Result<AccessionDetail, SpecimenError> {
    let row =
        accessions::accession(conn, accession_id)?.ok_or(SpecimenError::NotFound("accession"))?;
    let mut collection = accessions::collection(conn, accession_id)?;
    collection.observations = accessions::observations(conn, collection.collection_id)?;
    let mut specimens = Vec::new();
    for (specimen_id, specimen_type_code, acceptance, received_at, revision) in
        accessions::specimens(conn, collection.collection_id)?
    {
        let mut decisions = receipt_history::decisions(conn, specimen_id)?;
        for decision in &mut decisions {
            decision.discrepancies = receipt_history::discrepancies(conn, decision.decision_id)?;
        }
        specimens.push(AccessionSpecimenView {
            specimen_id,
            specimen_type_code,
            acceptance,
            received_at,
            revision,
            decisions,
            containers: specimen_containers(conn, specimen_id, acceptance)?,
            allowed_actions: specimen_actions(acceptance),
        });
    }
    Ok(AccessionDetail {
        accession_id,
        accession_number: row.number,
        order_id: row.order_id,
        order_item_ids: accessions::accession_items(conn, accession_id)?,
        subject_id: row.subject_id,
        created_at: row.created_at,
        collection,
        specimens,
    })
}

/// The accession a specimen belongs to.
pub fn specimen_accession(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<AccessionId, SpecimenError> {
    accessions::specimen_accession(conn, specimen_id)?.ok_or(SpecimenError::NotFound("specimen"))
}

/// Whether an order item is already linked to an accession, for the
/// add-on items a kept measurement can fill (result files brief F17).
pub fn order_item_accessioned(
    conn: &Connection,
    item_id: lis_orders::OrderItemId,
) -> Result<bool, SpecimenError> {
    Ok(accessions::item_accessioned(conn, item_id)?)
}

/// Primary-container label codes (sample barcodes) for each listed order, in
/// accession order, for the order worklist (REQ-ORD-001). Orders with no
/// received sample are absent from the map.
pub fn order_sample_barcodes(
    conn: &Connection,
    order_ids: &[lis_orders::OrderId],
) -> Result<BTreeMap<lis_orders::OrderId, Vec<String>>, SpecimenError> {
    // Hyphenated UUIDs need no JSON escaping.
    let ids = order_ids
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(",");
    let mut barcodes: BTreeMap<_, Vec<String>> = BTreeMap::new();
    let mut statement = conn.prepare_cached(
        "SELECT a.order_id, c.label_code FROM spc_accessions a \
         JOIN spc_collections col ON col.accession_id = a.id \
         JOIN spc_specimens s ON s.collection_id = col.id \
         JOIN spc_containers c ON c.specimen_id = s.id AND c.kind = 'primary' \
         WHERE a.order_id IN (SELECT value FROM json_each(?1)) \
         ORDER BY a.created_at, a.id, c.label_code",
    )?;
    let rows = statement.query_map([format!("[{ids}]")], |row| Ok((row.get(0)?, row.get(1)?)))?;
    for row in rows {
        let (order_id, label_code) = row?;
        barcodes.entry(order_id).or_default().push(label_code);
    }
    Ok(barcodes)
}

/// Current accession number for an accession or specimen audit record. The
/// label is resolved from the owner table and never copied into audit storage.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, SpecimenError> {
    let accession_id = match record_type {
        "accession" => match record_id.parse::<AccessionId>() {
            Ok(id) => id,
            Err(_) => return Ok(None),
        },
        "specimen" => {
            let Ok(id) = record_id.parse::<SpecimenId>() else {
                return Ok(None);
            };
            match accessions::specimen_accession(conn, id)? {
                Some(accession_id) => accession_id,
                None => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    Ok(accessions::accession(conn, accession_id)?.map(|accession| accession.number))
}

/// The accession a container belongs to.
pub(crate) fn container_accession(
    conn: &Connection,
    container_id: ContainerId,
) -> Result<AccessionId, SpecimenError> {
    let row =
        containers::container(conn, container_id)?.ok_or(SpecimenError::NotFound("container"))?;
    specimen_accession(conn, row.specimen_id)
}

fn specimen_containers(
    conn: &Connection,
    specimen_id: SpecimenId,
    acceptance: SpecimenAcceptance,
) -> Result<Vec<ContainerView>, SpecimenError> {
    let mut views = containers::specimen_containers(conn, specimen_id)?;
    for view in &mut views {
        complete(conn, view, acceptance)?;
    }
    Ok(views)
}

/// Adds open holds, label jobs and allowed actions to a container view.
fn complete(
    conn: &Connection,
    view: &mut ContainerView,
    acceptance: SpecimenAcceptance,
) -> Result<(), SpecimenError> {
    view.open_holds = holds::open_holds(conn, view.container_id)?;
    view.label_jobs = labels::container_jobs(conn, view.container_id)?;
    view.allowed_actions = container_actions(view.state, view.open_holds.len(), acceptance);
    Ok(())
}

/// The container with this printed label code, with its accession and
/// subject (REQ-SPC-001). An audited read (capability `accession`).
pub fn find_container(
    conn: &Connection,
    label_code: &str,
) -> Result<ContainerLookup, SpecimenError> {
    let found = containers::container_by_label(conn, label_code.trim())?
        .ok_or(SpecimenError::NotFound("container"))?;
    let mut container = found.view;
    complete(conn, &mut container, found.acceptance)?;
    let accession = accessions::accession(conn, found.accession_id)?
        .ok_or(SpecimenError::NotFound("accession"))?;
    Ok(ContainerLookup {
        container,
        specimen_type_code: found.specimen_type_code,
        acceptance: found.acceptance,
        accession_id: found.accession_id,
        accession_number: accession.number,
        subject: lis_identity::subject_summary(conn, accession.subject_id)?,
    })
}

/// Storage locations by code, active or not.
pub fn list_locations(conn: &Connection) -> Result<Vec<StorageLocationView>, SpecimenError> {
    Ok(config::locations(conn)?)
}

/// Label printers by code, active or not.
pub fn list_printers(conn: &Connection) -> Result<Vec<LabelPrinterView>, SpecimenError> {
    Ok(config::printers(conn)?)
}

/// An excursion with the holds its assessment placed.
pub fn excursion(
    conn: &Connection,
    excursion_id: ExcursionId,
) -> Result<ExcursionView, SpecimenError> {
    let mut view =
        holds::excursion(conn, excursion_id)?.ok_or(SpecimenError::NotFound("excursion"))?;
    view.holds = holds::excursion_holds(conn, excursion_id)?;
    view.affected_container_labels = conn.prepare_cached("SELECT DISTINCT c.id, c.label_code FROM spc_holds h JOIN spc_containers c ON c.id=h.container_id WHERE h.excursion_id=?1 ORDER BY c.label_code")?.query_map([excursion_id], |row| Ok((row.get(0)?,row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    Ok(view)
}

/// Rechecks every specimen-side gate for analytical testing (REQ-SPC-003,
/// REQ-SPC-005). Callers run it inside their own transaction. The inner
/// error lists every blocking gate: `specimen_not_accepted`,
/// `specimen_held`, `specimen_rejected`, `container_depleted`,
/// `container_disposed`, `container_on_hold` and `matrix_mismatch`.
pub fn testing_gate(
    conn: &Connection,
    container_id: ContainerId,
    required_specimen_type: &str,
) -> Result<Result<ContainerForTesting, Vec<GateReason>>, SpecimenError> {
    let row =
        containers::testing_row(conn, container_id)?.ok_or(SpecimenError::NotFound("container"))?;
    let mut gates = testing_gates(
        row.acceptance,
        row.state,
        row.open_holds > 0,
        row.specimen_type_code == required_specimen_type,
    );
    crate::collection_history::append_review_gate(conn, row.container.specimen_id, &mut gates)?;
    append_chronology_gate(conn, row.container.specimen_id, &mut gates)?;
    crate::standing_authority::append_gate(conn, row.container.specimen_id, &mut gates)?;
    Ok(if gates.is_empty() {
        Ok(row.container)
    } else {
        Err(gates)
    })
}

/// Rechecks every specimen-side gate for releasing a report (REQ-SPC-005,
/// NFR-01). Callers run it inside their own transaction. The inner error
/// lists every blocking gate: `specimen_not_accepted`, `specimen_held`,
/// `specimen_rejected`, `container_quality_hold` and
/// `container_excursion_hold`. Unlike [`testing_gate`], used-up material and
/// retention or legal holds do not block release.
pub fn release_gate(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Result<(), Vec<GateReason>>, SpecimenError> {
    let (acceptance, _) = accessions::specimen_state(conn, specimen_id)?
        .ok_or(SpecimenError::NotFound("specimen"))?;
    let mut gates = release_gates(acceptance, &holds::open_hold_kinds(conn, specimen_id)?);
    crate::collection_history::append_review_gate(conn, specimen_id, &mut gates)?;
    append_chronology_gate(conn, specimen_id, &mut gates)?;
    crate::standing_authority::append_gate(conn, specimen_id, &mut gates)?;
    Ok(if gates.is_empty() { Ok(()) } else { Err(gates) })
}

/// Every hold ever placed on the specimen's containers, open or released,
/// oldest first. Hold reasons are user-typed: callers that show them audit
/// the read.
pub fn container_hold_history(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<ContainerHoldRecord>, SpecimenError> {
    Ok(holds::specimen_hold_history(conn, specimen_id)?)
}

/// The subject's specimens with their collection times, oldest receipt
/// first (REQ-BIL-005 grouping by date of service).
pub fn specimens_for_subject(
    conn: &Connection,
    subject_id: SubjectId,
) -> Result<Vec<SubjectSpecimen>, SpecimenError> {
    Ok(accessions::subject_specimens(conn, subject_id)?)
}

fn append_chronology_gate(
    conn: &Connection,
    specimen_id: SpecimenId,
    gates: &mut Vec<GateReason>,
) -> Result<(), SpecimenError> {
    if receipt_history::chronology_uncertain(conn, specimen_id)? {
        gates.push(GateReason { code: "receipt_chronology_unresolved".to_owned(), message: "Legacy receipt chronology needs an attributable quality review before testing or release.".to_owned() });
    }
    Ok(())
}
