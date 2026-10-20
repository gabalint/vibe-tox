//! Delivery reads: obligation status and labels, and the delivery queue
//! (REQ-RPT-003, REQ-RPT-011, REQ-RPT-012, REQ-RPT-013). No PHI: report
//! numbers, labels and states only.

use std::collections::HashMap;

use lis_identity::ClientId;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::obligation::obligation_view;
use crate::delivery::labels::{BlockedBy, DeliveryStatus};
use crate::error::{ReportingError, invalid};
use crate::impact;
use crate::model::{
    DeliveryEvidence, ObligationId, ObligationKind, ObligationStatus, ReportId, ReportRevisionId,
    RevisionAvailability,
};
use crate::policy::DELIVERY_PAUSED_WHILE_IMPACT_OPEN;
use crate::queries::candidate::user_view;
use crate::repo::candidates;
use crate::repo::delivery::{self, AttemptRow, DestinationRow, ObligationRow};
use crate::repo::reports::{self, RevisionRow};
use crate::stored::content_incident;
use crate::views::ObligationView;

/// Reads shared by the obligation views of one request.
pub(crate) struct DeliveryContext {
    time_zone: String,
    paused: HashMap<ReportRevisionId, bool>,
    /// Whether a current revision's stored content fails its digest; the
    /// delivery queue leaves such a revision out (NFR-07).
    damaged: HashMap<ReportRevisionId, bool>,
    clients: HashMap<ReportId, ClientId>,
}

impl DeliveryContext {
    pub(crate) fn new(conn: &Connection) -> Result<Self, ReportingError> {
        let time_zone = lis_identity::current_laboratory_profile(conn)?
            .map(|p| p.time_zone)
            .ok_or(ReportingError::Inconsistent("laboratory profile"))?;
        Ok(Self {
            time_zone,
            paused: HashMap::new(),
            damaged: HashMap::new(),
            clients: HashMap::new(),
        })
    }

    pub(super) fn client(
        &mut self,
        conn: &Connection,
        report_id: ReportId,
    ) -> Result<ClientId, ReportingError> {
        if let Some(client_id) = self.clients.get(&report_id) {
            return Ok(*client_id);
        }
        let client_id = order_client(conn, report_id)?;
        self.clients.insert(report_id, client_id);
        Ok(client_id)
    }

    /// L13: whether delivery of `revision`'s report PDF is paused; it is
    /// while its stored content fails its digest and impact is unknown.
    fn paused(
        &mut self,
        conn: &Connection,
        revision: &RevisionRow,
    ) -> Result<bool, ReportingError> {
        if !DELIVERY_PAUSED_WHILE_IMPACT_OPEN
            || revision.availability != RevisionAvailability::Current
        {
            return Ok(false);
        }
        if let Some(paused) = self.paused.get(&revision.id) {
            return Ok(*paused);
        }
        let paused = match impact::impact(conn, revision) {
            Ok(impact) => impact.pauses_delivery(),
            Err(ReportingError::ContentMismatch(_)) => true,
            Err(error) => return Err(error),
        };
        self.paused.insert(revision.id, paused);
        Ok(paused)
    }

    /// Whether `revision` is current and its stored content fails its
    /// digest (NFR-07), whatever its obligations' status.
    fn damaged(
        &mut self,
        conn: &Connection,
        revision: &RevisionRow,
    ) -> Result<bool, ReportingError> {
        if revision.availability != RevisionAvailability::Current {
            return Ok(false);
        }
        if let Some(damaged) = self.damaged.get(&revision.id) {
            return Ok(*damaged);
        }
        let candidate = candidates::candidate(conn, revision.candidate_id)?
            .ok_or(ReportingError::Inconsistent("released candidate"))?;
        let damaged = content_incident(conn, &candidate)?.is_some();
        self.damaged.insert(revision.id, damaged);
        Ok(damaged)
    }
}

