//! Computed review gates, staleness, and dependency snapshots.

use super::read_context::ReviewRead;
use super::*;

/// Computes every current safety gate for one immutable result revision.
pub fn review_gates(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Vec<GateReason>, ResultError> {
    review_gates_read(&mut ReviewRead::new(conn), revision_id)
}

pub(super) fn review_gates_read(
    read: &mut ReviewRead<'_>,
    revision_id: ResultRevisionId,
) -> Result<Vec<GateReason>, ResultError> {
    let conn = read.conn;
    let facts = read.facts(revision_id)?;
    let mut gates = Vec::new();
    if let Some(range) = super::range::range_gate(read, revision_id)? {
        gates.push(range);
    }
    if facts.current_revision_id != Some(revision_id) {
        gates.push(gate(
            "result_revision_not_current",
            "The selected result revision is no longer current.",
        ));
    }
    gates.extend(read.testing(facts.component_id)?);
    let mut material = read.material(facts.specimen_id)?;
    preserve_review_hold_codes(&mut material);
    gates.extend(material);
    let evidence = read.evidence(revision_id)?;
    for (position_id, analyte_id) in &evidence {
        if let Err(blocked) = read.analytical.status(*position_id, *analyte_id)? {
            gates.extend(blocked);
        }
        let open: bool = conn
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM res_quarantine_items \
             WHERE position_id = ?1 AND analyte_id = ?2 AND state = 'open')",
            )?
            .query_row(params![position_id, analyte_id], |row| row.get(0))?;
        if open {
            gates.push(gate(
                "quarantine_open",
                "An unresolved imported row affects this result.",
            ));
        }
    }
    if facts.purpose == ComponentPurpose::Analytical
        && read.validity_required(facts.component_id, facts.specimen_id)?
        && read.validity(facts.specimen_id)?.is_none()
    {
        gates.push(gate(
            "validity_assessment_missing",
            "Required specimen-validity evidence has not been assessed.",
        ));
    }
    deduplicate_gates(&mut gates);
    Ok(gates)
}

fn preserve_review_hold_codes(gates: &mut [GateReason]) {
    for reason in gates {
        if matches!(
            reason.code.as_str(),
            "container_quality_hold" | "container_excursion_hold"
        ) {
            reason.code = "container_on_hold".to_owned();
        }
    }
}

/// Computed review status. No stale marker is written (REQ-RES-005).
pub fn review_status(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<ReviewStatus, ResultError> {
    review_status_read(&mut ReviewRead::new(conn), revision_id)
}

pub(super) fn review_status_read(
    read: &mut ReviewRead<'_>,
    revision_id: ResultRevisionId,
) -> Result<ReviewStatus, ResultError> {
    let conn = read.conn;
    let Some((review_id, decision, reviewed_revision_id)) =
        super::review_chronology::latest(conn, revision_id)?
    else {
        return super::review_chronology::without_latest(conn, revision_id);
    };
    if decision == crate::ReviewDecision::Rejected {
        return Ok(ReviewStatus::Stale {
            reasons: vec![gate(
                "review_rejected",
                "The latest technical review rejected this result.",
            )],
        });
    }
    let Some(review_seq) = read.audit(review_id)? else {
        return Ok(super::review_chronology::unresolved());
    };
    let mut reasons = Vec::new();
    if let Some(range) = super::range::range_gate(read, reviewed_revision_id)? {
        reasons.push(range);
    }
    let facts = read.facts(reviewed_revision_id)?;
    if facts.current_revision_id != Some(reviewed_revision_id) {
        reasons.push(gate(
            "result_revision_changed",
            "A reviewed result revision is no longer current.",
        ));
    }
    reasons.extend(
        super::invalidations::stale_reasons_after_read(read, reviewed_revision_id, review_seq)?
            .into_iter()
            .map(|reason| reason.gate_reason()),
    );
    reasons.extend(current_analytical_gates(read, reviewed_revision_id)?);
    if read.validity_required(facts.component_id, facts.specimen_id)? {
        match read.validity(facts.specimen_id)? {
            Some(validity)
                if dependency_exists(
                    conn,
                    review_id,
                    "validity_assessment",
                    validity.to_string(),
                )? => {}
            Some(_) => reasons.push(gate(
                "validity_assessment_changed",
                "The specimen-validity assessment changed after review.",
            )),
            None => reasons.push(gate(
                "validity_assessment_stale",
                "The reviewed specimen-validity evidence is no longer current.",
            )),
        }
    }
    deduplicate_gates(&mut reasons);
    if reasons.is_empty() {
        Ok(ReviewStatus::Current { review_id })
    } else {
        Ok(ReviewStatus::Stale { reasons })
    }
}

fn current_analytical_gates(
    read: &mut ReviewRead<'_>,
    revision_id: ResultRevisionId,
) -> Result<Vec<GateReason>, ResultError> {
    let mut reasons = Vec::new();
    for (position_id, analyte_id) in read.evidence(revision_id)? {
        if let Err(blocked) = read.analytical.status(position_id, analyte_id)? {
            reasons.extend(blocked);
        }
    }
    Ok(reasons)
}

fn dependency_exists(
    conn: &Connection,
    review_id: TechnicalReviewId,
    kind: &str,
    id: String,
) -> Result<bool, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM res_review_dependencies \
         WHERE review_id = ?1 AND kind = ?2 AND ref_id = ?3)",
        )?
        .query_row(params![review_id, kind, id], |row| row.get(0))?)
}

