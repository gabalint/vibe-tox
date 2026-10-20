//! The context sections of a snapshot, each read through its owner's query.

use lis_identity::SubjectSummary;
use lis_orders::OrderDetail;
use lis_specimens::{AccessionDetail, AccessionSpecimenView, ReceiptDecision, SpecimenId};
use rusqlite::Connection;

use super::{
    IdentifierSection, LaboratorySection, MedicationSection, ObservationSection, OrderSection,
    SpecimenSection, SubjectSection, ValiditySection,
};
use crate::error::{ReportingError, gate};

pub(super) fn specimen_section(
    conn: &Connection,
    accession: &AccessionDetail,
    specimen: &AccessionSpecimenView,
) -> Result<SpecimenSection, ReportingError> {
    let type_name = lis_catalog::list_specimen_types(conn)?
        .into_iter()
        .find(|t| t.code == specimen.specimen_type_code)
        .map(|t| t.name)
        .ok_or(ReportingError::Inconsistent("specimen type"))?;
    let latest = specimen.decisions.last();
    let conditional = latest.filter(|d| d.decision == ReceiptDecision::AcceptedConditionally);
    let collection = &accession.collection;
    Ok(SpecimenSection {
        specimen_id: specimen.specimen_id,
        specimen_type_code: specimen.specimen_type_code.clone(),
        specimen_type_name: type_name,
        accession_id: accession.accession_id,
        accession_number: accession.accession_number.clone(),
        order_item_ids: accession.order_item_ids.clone(),
        acceptance: specimen.acceptance,
        latest_decision_id: latest.map(|d| d.decision_id),
        conditional_policy: conditional.and_then(|d| d.policy_reference.clone()),
        conditional_scope: conditional.and_then(|d| d.scope.clone()),
        discrepancy_codes: latest
            .map(|d| d.discrepancies.iter().map(|x| x.code).collect())
            .unwrap_or_default(),
        condition_report_review: None,
        condition_history: specimen
            .decisions
            .iter()
            .flat_map(|decision| {
                decision
                    .discrepancies
                    .iter()
                    .map(move |discrepancy| super::ConditionSection {
                        decision_id: decision.decision_id,
                        decision: decision.decision,
                        recorded_at: decision.decided_at,
                        code: discrepancy.code,
                        description: discrepancy.note.clone(),
                    })
            })
            .collect(),
        collection_precision: collection.precision,
        collected_at: collection.collected_at,
        utc_offset_minutes: collection.utc_offset_minutes,
        collected_date: collection.collected_date.clone(),
        collector: collection.collector.clone(),
        observations: collection
            .observations
            .iter()
            .map(|o| ObservationSection {
                code: o.code.clone(),
                value: o.value_text.clone(),
                observed_by: o.observed_by.clone(),
            })
            .collect(),
        received_at: specimen.received_at,
        container_labels: specimen
            .containers
            .iter()
            .map(|c| c.label_code.clone())
            .collect(),
    })
}

pub(super) fn subject_section(subject: &SubjectSummary) -> SubjectSection {
    SubjectSection {
        subject_id: subject.subject_id,
        revision: subject.revision,
        family_name: subject.family_name.expose().clone(),
        given_names: subject.given_names.as_ref().map(|g| g.expose().clone()),
        birth_date: subject.birth_date.expose().clone(),
        sex: subject
            .administrative_sex
            .as_ref()
            .map(|s| s.expose().clone()),
        identifiers: subject
            .identifiers
            .iter()
            .map(|i| IdentifierSection {
                identifier_type: i.identifier_type.clone(),
                value: i.value.expose().clone(),
                authority: i.assigning_authority.clone(),
            })
            .collect(),
    }
}

pub(super) fn order_section(
    conn: &Connection,
    order: &OrderDetail,
) -> Result<OrderSection, ReportingError> {
    let practitioner = lis_identity::get_practitioner(conn, order.practitioner_id)?;
    let client = lis_identity::get_client(conn, order.client_id)?;
    let location_name = order
        .client_location_id
        .map(|id| {
            client
                .locations
                .iter()
                .find(|l| l.id == id)
                .map(|l| l.name.clone())
                .ok_or(ReportingError::Inconsistent("client location"))
        })
        .transpose()?;
    Ok(OrderSection {
        order_id: order.order_id,
        version: order.version,
        placer_order_id: order.placer_order_id.clone(),
        practitioner_name: crate::display::person_name(
            &practitioner.family_name,
            practitioner.given_names.as_deref(),
        ),
        practitioner_npi: practitioner.npi.as_str().to_owned(),
        client_name: client.name,
        location_name,
        medications: order
            .evidence
            .medications
            .iter()
            .filter(|m| m.removed_in_version.is_none())
            .map(|m| MedicationSection {
                name: m.received_text.clone(),
                source: m.source,
                as_of: m.as_of_date.clone(),
            })
            .collect(),
    })
}

pub(super) fn validity(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Option<ValiditySection>, ReportingError> {
    let Some(current) = lis_results::current_validity(conn, specimen_id)? else {
        return Ok(None);
    };
    let policy = lis_catalog::validity_policy(conn, current.policy_version_id)?;
    let category = policy
        .categories
        .into_iter()
        .find(|c| c.code == current.category_code)
        .ok_or(ReportingError::Inconsistent("validity category"))?;
    Ok(Some(ValiditySection {
        assessment_id: current.assessment_id,
        policy_version_id: current.policy_version_id,
        category_code: current.category_code,
        label: category.label,
        action: category.action,
        qualifier_text: category.qualifier_text,
    }))
}

pub(super) fn laboratory(conn: &Connection) -> Result<LaboratorySection, ReportingError> {
    let profile = lis_identity::current_laboratory_profile(conn)?.ok_or_else(|| {
        ReportingError::GateBlocked(vec![gate(
            "laboratory_profile_missing",
            "Set the laboratory profile before preparing reports.",
        )])
    })?;
    Ok(LaboratorySection {
        profile_id: profile.profile_id,
        version: profile.version,
        name: profile.name,
        address_lines: profile.address_lines,
        clia_id: profile.clia_id,
        director: profile.director,
        phone: profile.phone,
        time_zone: profile.time_zone,
    })
}
