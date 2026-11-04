//! What sending an issued revision to an outside system needs (Interfaces,
//! Report PDF and HL7 Results connections): the revision's identity, its stored
//! PDF, its frozen content, the parties its rules route on, and whether its
//! delivery is paused (L13). The content is PHI; callers must not log it.

use lis_core::UtcTimestamp;
use lis_orders::OrderParties;
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::model::{ReportId, ReportRelationship, ReportRevisionId};
use crate::repo::reports;
use crate::snapshot::ReportSnapshot;

/// A released revision as an outgoing message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionOutbound {
    pub report_id: ReportId,
    pub report_number: String,
    pub revision_no: i64,
    pub relationship: ReportRelationship,
    pub released_at: UtcTimestamp,
    /// The issued PDF's digest; its bytes never change.
    pub artifact_sha256: String,
    /// The order's client, practitioner and location, and the tests on this
    /// report's specimen, as frozen at release.
    pub parties: OrderParties,
    /// The content the revision froze, verified against its digest.
    pub snapshot: ReportSnapshot,
}

/// Revision `revision_id` as an outgoing message, read in the caller's transaction.
pub fn revision_outbound(
    conn: &Connection,
    revision_id: ReportRevisionId,
) -> Result<RevisionOutbound, ReportingError> {
    let revision =
        reports::revision(conn, revision_id)?.ok_or(ReportingError::NotFound("report revision"))?;
    let report = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("revision report"))?;
    let snapshot = ReportSnapshot::of_revision(conn, &revision)?;
    let mut parties = OrderParties::default();
    parties.add_order(
        conn,
        snapshot.order.order_id,
        Some(&snapshot.specimen.order_item_ids),
    )?;
    Ok(RevisionOutbound {
        report_id: report.id,
        report_number: report.report_number,
        revision_no: revision.revision_no,
        relationship: revision.relationship,
        released_at: revision.released_at,
        artifact_sha256: revision.artifact_sha256,
        parties,
        snapshot,
    })
}

/// The latest revision of report `report_id`: the withdrawn one, after a withdrawal.
pub fn latest_revision_id(
    conn: &Connection,
    report_id: ReportId,
) -> Result<ReportRevisionId, ReportingError> {
    Ok(reports::latest_revision(conn, report_id)?
        .ok_or(ReportingError::NotFound("report revision"))?
        .id)
}

/// L13: whether revision `revision_id`'s PDF must wait: while it is current with
/// open impact or a correction due, or its stored content fails its digest.
pub fn revision_delivery_paused(
    conn: &Connection,
    revision_id: ReportRevisionId,
) -> Result<bool, ReportingError> {
    let revision =
        reports::revision(conn, revision_id)?.ok_or(ReportingError::NotFound("report revision"))?;
    super::delivery::paused(conn, &revision)
}