/// The ordering client of a report's specimen: the only client whose
/// destinations may receive it (RPT-012).
pub(crate) fn order_client(
    conn: &Connection,
    report_id: ReportId,
) -> Result<ClientId, ReportingError> {
    let report =
        reports::report(conn, report_id)?.ok_or(ReportingError::Inconsistent("revision report"))?;
    let accession = lis_specimens::specimen_accession(conn, report.specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession)?;
    Ok(lis_orders::get_order(conn, accession.order_id)?.client_id)
}

/// The status of an obligation; the first match wins (brief §2.1).
pub(crate) fn status(
    conn: &Connection,
    context: &mut DeliveryContext,
    obligation: &ObligationRow,
    destination: &DestinationRow,
    revision: &RevisionRow,
    attempts: &[AttemptRow],
) -> Result<DeliveryStatus, ReportingError> {
    if let Some(cause) = obligation.closure {
        return Ok(DeliveryStatus::Closed(cause));
    }
    if destination.retired_at.is_some() {
        return Ok(DeliveryStatus::Blocked(BlockedBy::Retired));
    }
    if destination.reported_wrong_at.is_some() {
        return Ok(DeliveryStatus::Blocked(BlockedBy::ReportedWrong));
    }
    if destination.verification.is_none() {
        return Ok(DeliveryStatus::Blocked(BlockedBy::Unverified));
    }
    if obligation.kind == ObligationKind::Report && context.paused(conn, revision)? {
        return Ok(DeliveryStatus::Paused);
    }
    if let Some(handed) = attempts
        .iter()
        .find(|a| a.evidence == DeliveryEvidence::HandedOff)
    {
        return Ok(DeliveryStatus::HandedOff {
            attested_by: user_view(conn, handed.recorded_by)?.display_name,
            observed_at: crate::display::lab_time(handed.observed_at, &context.time_zone)?,
        });
    }
    Ok(match attempts.last() {
        Some(AttemptRow {
            evidence: DeliveryEvidence::Failed,
            failure_code: Some(code),
            ..
        }) => DeliveryStatus::Failed(*code),
        Some(_) => DeliveryStatus::OutcomeUnknown,
        None => DeliveryStatus::Awaiting,
    })
}

lis_core::text_enum! {
    /// Delivery queue filter.
    #[ts(export)]
    DeliveryQueueFilter {
        /// No attempt yet.
        Awaiting = "awaiting",
        /// The latest attempt failed or has an unknown outcome.
        Attention = "attention",
        /// The destination is unverified, reported wrong or retired.
        Blocked = "blocked",
        /// Impact on the revision is open.
        Paused = "paused",
        /// Every obligation not closed, including handed-off ones.
        All = "all",
    }
}

impl DeliveryQueueFilter {
    fn admits(self, code: ObligationStatus) -> bool {
        match self {
            Self::Awaiting => code == ObligationStatus::Awaiting,
            Self::Attention => code == ObligationStatus::Attention,
            Self::Blocked => code == ObligationStatus::Blocked,
            Self::Paused => code == ObligationStatus::Paused,
            Self::All => true,
        }
    }
}

/// The most obligations one page of the delivery queue returns.
const QUEUE_LIMIT_MAX: u32 = 500;

/// Input of `list_delivery_obligations`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListDeliveryObligationsInput {
    /// Which obligations.
    pub filter: DeliveryQueueFilter,
    /// At most this many, 1–500.
    pub limit: u32,
    /// The last obligation of the previous page; `None` for the first page.
    #[ts(optional)]
    pub after: Option<ObligationId>,
}

/// Pages of SQL rows one queue request reads at most. Only rows left out
/// after the SQL filter (paused, damaged content) use more than one.
const QUEUE_SCAN_PAGES: usize = 4;

/// A bounded queue scan. `next_after` is set when the scan stopped before
/// reaching the end, even when every scanned row was omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeliveryQueuePage {
    pub items: Vec<ObligationView>,
    pub next_after: Option<ObligationId>,
}

