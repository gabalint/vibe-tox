//! Specimen result detail, evidence materialization, and review queue.

use super::*;

/// Full audited result detail for one specimen.
pub fn get_specimen_results(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<SpecimenResults, ResultError> {
    let specimen = conn
        .query_row(
            "SELECT container_id, accession_number, specimen_type_code, acceptance \
             FROM spc_v_containers WHERE specimen_id = ?1 ORDER BY container_id LIMIT 1",
            [specimen_id],
            |row| {
                Ok((
                    row.get::<_, ContainerId>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, lis_specimens::SpecimenAcceptance>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(ResultError::NotFound("specimen"))?;
    let lookup = lis_specimens::find_container(
        conn,
        &conn.query_row(
            "SELECT label_code FROM spc_v_containers WHERE container_id = ?1",
            [specimen.0],
            |row| row.get::<_, String>(0),
        )?,
    )?;
    let analytes: BTreeMap<_, _> = lis_catalog::list_analytes(conn)?
        .into_iter()
        .map(|a| (a.id, (a.code, a.name)))
        .collect();
    let mut lines = result_lines(conn, specimen_id, &analytes)?;
    for line in &mut lines {
        if let Some(current) = &line.current {
            line.gates = review_gates(conn, current.revision_id)?;
            line.review_status = review_status(conn, current.revision_id)?;
        }
    }
    let validity_history = validity_history(conn, specimen_id)?;
    let current_id = current_validity_id(conn, specimen_id)?;
    let current_validity = current_id.and_then(|id| {
        validity_history
            .iter()
            .find(|assessment| assessment.assessment_id == id)
            .cloned()
    });
    let components = specimen_components(conn, specimen_id)?;
    let pending_analytes = pending_analytes(conn, specimen_id, &components)?;
    let validity_required = components_require_validity(conn, &components)?;
    let measured_not_ordered = measured_not_ordered(conn, specimen_id, &analytes)?;
    let mut allowed_actions = vec!["submit_technical_review".to_owned()];
    if measured_not_ordered
        .iter()
        .any(|measurement| !measurement.add_on_components.is_empty())
    {
        allowed_actions.push("use_measured_result".to_owned());
    }
    let reflex_history = reflex_history(conn, lookup.accession_id)?;
    Ok(SpecimenResults {
        subject: lookup.subject,
        accession_number: specimen.1,
        specimen_id,
        specimen_type_code: specimen.2,
        acceptance: specimen.3,
        lines,
        validity_required,
        current_validity,
        validity_history,
        pending_analytes,
        measured_not_ordered,
        allowed_actions,
        reflex_history,
    })
}

fn reflex_history(
    conn: &Connection,
    accession_id: lis_specimens::AccessionId,
) -> Result<Vec<crate::views::ReflexHistoryEntry>, ResultError> {
    let accession = lis_specimens::get_accession(conn, accession_id)?;
    let order = lis_orders::get_order(conn, accession.order_id)?;
    let orderables = lis_catalog::list_orderables(conn)?;
    order
        .trigger_assessments
        .iter()
        .filter(|a| accession.order_item_ids.contains(&a.parent_item_id))
        .map(|assessment| {
            let orderable = orderables
                .iter()
                .find(|o| o.id == assessment.orderable_id)
                .ok_or(ResultError::NotFound("reflex orderable"))?;
            Ok(crate::views::ReflexHistoryEntry {
                assessment: assessment.clone(),
                orderable_code: orderable.code.clone(),
                orderable_name: orderable.name.clone(),
                created_item_state: assessment.created_item_id.and_then(|id| {
                    order
                        .items
                        .iter()
                        .find(|i| i.item_id == id)
                        .map(|i| i.state)
                }),
            })
        })
        .collect()
}

fn result_lines(
    conn: &Connection,
    specimen_id: SpecimenId,
    analytes: &BTreeMap<AnalyteId, (String, String)>,
) -> Result<Vec<ResultLineView>, ResultError> {
    let rows: Vec<(ResultId, OrderComponentId, AnalyteId, ComponentPurpose, Option<ResultRevisionId>, i64)> = conn
        .prepare_cached(
            "SELECT r.id, r.order_component_id, r.analyte_id, r.purpose, r.current_revision_id, r.revision \
             FROM res_results r JOIN res_result_revisions rr ON rr.id = r.current_revision_id \
             WHERE rr.specimen_id = ?1 ORDER BY r.id",
        )?
        .query_map([specimen_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut result = Vec::new();
    for (id, component, analyte_id, purpose, current_id, revision) in rows {
        let history = revisions(conn, id)?;
        let current = current_id.and_then(|current| {
            history
                .iter()
                .find(|revision| revision.revision_id == current)
                .cloned()
        });
        let selected: BTreeSet<MeasurementId> = history
            .iter()
            .flat_map(|revision| revision.evidence.iter().map(|m| m.measurement_id))
            .collect();
        let unselected_evidence = all_measurements(conn, specimen_id, component, analyte_id)?
            .into_iter()
            .filter(|measurement| !selected.contains(&measurement.measurement_id))
            .collect();
        let (code, name) = analytes
            .get(&analyte_id)
            .cloned()
            .unwrap_or_else(|| ("unknown".to_owned(), "Unknown analyte".to_owned()));
        result.push(ResultLineView {
            result_id: id,
            revision,
            component_id: component,
            analyte_id,
            analyte_code: code,
            analyte_name: name,
            purpose,
            current,
            history,
            unselected_evidence,
            gates: Vec::new(),
            review_status: ReviewStatus::None,
        });
    }
    Ok(result)
}

fn revisions(
    conn: &Connection,
    result_id: ResultId,
) -> Result<Vec<ResultRevisionView>, ResultError> {
    let mut rows: Vec<ResultRevisionView> = conn
        .prepare_cached(
            "SELECT id, revision_no, cause, outcome, comparator, magnitude, units_text, code_text, \
             classification, classification_reason, method_version_id, method_certainty, output_scale, \
             sequence_role, reason, created_by, created_at FROM res_result_revisions \
             WHERE result_id = ?1 ORDER BY revision_no DESC",
        )?
        .query_map([result_id], crate::views::result_revision_row)?
        .collect::<Result<_, _>>()?;
    for revision in &mut rows {
        revision.evidence = measurements_for_revision(conn, revision.revision_id)?;
    }
    Ok(rows)
}

fn validity_history(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<ValidityAssessmentView>, ResultError> {
    let mut assessments: Vec<ValidityAssessmentView> = conn
        .prepare_cached(
            "SELECT id, policy_version_id, category_code, supersedes_id, reason, assessed_by, assessed_at \
             FROM res_validity_assessments WHERE specimen_id = ?1 ORDER BY assessed_at DESC, id DESC",
        )?
        .query_map([specimen_id], |row| {
            Ok(ValidityAssessmentView {
                assessment_id: row.get(0)?,
                policy_version_id: row.get(1)?,
                category_code: row.get(2)?,
                supersedes_id: row.get(3)?,
                reason: row.get(4)?,
                assessed_by: row.get(5)?,
                assessed_at: row.get(6)?,
                evidence_revision_ids: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;
    for assessment in &mut assessments {
        assessment.evidence_revision_ids = conn
            .prepare_cached(
                "SELECT result_revision_id FROM res_validity_evidence \
                 WHERE assessment_id = ?1 ORDER BY result_revision_id",
            )?
            .query_map([assessment.assessment_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(assessments)
}

fn pending_analytes(
    conn: &Connection,
    specimen_id: SpecimenId,
    components: &[lis_orders::ComponentForTesting],
) -> Result<Vec<PendingAnalyteView>, ResultError> {
    let mut pending = Vec::new();
    for component in components {
        if !component_active(conn, component.component_id)? {
            continue;
        }
        let manifest = lis_catalog::get_orderable_version(conn, component.orderable_version_id)?;
        let catalog_component = manifest
            .components
            .iter()
            .find(|candidate| candidate.position == component.catalog_component_position)
            .ok_or(ResultError::NotFound("frozen catalog component"))?;
        for analyte in &catalog_component.analytes {
            let present: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM res_results r \
                 JOIN res_result_revisions rr ON rr.id = r.current_revision_id \
                 WHERE r.order_component_id = ?1 AND r.analyte_id = ?2 \
                   AND rr.specimen_id = ?3)",
                params![component.component_id, analyte.analyte_id, specimen_id],
                |row| row.get(0),
            )?;
            if !present {
                pending.push(PendingAnalyteView {
                    component_id: component.component_id,
                    analyte_id: analyte.analyte_id,
                    analyte_code: analyte.code.clone(),
                    analyte_name: analyte.name.clone(),
                });
            }
        }
    }
    pending.sort_by(|left, right| {
        left.analyte_code
            .cmp(&right.analyte_code)
            .then_with(|| left.component_id.cmp(&right.component_id))
    });
    Ok(pending)
}

/// The specimen's accession and specimen type code.
pub(super) fn specimen_accession(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<(lis_specimens::AccessionDetail, String), ResultError> {
    let (accession_id, specimen_type): (lis_specimens::AccessionId, String) = conn
        .query_row(
            "SELECT accession_id, specimen_type_code FROM spc_v_containers \
             WHERE specimen_id = ?1 ORDER BY container_id LIMIT 1",
            [specimen_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(ResultError::NotFound("specimen"))?;
    Ok((
        lis_specimens::get_accession(conn, accession_id)?,
        specimen_type,
    ))
}

pub(crate) fn specimen_components(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<lis_orders::ComponentForTesting>, ResultError> {
    let (accession, specimen_type) = specimen_accession(conn, specimen_id)?;
    let item_ids: BTreeSet<_> = accession.order_item_ids.into_iter().collect();
    Ok(lis_orders::components_for_order(conn, accession.order_id)?
        .into_iter()
        .filter(|component| {
            item_ids.contains(&component.item_id) && component.specimen_type_code == specimen_type
        })
        .collect())
}

pub(super) struct ReviewQueueMetadata {
    pub(super) subject: lis_identity::SubjectSummary,
    pub(super) accession_number: String,
    pub(super) specimen_type_code: String,
    pub(super) lines: i64,
    pub(super) pending: i64,
}

pub(super) fn review_queue_labels(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<(String, String, String), ResultError> {
    conn.query_row(
        "SELECT accession_number, specimen_type_code, label_code FROM spc_v_containers \
             WHERE specimen_id = ?1 ORDER BY container_id LIMIT 1",
        [specimen_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?
    .ok_or(ResultError::NotFound("specimen"))
}

pub(super) fn review_queue_metadata(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<ReviewQueueMetadata, ResultError> {
    let (accession_number, specimen_type_code, label) = review_queue_labels(conn, specimen_id)?;
    let lines = conn.query_row(
        "SELECT count(*) FROM res_results result \
         JOIN res_result_revisions current ON current.id = result.current_revision_id \
         WHERE current.specimen_id = ?1",
        [specimen_id],
        |row| row.get(0),
    )?;
    let components = specimen_components(conn, specimen_id)?;
    let pending = pending_analytes(conn, specimen_id, &components)?.len() as i64;
    Ok(ReviewQueueMetadata {
        subject: lis_specimens::find_container(conn, &label)?.subject,
        accession_number,
        specimen_type_code,
        lines,
        pending,
    })
}

/// Bounded review queue, derived from current result lines and open quarantine.
pub fn review_queue(
    conn: &Connection,
    filter: ReviewQueueFilter,
    limit: u32,
) -> Result<Vec<ReviewQueueEntry>, ResultError> {
    super::queues::review_queue_entries(conn, filter, limit)
}