#[derive(Debug, Clone)]
pub(super) struct RevisionFacts {
    pub(super) component_id: OrderComponentId,
    pub(super) purpose: ComponentPurpose,
    pub(super) specimen_id: SpecimenId,
    pub(super) current_revision_id: Option<ResultRevisionId>,
}

pub(super) fn revision_facts(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<RevisionFacts, ResultError> {
    conn.prepare_cached(
        "SELECT r.order_component_id, r.purpose, rr.specimen_id, r.current_revision_id \
         FROM res_result_revisions rr JOIN res_results r ON r.id = rr.result_id WHERE rr.id = ?1",
    )?
    .query_row([revision_id], |row| {
        Ok(RevisionFacts {
            component_id: row.get(0)?,
            purpose: row.get(1)?,
            specimen_id: row.get(2)?,
            current_revision_id: row.get(3)?,
        })
    })
    .optional()?
    .ok_or(ResultError::NotFound("result revision"))
}

pub(super) type EvidencePosition = (lis_analytical::PositionId, AnalyteId);

pub(super) fn evidence_positions(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<Vec<EvidencePosition>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT DISTINCT m.position_id, m.analyte_id \
             FROM res_revision_evidence e JOIN res_measurements m ON m.id = e.measurement_id \
             WHERE e.revision_id = ?1",
        )?
        .query_map([revision_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?)
}

pub(super) fn validity_required(
    conn: &Connection,
    component_id: OrderComponentId,
    specimen_id: SpecimenId,
) -> Result<bool, ResultError> {
    let components = specimen_components(conn, specimen_id)?;
    if !components
        .iter()
        .any(|component| component.component_id == component_id)
    {
        return Err(ResultError::WrongState("result component"));
    }
    components_require_validity(conn, &components)
}

pub(super) fn components_require_validity(
    conn: &Connection,
    components: &[lis_orders::ComponentForTesting],
) -> Result<bool, ResultError> {
    for component in components {
        if component.purpose == ComponentPurpose::Validity
            && component_active(conn, component.component_id)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn component_active(
    conn: &Connection,
    component_id: OrderComponentId,
) -> Result<bool, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT component_state = 'active' FROM ord_v_components WHERE component_id = ?1",
        )?
        .query_row([component_id], |row| row.get(0))?)
}

pub(super) fn current_validity_id(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Option<ValidityAssessmentId>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT a.id FROM res_validity_assessments a \
             WHERE a.specimen_id = ?1 AND NOT EXISTS \
               (SELECT 1 FROM res_validity_assessments n WHERE n.supersedes_id = a.id) \
             AND NOT EXISTS (SELECT 1 FROM res_validity_evidence e \
               JOIN res_result_revisions rr ON rr.id = e.result_revision_id \
               JOIN res_results r ON r.id = rr.result_id \
               WHERE e.assessment_id = a.id AND r.current_revision_id IS NOT e.result_revision_id) \
             ORDER BY a.assessed_at DESC, a.id DESC LIMIT 1",
        )?
        .query_row([specimen_id], |row| row.get(0))
        .optional()?)
}

fn deduplicate_gates(gates: &mut Vec<GateReason>) {
    let mut seen = BTreeSet::new();
    gates.retain(|gate| seen.insert(gate.code.clone()));
}

/// Current result and order facts used by commands without duplicating SQL.
pub(crate) fn current_result(
    conn: &Connection,
    result_id: ResultId,
) -> Result<
    (
        ResultRevisionId,
        i64,
        OrderComponentId,
        AnalyteId,
        SpecimenId,
    ),
    ResultError,
> {
    conn.prepare_cached("SELECT r.current_revision_id, r.revision, r.order_component_id, r.analyte_id, rr.specimen_id \
         FROM res_results r JOIN res_result_revisions rr ON rr.id = r.current_revision_id \
         WHERE r.id = ?1")?.query_row(
        [result_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )
    .optional()?
    .ok_or(ResultError::NotFound("result"))
}

/// Checks that a revision is current and returns its parent order facts.
pub(crate) fn reflex_parent(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<(lis_orders::OrderId, lis_orders::OrderItemId, i64), ResultError> {
    let (order_id, item_id) = conn
        .prepare_cached(
            "SELECT c.order_id, c.item_id FROM res_result_revisions rr \
             JOIN res_results r ON r.id = rr.result_id \
             JOIN ord_v_components c ON c.component_id = r.order_component_id \
             WHERE rr.id = ?1 AND r.current_revision_id = rr.id",
        )?
        .query_row([revision_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or(ResultError::RevisionConflict)?;
    let order = lis_orders::get_order(conn, order_id)?;
    Ok((order_id, item_id, order.revision))
}
