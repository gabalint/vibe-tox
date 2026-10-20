//! Reports, revisions and their history (REQ-RPT-001, REQ-RPT-002, NFR-07).
//! Report detail and revision content are audited reads; the list holds
//! identifiers and states only.

use crate::queries::obligation::obligation_view;
use lis_core::{Capability, Principal, UtcTimestamp};
use rusqlite::{Connection, params};
use serde::Deserialize;
use ts_rs::TS;

use crate::delivery::labels::export_label;
use crate::error::ReportingError;
use crate::impact;
use crate::model::{ReportId, ReportRevisionId, RevisionAvailability};
use crate::queries::candidate::user_view;
use crate::queries::delivery::DeliveryContext;
use crate::queries::incidents::report_incidents;
use crate::repo::candidates;
use crate::repo::delivery;
use crate::repo::exports;
use crate::repo::reports::{self, RevisionRow};
use crate::stored::stored_content;
use crate::views::{
    ExportView, ImpactAssessmentView, ImpactReasonView, ImpactView, ObligationView, ReportDetail,
    ReportSummary, RevisionDetail, RevisionProvenance, RevisionSummary, WithdrawalView,
};

/// Current report number for an audit record, including the revision number
/// when the record is a released revision (REQ-OPS-001).
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, ReportingError> {
    match record_type {
        "report" => {
            let Ok(id) = record_id.parse::<ReportId>() else {
                return Ok(None);
            };
            Ok(reports::report(conn, id)?.map(|report| report.report_number))
        }
        "report_revision" => {
            let Ok(id) = record_id.parse::<ReportRevisionId>() else {
                return Ok(None);
            };
            let Some(revision) = reports::revision(conn, id)? else {
                return Ok(None);
            };
            Ok(reports::report(conn, revision.report_id)?.map(|report| {
                format!("{} revision {}", report.report_number, revision.revision_no)
            }))
        }
        _ => Ok(None),
    }
}

