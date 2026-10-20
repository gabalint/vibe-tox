//! The released lines billing derives from (brief §5.2, REQ-BIL-005): a
//! specimen's current, final, released revision and its reported lines,
//! read from `rpt_v_revisions` and `rpt_v_released_lines`. Read-only, with
//! no rule of its own: availability and completeness are reporting's.

use lis_catalog::AnalyteId;
use lis_core::UtcTimestamp;
use lis_orders::OrderComponentId;
use lis_results::ResultRevisionId;
use lis_specimens::SpecimenId;
use rusqlite::{Connection, OptionalExtension};

use crate::error::ReportingError;
use crate::model::{ReportId, ReportRevisionId};

/// A specimen's current final revision, which billing counts as reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentFinalRelease {
    /// The report.
    pub report_id: ReportId,
    /// Its PHI-free number.
    pub report_number: String,
    /// The current revision.
    pub revision_id: ReportRevisionId,
    /// Its number, from 1.
    pub revision_no: i64,
    /// When it was released.
    pub released_at: UtcTimestamp,
    /// The lines with disposition `reported`.
    pub reported_lines: Vec<ReportedLine>,
}

/// One analyte reported on a revision, with the result revision it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedLine {
    /// The ordered component.
    pub order_component_id: OrderComponentId,
    /// The analyte.
    pub analyte_id: AnalyteId,
    /// The exact result revision on the report.
    pub result_revision_id: ResultRevisionId,
}

/// The current revision of `specimen_id`'s report when it is final and not
/// withdrawn; `None` otherwise. Usable inside a transaction.
pub fn current_final_release(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<Option<CurrentFinalRelease>, ReportingError> {
    let Some((report_id, report_number, revision_id, revision_no, released_at)) = conn
        .prepare_cached(
            "SELECT report_id, report_number, revision_id, revision_no, released_at \
             FROM rpt_v_revisions \
             WHERE specimen_id = ?1 AND availability = 'current' AND completeness = 'final'",
        )?
        .query_row([specimen_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .optional()?
    else {
        return Ok(None);
    };
    let reported_lines = conn
        .prepare_cached(
            "SELECT order_component_id, analyte_id, result_revision_id FROM rpt_v_released_lines \
             WHERE revision_id = ?1 AND disposition = 'reported' \
             ORDER BY order_component_id, analyte_id",
        )?
        .query_map([revision_id], |row| {
            Ok(ReportedLine {
                order_component_id: row.get(0)?,
                analyte_id: row.get(1)?,
                result_revision_id: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(CurrentFinalRelease {
        report_id,
        report_number,
        revision_id,
        revision_no,
        released_at,
        reported_lines,
    }))
}