/// One page of the obligations not closed: withdrawal notices first, then
/// corrections, then oldest first (REQ-RPT-013). The SQL pages by the
/// queue's stable sort key and narrows rows by their stored status; only
/// the page's rows are expanded. Rows the computed pause or damaged
/// content leaves out are replaced by the following ones, read a page at a
/// time, for at most [`QUEUE_SCAN_PAGES`] pages, so one request reads a
/// bounded number of rows (NFR-05, NFR-07).
pub fn delivery_queue_page(
    conn: &Connection,
    input: ListDeliveryObligationsInput,
) -> Result<DeliveryQueuePage, ReportingError> {
    if !(1..=QUEUE_LIMIT_MAX).contains(&input.limit) {
        return Err(invalid(
            "limit",
            "out_of_range",
            "Use a limit from 1 to 500.",
        ));
    }
    if let Some(after) = input.after
        && delivery::obligation(conn, after)?.is_none()
    {
        return Err(invalid("after", "not_found", "Reload the delivery queue."));
    }
    let limit = usize::try_from(input.limit).unwrap_or(usize::MAX);
    let mut rows = delivery::queue_page(conn, input.filter, input.after, input.limit)?;
    if rows.is_empty() {
        return Ok(DeliveryQueuePage {
            items: Vec::new(),
            next_after: None,
        });
    }
    let mut context = DeliveryContext::new(conn)?;
    let mut views = Vec::new();
    for scan in 0..QUEUE_SCAN_PAGES {
        for row in &rows {
            views.extend(queued_view(conn, &mut context, row, input.filter)?);
            if views.len() == limit {
                return Ok(DeliveryQueuePage {
                    items: views,
                    next_after: Some(row.id),
                });
            }
        }
        let Some(last) = rows.last() else {
            return Ok(DeliveryQueuePage {
                items: views,
                next_after: None,
            });
        };
        if rows.len() < limit {
            return Ok(DeliveryQueuePage {
                items: views,
                next_after: None,
            });
        }
        if scan + 1 == QUEUE_SCAN_PAGES {
            return Ok(DeliveryQueuePage {
                items: views,
                next_after: Some(last.id),
            });
        }
        rows = delivery::queue_page(conn, input.filter, Some(last.id), input.limit)?;
        if rows.is_empty() {
            return Ok(DeliveryQueuePage {
                items: views,
                next_after: None,
            });
        }
    }
    unreachable!("the bounded queue scan always returns")
}

/// Compatibility read for composite work queues, which consume only the
/// visible items and apply their own queue bound.
pub fn delivery_queue(
    conn: &Connection,
    input: ListDeliveryObligationsInput,
) -> Result<Vec<ObligationView>, ReportingError> {
    // Composite queues supply their own final bound and continuation. Read every
    // required owner page so their cursor is never applied to a truncated source.
    // The desktop page command keeps its separate 1..=500 input contract.
    if input.limit <= QUEUE_LIMIT_MAX {
        return Ok(delivery_queue_page(conn, input)?.items);
    }
    let mut rows = Vec::new();
    let mut after = input.after;
    while rows.len() < input.limit as usize {
        let remaining = input.limit.saturating_sub(rows.len() as u32);
        let page = delivery_queue_page(
            conn,
            ListDeliveryObligationsInput {
                filter: input.filter,
                limit: remaining.min(QUEUE_LIMIT_MAX),
                after,
            },
        )?;
        rows.extend(page.items);
        let Some(next) = page.next_after else {
            break;
        };
        after = Some(next);
    }
    Ok(rows)
}

/// `row` as the queue shows it, or `None` when `filter` leaves it out.
fn queued_view(
    conn: &Connection,
    context: &mut DeliveryContext,
    row: &ObligationRow,
    filter: DeliveryQueueFilter,
) -> Result<Option<ObligationView>, ReportingError> {
    let revision = reports::revision(conn, row.revision_id)?
        .ok_or(ReportingError::Inconsistent("obligation revision"))?;
    // NFR-07: the incidents queue lists a revision with damaged content.
    if context.damaged(conn, &revision)? {
        return Ok(None);
    }
    let mut view = obligation_view(conn, context, row)?;
    for attempt in &mut view.attempts {
        attempt.note = None;
    }
    Ok(filter.admits(view.status.code).then_some(view))
}
