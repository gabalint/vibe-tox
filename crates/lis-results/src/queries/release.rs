//! Transaction-safe result projections consumed by reporting and billing
//! (REQ-RPT-001, REQ-BIL-005, NFR-01).

use super::*;

use crate::views::{
    CurrentValidity, OutOfScopeDetection, ReleaseLine, ReleaseReviewStatus, ReleaseRevision,
};

/// Every result line belonging to `specimen_id`, with its exact current
/// revision, computed review freshness and unresolved quarantine state.
pub fn specimen_release_lines(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<ReleaseLine>, ResultError> {
    let rows: Vec<(
        ResultId,
        OrderComponentId,
        AnalyteId,
        ComponentPurpose,
        Option<ResultRevisionId>,
    )> = conn
        .prepare_cached(
            "SELECT result.id, result.order_component_id, result.analyte_id, result.purpose, \
                    result.current_revision_id \
             FROM res_results result \
             WHERE EXISTS (SELECT 1 FROM res_result_revisions history \
                           WHERE history.result_id = result.id AND history.specimen_id = ?1) \
             ORDER BY result.id",
        )?
        .query_map([specimen_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    let mut lines = Vec::with_capacity(rows.len());
    for (result_id, order_component_id, analyte_id, purpose, current_id) in rows {
        let current = current_id
            .map(|id| release_revision(conn, specimen_id, id))
            .transpose()?
            .flatten();
        let review = match &current {
            Some(revision) => release_review_status(conn, revision.revision_id)?,
            None => ReleaseReviewStatus::None,
        };
        lines.push(ReleaseLine {
            result_id,
            order_component_id,
            analyte_id,
            purpose,
            current,
            review,
            open_quarantine: open_quarantine(conn, specimen_id, analyte_id)?,
        });
    }
    Ok(lines)
}

fn release_revision(
    conn: &Connection,
    specimen_id: SpecimenId,
    revision_id: ResultRevisionId,
) -> Result<Option<ReleaseRevision>, ResultError> {
    Ok(conn
        .query_row(
            "SELECT id, revision_no, cause, outcome, comparator, magnitude, units_text, code_text, \
                    classification, classification_reason, method_version_id, method_certainty, \
                    output_scale, sequence_role, created_at \
             FROM res_result_revisions WHERE id = ?1 AND specimen_id = ?2",
            params![revision_id, specimen_id],
            |row| {
                Ok(ReleaseRevision {
                    revision_id: row.get(0)?,
                    revision_no: row.get(1)?,
                    cause: row.get(2)?,
                    outcome: row.get(3)?,
                    comparator: row.get(4)?,
                    magnitude: row.get(5)?,
                    units_text: row.get(6)?,
                    code_text: row.get(7)?,
                    classification: row.get(8)?,
                    classification_reason: row.get(9)?,
                    method_version_id: row.get(10)?,
                    certainty: row.get(11)?,
                    output_scale: row.get(12)?,
                    sequence_role: row.get(13)?,
                    created_at: row.get(14)?,
                })
            },
        )
        .optional()?)
}

fn release_review_status(
    conn: &Connection,
    revision_id: ResultRevisionId,
) -> Result<ReleaseReviewStatus, ResultError> {
    match review_status(conn, revision_id)? {
        ReviewStatus::None => Ok(ReleaseReviewStatus::None),
        ReviewStatus::Stale { reasons } => Ok(ReleaseReviewStatus::Stale { reasons }),
        ReviewStatus::Current { review_id } => conn
            .query_row(
                "SELECT reviewed_by, reviewed_at FROM res_technical_reviews WHERE id = ?1",
                [review_id],
                |row| {
                    Ok(ReleaseReviewStatus::Current {
                        review_id,
                        reviewed_by: row.get(0)?,
                        reviewed_at: row.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or(ResultError::NotFound("technical review")),
    }
}

fn open_quarantine(
    conn: &Connection,
    specimen_id: SpecimenId,
    analyte_id: AnalyteId,
) -> Result<bool, ResultError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM res_quarantine_items quarantine \
         JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
         JOIN spc_v_containers container ON container.container_id = position.container_id \
         WHERE container.specimen_id = ?1 AND quarantine.analyte_id = ?2 \
           AND quarantine.state = 'open')",
        params![specimen_id, analyte_id],
        |row| row.get(0),
    )?)
}

/// The latest specimen-validity assessment whose evidence revisions are
/// still current.
pub fn current_validity(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Option<CurrentValidity>, ResultError> {
    let Some(assessment_id) = current_validity_id(conn, specimen_id)? else {
        return Ok(None);
    };
    Ok(conn
        .query_row(
            "SELECT id, policy_version_id, category_code FROM res_validity_assessments \
             WHERE id = ?1",
            [assessment_id],
            |row| {
                Ok(CurrentValidity {
                    assessment_id: row.get(0)?,
                    policy_version_id: row.get(1)?,
                    category_code: row.get(2)?,
                })
            },
        )
        .optional()?)
}

/// Every performed-not-ordered detection for `specimen_id`, including
/// resolved items. Raw values are intentionally excluded from this contract.
pub fn out_of_scope_detections(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<OutOfScopeDetection>, ResultError> {
    Ok(conn
        .prepare_cached(
            "SELECT quarantine.id, quarantine.import_attempt_id, quarantine.position_id, \
                    quarantine.analyte_id, quarantine.source_test_code, quarantine.state, \
                    attempt.processed_at \
             FROM res_quarantine_items quarantine \
             JOIN res_import_attempts attempt ON attempt.id = quarantine.import_attempt_id \
             JOIN ana_v_positions position ON position.position_id = quarantine.position_id \
             JOIN spc_v_containers container ON container.container_id = position.container_id \
             WHERE container.specimen_id = ?1 \
               AND quarantine.reason = 'not_in_ordered_scope' \
             ORDER BY attempt.processed_at, quarantine.id",
        )?
        .query_map([specimen_id], |row| {
            Ok(OutOfScopeDetection {
                item_id: row.get(0)?,
                import_attempt_id: row.get(1)?,
                position_id: row.get(2)?,
                analyte_id: row.get(3)?,
                source_test_code: row.get(4)?,
                state: row.get(5)?,
                detected_at: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}
