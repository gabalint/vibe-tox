//! Reporting work queues and release reconciliation for the work-queue
//! screen (REQ-OPS-004). Identifiers and states only: no PHI, not audited.

use lis_core::{GateReason, UtcTimestamp};
use lis_specimens::{SpecimenAcceptance, SpecimenId};
use rusqlite::{Connection, params};
use serde::Serialize;
use ts_rs::TS;

use crate::error::ReportingError;
use crate::gates;
use crate::impact;
use crate::model::{ReportCompleteness, ReportId, ReportRevisionId, RevisionAvailability};
use crate::repo::reports;
use crate::snapshot;

/// A specimen with reviewed results not on its current revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReleaseQueueItem {
    pub specimen_id: SpecimenId,
    /// Human accession identifier, present before a report number exists.
    #[ts(optional)]
    pub accession_number: Option<String>,
    /// Set once a candidate was prepared.
    pub report_number: Option<String>,
    /// The oldest review not yet released.
    pub since: UtcTimestamp,
    /// Gates that block preparing a candidate now; empty when ready.
    pub gates: Vec<GateReason>,
    /// Another matching row exists beyond the requested limit.
    #[ts(optional)]
    pub has_more: Option<bool>,
}

/// Which release queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseQueue {
    /// No gate blocks preparing a candidate.
    Ready,
    /// A gate blocks preparing a candidate.
    Blocked,
}

/// Specimens whose current approved reviews are not on a current revision,
/// oldest review first: `res_v_reviewed_lines` anti-joined with current
/// `rpt_v_released_lines` (brief §6). Withdrawn reports are excluded. Ready
/// and blocked are told apart before the limit, so neither hides the other.
pub fn release_queue(
    conn: &Connection,
    queue: ReleaseQueue,
    limit: u32,
) -> Result<Vec<ReleaseQueueItem>, ReportingError> {
    let rows: Vec<(SpecimenId, UtcTimestamp)> = conn
        .prepare_cached(
            "SELECT l.specimen_id, min(l.reviewed_at) FROM res_v_reviewed_lines l \
             WHERE NOT EXISTS (SELECT 1 FROM rpt_v_released_lines r \
                 WHERE r.specimen_id = l.specimen_id AND r.availability = 'current' \
                   AND r.disposition = 'reported' AND r.result_revision_id = l.revision_id) \
               AND NOT EXISTS (SELECT 1 FROM rpt_reports rep \
                 JOIN rpt_withdrawals w ON w.report_id = rep.id WHERE rep.specimen_id = l.specimen_id) \
             GROUP BY l.specimen_id ORDER BY 2, 1",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let query_limit = limit.saturating_add(1);
    let mut items = Vec::new();
    for (specimen_id, since) in rows {
        if items.len() >= query_limit {
            break;
        }
        let gates = preparation_gates(conn, specimen_id)?;
        let wanted = match queue {
            ReleaseQueue::Ready => gates.is_empty(),
            ReleaseQueue::Blocked => !gates.is_empty(),
        };
        if wanted {
            let accession_id = lis_specimens::specimen_accession(conn, specimen_id)?;
            items.push(ReleaseQueueItem {
                specimen_id,
                accession_number: Some(
                    lis_specimens::get_accession(conn, accession_id)?.accession_number,
                ),
                report_number: reports::report_for_specimen(conn, specimen_id)?
                    .map(|r| r.report_number),
                since,
                gates,
                has_more: Some(false),
            });
        }
    }
    let has_more = items.len() > limit;
    items.truncate(limit);
    for item in &mut items {
        item.has_more = Some(has_more);
    }
    Ok(items)
}

/// The gates that block preparing a candidate of `specimen_id` now.
fn preparation_gates(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Vec<GateReason>, ReportingError> {
    match snapshot::build(conn, specimen_id) {
        Ok(built) => {
            let mut gates = gates::content_gates(&built.snapshot, &built.manifest);
            gates.extend(gates::evaluate(conn, &built.snapshot)?);
            Ok(gates)
        }
        Err(ReportingError::GateBlocked(reasons)) => Ok(reasons),
        Err(error) => Err(error),
    }
}

/// A current revision in a reporting queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RevisionQueueItem {
    pub report_id: ReportId,
    pub report_number: String,
    pub revision_id: ReportRevisionId,
    #[ts(type = "number")]
    pub revision_no: i64,
    pub released_at: UtcTimestamp,
    /// Open impact reasons; zero outside the impact queue.
    #[ts(type = "number")]
    pub open_impact: usize,
    /// An assessment requires a correction not yet issued.
    pub correction_due: bool,
}

/// Current partial revisions, oldest release first (REQ-RPT-005).
pub fn partial_reports(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<RevisionQueueItem>, ReportingError> {
    let mut items = Vec::new();
    for revision in reports::current_revisions(conn)? {
        if revision.completeness == ReportCompleteness::Partial {
            items.push(item(conn, &revision, 0, false)?);
        }
        if items.len() >= usize::try_from(limit).unwrap_or(usize::MAX) {
            break;
        }
    }
    Ok(items)
}

/// Current revisions with open impact or a correction due, oldest release
/// first (REQ-RPT-002; E3: computed over every current revision). A
/// revision whose stored content fails its digest is left out: the
/// incidents queue lists it (NFR-07).
pub fn impact_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<RevisionQueueItem>, ReportingError> {
    let mut queued = Vec::new();
    for revision in reports::current_revisions(conn)? {
        let impact = match impact::impact(conn, &revision) {
            Ok(impact) => impact,
            Err(ReportingError::ContentMismatch(_)) => continue,
            Err(error) => return Err(error),
        };
        if impact.pauses_delivery() {
            queued.push((
                impact::release_sequence(conn, &revision)?,
                item(conn, &revision, impact.open.len(), impact.correction_due)?,
            ));
        }
    }
    queued.sort_unstable_by_key(|(seq, _)| *seq);
    Ok(queued
        .into_iter()
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|(_, item)| item)
        .collect())
}

fn item(
    conn: &Connection,
    revision: &reports::RevisionRow,
    open_impact: usize,
    correction_due: bool,
) -> Result<RevisionQueueItem, ReportingError> {
    let report = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("revision report"))?;
    Ok(RevisionQueueItem {
        report_id: report.id,
        report_number: report.report_number,
        revision_id: revision.id,
        revision_no: revision.revision_no,
        released_at: revision.released_at,
        open_impact,
        correction_due,
    })
}

