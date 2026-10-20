//! Transactional technical review and validity-assessment decisions.

use super::*;

pub fn submit_technical_review(
    ctx: &mut CommandContext<'_>,
    input: SubmitTechnicalReviewInput,
) -> Result<ReviewView, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    if input.result_revision_ids.is_empty() {
        return Err(crate::error::invalid(
            "resultRevisionIds",
            "required",
            "Select at least one result revision.",
        ));
    }
    validate_review_reason(input.decision, input.reason.as_deref())?;
    let revisions: BTreeSet<_> = input.result_revision_ids.into_iter().collect();
    let dependencies =
        validate_review_revisions(ctx.tx(), input.specimen_id, input.decision, &revisions)?;
    let review_id = TechnicalReviewId::generate();
    ctx.tx().execute(
        "INSERT INTO res_technical_reviews \
         (id, specimen_id, decision, reason, reviewed_by, reviewed_at, sequence) \
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, coalesce(max(sequence), 0) + 1 \
         FROM res_technical_reviews",
        params![
            review_id,
            input.specimen_id,
            input.decision,
            input.reason,
            actor,
            ctx.now()
        ],
    )?;
    for revision_id in &revisions {
        ctx.tx().execute(
            "INSERT INTO res_review_items (review_id, result_revision_id) VALUES (?1, ?2)",
            params![review_id, revision_id],
        )?;
    }
    for (kind, reference) in dependencies {
        ctx.tx().execute(
            "INSERT INTO res_review_dependencies (review_id, kind, ref_id) VALUES (?1, ?2, ?3)",
            params![review_id, kind, reference],
        )?;
    }
    let mut event = clinical("result.submit_review")
        .record("technical_review", review_id)
        .detail("decision", AuditDetail::code(input.decision.as_str()))
        .detail("item_count", AuditDetail::count(revisions.len()));
    if let Some(reason) = input.reason.clone() {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    Ok(ReviewView {
        review_id,
        specimen_id: input.specimen_id,
        decision: input.decision,
        result_revision_ids: revisions.into_iter().collect(),
        reason: input.reason,
        reviewed_by: actor,
        reviewed_at: ctx.now(),
    })
}