/// A revision as listed.
pub(crate) fn revision_summary(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<RevisionSummary, ReportingError> {
    Ok(RevisionSummary {
        revision_id: revision.id,
        revision_no: revision.revision_no,
        relationship: revision.relationship,
        completeness: revision.completeness,
        availability: revision.availability,
        released_at: revision.released_at,
        released_by: user_view(conn, revision.released_by)?,
        amendment_reason: revision.amendment_reason.clone(),
        artifact_sha256: revision.artifact_sha256.clone(),
    })
}

/// A report with its revisions, obligations, exports, impact, withdrawal
/// and preservation incidents, with what `caller` may do now. PHI: an
/// audited read.
pub fn get_report(
    conn: &Connection,
    report_id: ReportId,
    caller: &Principal,
) -> Result<ReportDetail, ReportingError> {
    let report = reports::report(conn, report_id)?.ok_or(ReportingError::NotFound("report"))?;
    let accession = lis_specimens::specimen_accession(conn, report.specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession)?;
    let revisions = reports::revisions(conn, report_id)?;
    let latest = revisions.last();
    let withdrawal = reports::withdrawal(conn, report_id)?;
    let delivers = caller.require(Capability::DeliverReports).is_ok();
    let mut obligations = obligation_views(conn, report_id)?;
    if !delivers {
        obligations
            .iter_mut()
            .for_each(|o| o.allowed_actions.clear());
    }
    let impact = match latest.filter(|r| r.availability == RevisionAvailability::Current) {
        Some(revision) => current_impact(conn, revision)?,
        None => None,
    };
    let allowed_actions = report_actions(
        caller,
        latest.map(|r| r.availability),
        impact.as_ref(),
        withdrawal.is_some(),
    );
    Ok(ReportDetail {
        report_id,
        report_number: report.report_number,
        specimen_id: report.specimen_id,
        client_id: lis_orders::get_order(conn, accession.order_id)?.client_id,
        subject: lis_identity::subject_summary(conn, accession.subject_id)?,
        accession_number: accession.accession_number,
        availability: latest.map(|r| r.availability),
        impact,
        withdrawal: withdrawal
            .map(|w| -> Result<_, ReportingError> {
                let revision_no = revisions
                    .iter()
                    .find(|r| r.id == w.revision_id)
                    .map(|r| r.revision_no)
                    .ok_or(ReportingError::Inconsistent("withdrawn revision"))?;
                Ok(WithdrawalView {
                    revision_no,
                    reason: w.reason,
                    withdrawn_by: user_view(conn, w.withdrawn_by)?,
                    withdrawn_at: w.withdrawn_at,
                    notice_artifact_sha256: w.notice_artifact_sha256,
                })
            })
            .transpose()?,
        exports: export_views(conn, report_id, &revisions)?,
        revisions: revisions
            .iter()
            .map(|r| revision_summary(conn, r))
            .collect::<Result<_, _>>()?,
        obligations,
        incidents: report_incidents(conn, report_id, caller)?,
        allowed_actions,
    })
}

/// The impact on the current revision, or `None` while its stored content
/// fails its digest: the report stays readable, with its incidents, and
/// the content is never shown (NFR-07).
fn current_impact(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<Option<ImpactView>, ReportingError> {
    match impact_view(conn, revision) {
        Ok(view) => Ok(Some(view)),
        Err(ReportingError::ContentMismatch(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn obligation_views(
    conn: &Connection,
    report_id: ReportId,
) -> Result<Vec<ObligationView>, ReportingError> {
    let obligations = delivery::report_obligations(conn, report_id)?;
    if obligations.is_empty() {
        return Ok(Vec::new());
    }
    let mut context = DeliveryContext::new(conn)?;
    obligations
        .iter()
        .map(|o| obligation_view(conn, &mut context, o))
        .collect()
}

/// The report-level actions open to `caller`: prepare and withdraw while
/// not withdrawn (`release_reports`), add an obligation to a current
/// revision (`deliver_reports`), assess open impact (`manage_quality`).
fn report_actions(
    caller: &Principal,
    latest: Option<RevisionAvailability>,
    impact: Option<&ImpactView>,
    withdrawn: bool,
) -> Vec<String> {
    let holds = |capability| caller.require(capability).is_ok();
    let mut actions = Vec::new();
    if !withdrawn && holds(Capability::ReleaseReports) {
        actions.push("prepare_report_candidate");
        if latest.is_some() {
            actions.push("withdraw_report");
        }
    }
    if latest == Some(RevisionAvailability::Current) && holds(Capability::DeliverReports) {
        actions.push("add_delivery_obligation");
    }
    if impact.is_some_and(|i| !i.open.is_empty()) && holds(Capability::ManageQuality) {
        actions.push("record_impact_assessment");
    }
    actions.into_iter().map(str::to_owned).collect()
}

fn export_views(
    conn: &Connection,
    report_id: ReportId,
    revisions: &[RevisionRow],
) -> Result<Vec<ExportView>, ReportingError> {
    exports::report_exports(conn, report_id)?
        .into_iter()
        .map(|e| {
            let exported_by = user_view(conn, e.exported_by)?;
            Ok(ExportView {
                export_id: e.id,
                revision_no: e
                    .revision_id
                    .and_then(|id| revisions.iter().find(|r| r.id == id))
                    .map(|r| r.revision_no),
                withdrawal_notice: e.withdrawal_report_id.is_some(),
                purpose: e.purpose,
                file_name: e.file_name,
                label: export_label(&exported_by.display_name),
                exported_by,
                exported_at: e.exported_at,
            })
        })
        .collect()
}

/// The impact on a current revision (REQ-RPT-002).
pub(crate) fn impact_view(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<ImpactView, ReportingError> {
    let impact = impact::impact(conn, revision)?;
    Ok(ImpactView {
        revision_id: revision.id,
        open: impact
            .open
            .into_iter()
            .map(|r| ImpactReasonView {
                key: r.key,
                code: r.code.to_owned(),
                message: r.message,
            })
            .collect(),
        correction_due: impact.correction_due,
        assessments: impact
            .assessments
            .into_iter()
            .map(|a| {
                Ok(ImpactAssessmentView {
                    assessment_id: a.id,
                    decision: a.decision,
                    reason: a.reason,
                    keys: a.keys,
                    assessed_by: user_view(conn, a.assessed_by)?,
                    assessed_at: a.assessed_at,
                })
            })
            .collect::<Result<_, ReportingError>>()?,
    })
}

/// One revision with its exact printed content and provenance; the bytes
/// are never regenerated (NFR-07). PHI: an audited read.
pub fn get_revision(
    conn: &Connection,
    revision_id: ReportRevisionId,
) -> Result<RevisionDetail, ReportingError> {
    let revision =
        reports::revision(conn, revision_id)?.ok_or(ReportingError::NotFound("report revision"))?;
    let report = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("revision report"))?;
    let candidate = candidates::candidate(conn, revision.candidate_id)?
        .ok_or(ReportingError::Inconsistent("released candidate"))?;
    let rendered = candidate
        .render
        .as_ref()
        .ok_or(ReportingError::Inconsistent("released candidate artifact"))?;
    let content = stored_content(conn, &candidate)?.data;
    Ok(RevisionDetail {
        report_id: report.id,
        report_number: report.report_number,
        summary: revision_summary(conn, &revision)?,
        content,
        provenance: RevisionProvenance {
            template_id: candidate.template_id.clone(),
            template_version: candidate.template_version,
            template_sha256: rendered.template_sha256.clone(),
            renderer_version: rendered.renderer_version.clone(),
            content_sha256: candidate.content_sha256.clone(),
            data_sha256: candidate.data_sha256.clone(),
            artifact_sha256: rendered.artifact_sha256.clone(),
            fonts: serde_json::from_str(&rendered.fonts_json)?,
        },
    })
}

lis_core::text_enum! {
    /// Report list filter.
    #[ts(export)]
    ReportListFilter {
        /// Every report with a released revision.
        All = "all",
        /// Reports whose current revision is partial.
        Partial = "partial",
        /// Withdrawn reports.
        Withdrawn = "withdrawn",
    }
}

/// Input of `list_reports`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListReportsInput {
    /// Which reports.
    pub filter: ReportListFilter,
    /// A report number prefix.
    #[serde(default)]
    #[ts(optional)]
    pub search: Option<String>,
    /// At most this many.
    pub limit: u32,
    /// Only reports whose latest release is before this time (paging).
    #[serde(default)]
    #[ts(optional)]
    pub before_released_at: Option<UtcTimestamp>,
    /// Tie-breaker for a continuation at the same release timestamp.
    #[serde(default)]
    #[ts(optional)]
    pub before_report_id: Option<ReportId>,
}

/// Reports with a released revision, latest release first. No PHI.
pub fn list_reports(
    conn: &Connection,
    input: &ListReportsInput,
) -> Result<Vec<ReportSummary>, ReportingError> {
    if input.before_report_id.is_some() && input.before_released_at.is_none() {
        return Err(crate::error::invalid(
            "beforeReportId",
            "missing_cursor_time",
            "A report cursor needs its release timestamp.",
        ));
    }
    let latest = listed_revision_ids(conn, input)?
        .into_iter()
        .map(|id| {
            reports::revision(conn, id)?.ok_or(ReportingError::Inconsistent("listed revision"))
        })
        .collect::<Result<Vec<_>, ReportingError>>()?;
    let mut summaries = latest
        .into_iter()
        .map(|revision| summary(conn, revision))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = summaries.len() > usize::try_from(input.limit).unwrap_or(usize::MAX);
    summaries.truncate(usize::try_from(input.limit).unwrap_or(usize::MAX));
    for summary in &mut summaries {
        summary.has_more = Some(has_more);
    }
    Ok(summaries)
}

fn summary(conn: &Connection, revision: RevisionRow) -> Result<ReportSummary, ReportingError> {
    let report = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("revision report"))?;
    let accession = lis_specimens::specimen_accession(conn, report.specimen_id)?;
    Ok(ReportSummary {
        report_id: report.id,
        report_number: report.report_number,
        specimen_id: report.specimen_id,
        accession_number: lis_specimens::get_accession(conn, accession)?.accession_number,
        latest_revision_no: revision.revision_no,
        availability: revision.availability,
        completeness: revision.completeness,
        released_at: revision.released_at,
        has_more: Some(false),
    })
}

fn listed_revision_ids(
    conn: &Connection,
    input: &ListReportsInput,
) -> Result<Vec<ReportRevisionId>, ReportingError> {
    let prefix = input.search.as_deref().map(str::trim).unwrap_or("");
    let query_limit = input.limit.saturating_add(1);
    let ids = conn
        .prepare_cached(
            "SELECT v.revision_id FROM rpt_v_revisions v \
             WHERE v.revision_no = (SELECT max(o.revision_no) FROM rpt_v_revisions o \
                                    WHERE o.report_id = v.report_id) \
               AND (?1 = 'all' OR (?1 = 'withdrawn' AND v.availability = 'withdrawn') \
                    OR (?1 = 'partial' AND v.availability = 'current' AND v.completeness = 'partial')) \
               AND substr(v.report_number, 1, length(?2)) = ?2 \
               AND (?3 IS NULL OR v.released_at < ?3 OR (v.released_at = ?3 AND ?5 IS NOT NULL AND v.report_id > ?5)) \
             ORDER BY v.released_at DESC, v.report_id LIMIT ?4",
        )?
        .query_map(
            params![input.filter, prefix, input.before_released_at, query_limit, input.before_report_id],
            |row| row.get(0),
        )?
        .collect::<rusqlite::Result<Vec<ReportRevisionId>>>()?;
    Ok(ids)
}

#[cfg(test)]
mod paging_tests {
    use super::*;
    #[test]
    fn equal_release_times_continue_after_retained_cursor_even_if_prior_row_disappears() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE rpt_v_revisions(revision_id TEXT, report_id TEXT, report_number TEXT, revision_no INTEGER, availability TEXT, completeness TEXT, released_at TEXT)").unwrap();
        let at: UtcTimestamp = "2026-10-14T12:00:00.000000000Z".parse().unwrap();
        let mut ids = Vec::new();
        for _ in 0..205 {
            let id = ReportId::generate();
            ids.push(id);
            conn.execute(
                "INSERT INTO rpt_v_revisions VALUES (?1,?2,?3,1,'current','final',?4)",
                params![ReportRevisionId::generate(), id, id.to_string(), at],
            )
            .unwrap();
        }
        ids.sort_unstable();
        let mut input = ListReportsInput {
            filter: ReportListFilter::All,
            search: None,
            limit: 200,
            before_released_at: None,
            before_report_id: None,
        };
        assert_eq!(listed_revision_ids(&conn, &input).unwrap().len(), 201);
        input.before_released_at = Some(at);
        input.before_report_id = Some(ids[199]);
        conn.execute("DELETE FROM rpt_v_revisions WHERE report_id=?1", [ids[199]])
            .unwrap();
        let tail = listed_revision_ids(&conn, &input).unwrap();
        assert_eq!(tail.len(), 5);
        let reports: Vec<ReportId> = tail
            .iter()
            .map(|id| {
                conn.query_row(
                    "SELECT report_id FROM rpt_v_revisions WHERE revision_id=?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(reports, &ids[200..]);
        input.before_report_id = None;
        assert!(
            listed_revision_ids(&conn, &input).unwrap().is_empty(),
            "Legacy timestamp-only cursor keeps its historical strict-time semantics"
        );
    }
}