/// Accepted specimens in a period, by where their report stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReleaseReconciliation {
    pub from: Option<UtcTimestamp>,
    pub to: Option<UtcTimestamp>,
    #[ts(type = "number")]
    pub accepted: usize,
    #[ts(type = "number")]
    pub final_reports: usize,
    #[ts(type = "number")]
    pub partial_reports: usize,
    #[ts(type = "number")]
    pub unreleased: usize,
    #[ts(type = "number")]
    pub withdrawn: usize,
    #[ts(type = "number")]
    pub rejected: usize,
}

/// Specimens accepted in `[from, to)` = final + partial + unreleased +
/// withdrawn + rejected after acceptance (REQ-OPS-004).
pub fn release_reconciliation(
    conn: &Connection,
    from: UtcTimestamp,
    to: UtcTimestamp,
) -> Result<ReleaseReconciliation, ReportingError> {
    let mut specimens = lis_specimens::accepted_specimens_between(conn, from, to)?;
    specimens.sort_unstable();
    specimens.dedup();
    let mut counts = ReleaseReconciliation {
        from: Some(from),
        to: Some(to),
        accepted: specimens.len(),
        ..ReleaseReconciliation::default()
    };
    for specimen_id in specimens {
        let latest = match reports::report_for_specimen(conn, specimen_id)? {
            Some(report) => reports::latest_revision(conn, report.id)?,
            None => None,
        };
        match latest.map(|r| (r.availability, r.completeness)) {
            Some((RevisionAvailability::Withdrawn, _)) => counts.withdrawn += 1,
            Some((_, ReportCompleteness::Final)) => counts.final_reports += 1,
            Some((_, ReportCompleteness::Partial)) => counts.partial_reports += 1,
            None if rejected(conn, specimen_id)? => counts.rejected += 1,
            None => counts.unreleased += 1,
        }
    }
    Ok(counts)
}

fn rejected(conn: &Connection, specimen_id: SpecimenId) -> Result<bool, ReportingError> {
    let accession = lis_specimens::specimen_accession(conn, specimen_id)?;
    Ok(lis_specimens::get_accession(conn, accession)?
        .specimens
        .iter()
        .any(|s| s.specimen_id == specimen_id && s.acceptance == SpecimenAcceptance::Rejected))
}

/// Reports whose current revision has no delivery obligation (the
/// "no destination" delivery queue).
pub fn revisions_without_destination(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<RevisionQueueItem>, ReportingError> {
    let ids: Vec<ReportRevisionId> = conn
        .prepare_cached(
            "SELECT v.revision_id FROM rpt_v_revisions v WHERE v.availability = 'current' \
             AND NOT EXISTS (SELECT 1 FROM rpt_delivery_obligations o \
                             WHERE o.revision_id = v.revision_id) \
             ORDER BY v.released_at, v.revision_id LIMIT ?1",
        )?
        .query_map(params![limit], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let mut items = Vec::new();
    for id in ids {
        let revision =
            reports::revision(conn, id)?.ok_or(ReportingError::Inconsistent("queued revision"))?;
        items.push(item(conn, &revision, 0, false)?);
    }
    Ok(items)
}
