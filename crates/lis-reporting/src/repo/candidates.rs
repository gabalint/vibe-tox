//! SQL for candidates and their lines. State changes go through the
//! `rpt_candidates_transition` trigger, which refuses anything else.

use lis_catalog::{AnalyteId, OrderableId};
use lis_core::{UserId, UtcTimestamp};
use lis_orders::{OrderComponentId, OrderItemId};
use lis_results::{ResultId, ResultRevisionId, TechnicalReviewId};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    CandidateId, CandidateState, LineDisposition, LineKind, ReportCompleteness, ReportId,
    ReportRelationship, ReportRevisionId,
};

/// A candidate row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CandidateRow {
    pub(crate) id: CandidateId,
    pub(crate) report_id: ReportId,
    pub(crate) proposed_revision_no: i64,
    pub(crate) relationship: ReportRelationship,
    pub(crate) supersedes_revision_id: Option<ReportRevisionId>,
    pub(crate) amendment_reason: Option<String>,
    pub(crate) completeness: ReportCompleteness,
    pub(crate) signatory_user_id: UserId,
    pub(crate) prepared_at: UtcTimestamp,
    pub(crate) content_json: String,
    pub(crate) content_sha256: String,
    pub(crate) data_json: String,
    pub(crate) data_sha256: String,
    pub(crate) template_id: String,
    pub(crate) template_version: i64,
    pub(crate) state: CandidateState,
    pub(crate) render: Option<Rendered>,
    pub(crate) failure_code: Option<String>,
    pub(crate) revision: i64,
}

/// What rendering stored: the artifact and its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rendered {
    pub(crate) artifact_sha256: String,
    pub(crate) artifact_size: i64,
    pub(crate) template_sha256: String,
    pub(crate) renderer_version: String,
    pub(crate) fonts_json: String,
    pub(crate) rendered_at: UtcTimestamp,
}

const COLUMNS: &str = "id, report_id, proposed_revision_no, relationship, \
     supersedes_revision_id, amendment_reason, completeness, signatory_user_id, prepared_at, \
     content_json, content_sha256, data_json, data_sha256, template_id, template_version, state, \
     artifact_sha256, artifact_size, template_sha256, renderer_version, fonts_json, rendered_at, \
     failure_code, revision FROM rpt_candidates";

fn candidate_row(row: &Row<'_>) -> rusqlite::Result<CandidateRow> {
    let artifact_sha256: Option<String> = row.get(16)?;
    let render = match artifact_sha256 {
        Some(artifact_sha256) => Some(Rendered {
            artifact_sha256,
            artifact_size: row.get(17)?,
            template_sha256: row.get(18)?,
            renderer_version: row.get(19)?,
            fonts_json: row.get(20)?,
            rendered_at: row.get(21)?,
        }),
        None => None,
    };
    Ok(CandidateRow {
        id: row.get(0)?,
        report_id: row.get(1)?,
        proposed_revision_no: row.get(2)?,
        relationship: row.get(3)?,
        supersedes_revision_id: row.get(4)?,
        amendment_reason: row.get(5)?,
        completeness: row.get(6)?,
        signatory_user_id: row.get(7)?,
        prepared_at: row.get(8)?,
        content_json: row.get(9)?,
        content_sha256: row.get(10)?,
        data_json: row.get(11)?,
        data_sha256: row.get(12)?,
        template_id: row.get(13)?,
        template_version: row.get(14)?,
        state: row.get(15)?,
        render,
        failure_code: row.get(22)?,
        revision: row.get(23)?,
    })
}

pub(crate) fn candidate(
    conn: &Connection,
    id: CandidateId,
) -> rusqlite::Result<Option<CandidateRow>> {
    conn.prepare_cached(&format!("SELECT {COLUMNS} WHERE id = ?1"))?
        .query_row([id], candidate_row)
        .optional()
}

/// The report's candidate in `rendering` or `ready`, if any.
pub(crate) fn open_candidate(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Option<CandidateRow>> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} WHERE report_id = ?1 AND state IN ('rendering', 'ready')"
    ))?
    .query_row([report_id], candidate_row)
    .optional()
}

pub(crate) fn insert(conn: &Connection, c: &CandidateRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_candidates (id, report_id, proposed_revision_no, relationship, \
         supersedes_revision_id, amendment_reason, completeness, signatory_user_id, prepared_at, \
         content_json, content_sha256, data_json, data_sha256, template_id, template_version, \
         state, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'rendering', 1)",
    )?
    .execute(params![
        c.id,
        c.report_id,
        c.proposed_revision_no,
        c.relationship,
        c.supersedes_revision_id,
        c.amendment_reason,
        c.completeness,
        c.signatory_user_id,
        c.prepared_at,
        c.content_json,
        c.content_sha256,
        c.data_json,
        c.data_sha256,
        c.template_id,
        c.template_version
    ])?;
    Ok(())
}