fn validate_review_revisions(
    conn: &rusqlite::Connection,
    specimen_id: SpecimenId,
    decision: ReviewDecision,
    revisions: &BTreeSet<ResultRevisionId>,
) -> Result<BTreeSet<(&'static str, String)>, ResultError> {
    let mut dependencies = BTreeSet::new();
    for revision_id in revisions {
        let specimen: SpecimenId = conn
            .query_row(
                "SELECT specimen_id FROM res_result_revisions WHERE id = ?1",
                [revision_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(ResultError::NotFound("result revision"))?;
        if specimen != specimen_id {
            return Err(ResultError::WrongState("result revision"));
        }
        if decision == ReviewDecision::Approved {
            let gates = review_gates(conn, *revision_id)?;
            if gates
                .iter()
                .any(|g| g.code == "result_revision_not_current")
            {
                return Err(ResultError::RevisionConflict);
            }
            if !gates.is_empty() {
                return Err(ResultError::GateBlocked(gates));
            }
            dependencies.extend(dependency_snapshots(conn, *revision_id)?);
        }
    }
    Ok(dependencies)
}

fn validate_review_reason(
    decision: ReviewDecision,
    reason: Option<&str>,
) -> Result<(), ResultError> {
    match (decision, reason) {
        (_, Some(reason)) => require_reason("reason", reason),
        (ReviewDecision::Rejected, None) => Err(crate::error::invalid(
            "reason",
            "required",
            "Enter a reason for rejecting the review.",
        )),
        (ReviewDecision::Approved, None) => Ok(()),
    }
}

/// Records and versions a policy-based validity assessment.
pub fn record_validity_assessment(
    ctx: &mut CommandContext<'_>,
    input: RecordValidityAssessmentInput,
) -> Result<ValidityAssessmentView, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    validate_validity_policy(ctx.tx(), &input)?;
    if let Some(reason) = input.reason.as_deref() {
        require_reason("reason", reason)?;
    } else if input.supersedes_id.is_some() {
        return Err(crate::error::invalid(
            "reason",
            "required",
            "Enter why the validity assessment changes.",
        ));
    }
    let current: Option<ValidityAssessmentId> = ctx
        .tx()
        .query_row(
            "SELECT a.id FROM res_validity_assessments a WHERE a.specimen_id = ?1 AND NOT EXISTS \
             (SELECT 1 FROM res_validity_assessments n WHERE n.supersedes_id = a.id) LIMIT 1",
            [input.specimen_id],
            |row| row.get(0),
        )
        .optional()?;
    if current != input.supersedes_id {
        return Err(ResultError::RevisionConflict);
    }
    let evidence =
        validate_validity_evidence(ctx.tx(), input.specimen_id, input.evidence_revision_ids)?;
    let id = ValidityAssessmentId::generate();
    ctx.tx().execute(
        "INSERT INTO res_validity_assessments (id, specimen_id, policy_version_id, category_code, \
         supersedes_id, reason, assessed_by, assessed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            input.specimen_id,
            input.policy_version_id,
            input.category_code,
            input.supersedes_id,
            input.reason,
            actor,
            ctx.now()
        ],
    )?;
    for revision in &evidence {
        ctx.tx().execute(
            "INSERT INTO res_validity_evidence (assessment_id, result_revision_id) VALUES (?1, ?2)",
            params![id, revision],
        )?;
    }
    let mut event = clinical("result.assess_validity")
        .record("validity_assessment", id)
        .detail("specimen", AuditDetail::id(input.specimen_id))
        .detail("policy_version", AuditDetail::id(input.policy_version_id));
    if let Some(reason) = input.reason.clone() {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    Ok(ValidityAssessmentView {
        assessment_id: id,
        policy_version_id: input.policy_version_id,
        category_code: input.category_code,
        supersedes_id: input.supersedes_id,
        reason: input.reason,
        assessed_by: actor,
        assessed_at: ctx.now(),
        evidence_revision_ids: evidence.into_iter().collect(),
    })
}

fn validate_validity_policy(
    conn: &rusqlite::Connection,
    input: &RecordValidityAssessmentInput,
) -> Result<(), ResultError> {
    let specimen_type: String = conn
        .query_row(
            "SELECT specimen_type_code FROM spc_v_containers WHERE specimen_id = ?1 LIMIT 1",
            [input.specimen_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ResultError::NotFound("specimen"))?;
    let policy = lis_catalog::approved_validity_policy(conn, &specimen_type)?
        .ok_or(ResultError::WrongState("validity policy"))?;
    if policy.id == input.policy_version_id
        && policy
            .categories
            .iter()
            .any(|category| category.code == input.category_code)
    {
        Ok(())
    } else {
        Err(ResultError::WrongState("validity policy"))
    }
}

fn validate_validity_evidence(
    conn: &rusqlite::Connection,
    specimen_id: SpecimenId,
    revision_ids: Vec<ResultRevisionId>,
) -> Result<BTreeSet<ResultRevisionId>, ResultError> {
    if revision_ids.is_empty() {
        return Err(crate::error::invalid(
            "evidenceRevisionIds",
            "required",
            "Select validity result evidence.",
        ));
    }
    let evidence: BTreeSet<_> = revision_ids.into_iter().collect();
    for revision in &evidence {
        let (specimen, purpose): (SpecimenId, lis_catalog::ComponentPurpose) = conn
            .query_row(
                "SELECT rr.specimen_id, r.purpose FROM res_result_revisions rr \
                 JOIN res_results r ON r.id = rr.result_id \
                 WHERE rr.id = ?1 AND r.current_revision_id = rr.id",
                [revision],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(ResultError::RevisionConflict)?;
        if specimen != specimen_id || purpose != lis_catalog::ComponentPurpose::Validity {
            return Err(ResultError::WrongState("validity evidence"));
        }
    }
    Ok(evidence)
}