/// Moves a candidate to `superseded` or `released`; the trigger checks the transition.
pub(crate) fn set_state(
    conn: &Connection,
    id: CandidateId,
    revision: i64,
    state: CandidateState,
) -> rusqlite::Result<bool> {
    Ok(conn
        .prepare_cached(
            "UPDATE rpt_candidates SET state = ?3, revision = revision + 1 \
         WHERE id = ?1 AND revision = ?2",
        )?
        .execute(params![id, revision, state])?
        == 1)
}

pub(crate) fn set_ready(
    conn: &Connection,
    id: CandidateId,
    revision: i64,
    rendered: &Rendered,
) -> rusqlite::Result<bool> {
    Ok(conn
        .prepare_cached(
            "UPDATE rpt_candidates SET state = 'ready', artifact_sha256 = ?3, artifact_size = ?4, \
         template_sha256 = ?5, renderer_version = ?6, fonts_json = ?7, rendered_at = ?8, \
         revision = revision + 1 WHERE id = ?1 AND revision = ?2",
        )?
        .execute(params![
            id,
            revision,
            rendered.artifact_sha256,
            rendered.artifact_size,
            rendered.template_sha256,
            rendered.renderer_version,
            rendered.fonts_json,
            rendered.rendered_at
        ])?
        == 1)
}

pub(crate) fn set_failed(
    conn: &Connection,
    id: CandidateId,
    revision: i64,
    failure_code: &str,
) -> rusqlite::Result<bool> {
    Ok(conn.prepare_cached(
        "UPDATE rpt_candidates SET state = 'failed', failure_code = ?3, revision = revision + 1 \
         WHERE id = ?1 AND revision = ?2",
    )?
    .execute(params![id, revision, failure_code])?
        == 1)
}

/// One stored candidate line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineRow {
    pub(crate) kind: LineKind,
    pub(crate) order_component_id: Option<OrderComponentId>,
    pub(crate) analyte_id: Option<AnalyteId>,
    pub(crate) parent_item_id: Option<OrderItemId>,
    pub(crate) orderable_id: Option<OrderableId>,
    pub(crate) disposition: LineDisposition,
    pub(crate) result: Option<(ResultId, ResultRevisionId)>,
    pub(crate) review: Option<(TechnicalReviewId, UserId)>,
}

/// A candidate's lines, in line order.
pub(crate) fn lines(
    conn: &Connection,
    candidate_id: CandidateId,
) -> rusqlite::Result<Vec<LineRow>> {
    conn.prepare_cached(
        "SELECT kind, order_component_id, analyte_id, parent_item_id, orderable_id, disposition, \
         result_id, result_revision_id, review_id, reviewed_by FROM rpt_candidate_lines \
         WHERE candidate_id = ?1 ORDER BY line_no",
    )?
    .query_map([candidate_id], |row| {
        let result = match (row.get(6)?, row.get(7)?) {
            (Some(result), Some(revision)) => Some((result, revision)),
            _ => None,
        };
        let review = match (row.get(8)?, row.get(9)?) {
            (Some(review), Some(by)) => Some((review, by)),
            _ => None,
        };
        Ok(LineRow {
            kind: row.get(0)?,
            order_component_id: row.get(1)?,
            analyte_id: row.get(2)?,
            parent_item_id: row.get(3)?,
            orderable_id: row.get(4)?,
            disposition: row.get(5)?,
            result,
            review,
        })
    })?
    .collect()
}

pub(crate) fn insert_line(
    conn: &Connection,
    candidate_id: CandidateId,
    line_no: i64,
    line: &LineRow,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_candidate_lines (candidate_id, line_no, kind, order_component_id, \
         analyte_id, parent_item_id, orderable_id, disposition, result_id, result_revision_id, \
         review_id, reviewed_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?
    .execute(params![
        candidate_id,
        line_no,
        line.kind,
        line.order_component_id,
        line.analyte_id,
        line.parent_item_id,
        line.orderable_id,
        line.disposition,
        line.result.map(|r| r.0),
        line.result.map(|r| r.1),
        line.review.map(|r| r.0),
        line.review.map(|r| r.1)
    ])?;
    Ok(())
}
