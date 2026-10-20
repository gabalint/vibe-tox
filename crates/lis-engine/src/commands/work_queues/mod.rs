//! Capability-filtered and access-audited work queues (REQ-OPS-004, DM-03).
//!
//! This module owns the queue catalog and common DTO only. Orders, result
//! review, reporting and operational modules remain the homes of selection
//! rules. Counts are capped at 500; bounded item pages continue through all pending work.
//! `ops_alerts` lists only the checks whose owner capability the caller holds.

mod billing_items;
mod catalog;
mod ops_items;
mod owner_items;
mod reporting_items;
mod review_items;

use lis_audit::AuditDetail;
use lis_core::{CommandError, ErrorCode, FieldError, Principal, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::catalog::{QUEUES, QueueDefinition, queue_definition};
use crate::engine::Engine;
use crate::pipeline::{Failure, ModuleError, access, domain_failure};
use crate::registry::{Access, CommandSpec, spec};

pub(crate) const LIST_WORK_QUEUES: CommandSpec = spec(
    "list_work_queues",
    "operations.list_work_queues",
    Access::Session,
);
pub(crate) const LIST_WORK_QUEUE_ITEMS: CommandSpec = spec(
    "list_work_queue_items",
    "operations.list_work_queue_items",
    Access::Session,
);

pub(super) const MAX_ITEMS: u32 = 500;

/// One visible queue. Historical dynamic counts are unknown until its bounded scan completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkQueueSummary {
    pub queue: String,
    pub label: String,
    pub owner_capability: String,
    /// None means the historical queue has not been scanned; it never means zero work.
    #[ts(type = "number | null")]
    pub count: Option<usize>,
    /// The count is a lower bound: additional matching work or unchecked candidates remain.
    pub count_capped: bool,
    pub oldest_since: Option<UtcTimestamp>,
    pub next_action: String,
}

/// Input for one queue's bounded items.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListWorkQueueItemsInput {
    pub queue: String,
    #[ts(type = "number")]
    pub limit: u32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after: Option<WorkQueueCursor>,
}

/// Stable continuation value. Source times are used for navigation, never inferred causal order.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct WorkQueueCursor {
    pub since: UtcTimestamp,
    pub item_kind: String,
    pub item_id: String,
}

/// Operational shape without patient details, composed from an owner queue item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkQueueItem {
    pub queue: String,
    pub item_kind: String,
    pub item_id: String,
    pub title: String,
    pub state: String,
    /// Plain wording for `state`, owned beside the queue's next action.
    #[ts(optional)]
    pub state_label: Option<String>,
    pub since: UtcTimestamp,
    pub next_action: String,
    /// Another page may contain matching work; unchecked review candidates are not known matches.
    #[ts(optional)]
    pub has_more: Option<bool>,
}

/// A bounded operational response. `next` can exist even when `items` is empty.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkQueuePage {
    pub items: Vec<WorkQueueItem>,
    pub next: Option<WorkQueueCursor>,
    /// Candidate specimens checked during this page; zero for non-review queues.
    #[ts(type = "number")]
    pub scanned: usize,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum WorkQueueError {
    #[error("The review queue cursor is invalid. Refresh the queue and try again.")]
    InvalidReviewCursor,
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    #[error(transparent)]
    Order(#[from] lis_orders::OrderError),
    #[error(transparent)]
    Specimen(#[from] lis_specimens::SpecimenError),
    #[error(transparent)]
    Analytical(#[from] lis_analytical::AnalyticalError),
    #[error(transparent)]
    Result(#[from] lis_results::ResultError),
    #[error(transparent)]
    Reporting(#[from] lis_reporting::ReportingError),
    #[error(transparent)]
    Billing(#[from] lis_billing::BillingError),
    #[error(transparent)]
    Runtime(#[from] lis_ops::RuntimeError),
    #[error(transparent)]
    Backup(#[from] lis_ops::backup::BackupError),
    #[error(transparent)]
    Store(#[from] lis_store::StoreError),
}

impl ModuleError for WorkQueueError {
    fn denied(&self) -> Option<lis_core::Denied> {
        match self {
            Self::Store(error) => error.denied(),
            Self::InvalidReviewCursor
            | Self::Identity(_)
            | Self::Order(_)
            | Self::Specimen(_)
            | Self::Analytical(_)
            | Self::Result(_)
            | Self::Reporting(_)
            | Self::Billing(_)
            | Self::Runtime(_)
            | Self::Backup(_) => None,
        }
    }
}

impl From<WorkQueueError> for Failure {
    fn from(error: WorkQueueError) -> Self {
        match error {
            WorkQueueError::InvalidReviewCursor => validation(
                "after",
                "invalid_cursor",
                "The review queue cursor is invalid. Refresh the queue and try again.",
            ),
            WorkQueueError::Store(error) => error.into(),
            other => Failure::Internal(other.to_string()),
        }
    }
}

impl Engine {
    /// Lists only queues whose owner capability the caller currently holds.
    pub async fn list_work_queues(&self) -> Result<Vec<WorkQueueSummary>, CommandError> {
        self.run(&LIST_WORK_QUEUES, |call| async move {
            let principal = call.principal.clone();
            self.read(&call, move |conn| summaries(conn, &principal))
                .await
        })
        .await
    }

    /// Lists one permitted queue oldest first.
    pub async fn list_work_queue_items(
        &self,
        input: ListWorkQueueItemsInput,
    ) -> Result<WorkQueuePage, CommandError> {
        self.run(&LIST_WORK_QUEUE_ITEMS, |call| async move {
            let Some(definition) = queue_definition(&input.queue) else {
                return Err(validation("queue", "unknown", "Choose a known work queue."));
            };
            if input.limit == 0 || input.limit > MAX_ITEMS {
                return Err(validation(
                    "limit",
                    "out_of_range",
                    "Use a queue limit from 1 to 500.",
                ));
            }
            let capability = definition.capability;
            self.write(&call, move |ctx| ctx.require(capability))
                .await?;
            let principal = call.principal.clone();
            let page = self
                .read(&call, move |conn| {
                    load_page(
                        conn,
                        definition,
                        input.limit,
                        u32::MAX,
                        input.after.as_ref(),
                        &principal,
                    )
                })
                .await?;
            let event = access(LIST_WORK_QUEUE_ITEMS.action)
                .detail("queue", AuditDetail::code(definition.code))
                .detail("returned", AuditDetail::count(page.items.len()))
                .detail("scanned", AuditDetail::count(page.scanned));
            self.write(&call, move |ctx| {
                ctx.require(capability)?;
                ctx.audit(event).map(drop)
            })
            .await?;
            Ok(page)
        })
        .await
    }
}

fn summaries(
    conn: &rusqlite::Connection,
    principal: &Principal,
) -> Result<Vec<WorkQueueSummary>, WorkQueueError> {
    let mut result = Vec::new();
    for definition in QUEUES {
        if principal.require(definition.capability).is_err() {
            continue;
        }
        // Dynamic historical queues require progressive checks, not an eager full-history count.
        let page = if matches!(definition.code, "review_stale" | "review_blocked") {
            None
        } else {
            Some(load_page(
                conn,
                *definition,
                MAX_ITEMS,
                MAX_ITEMS + 1,
                None,
                principal,
            )?)
        };
        result.push(WorkQueueSummary {
            queue: definition.code.to_owned(),
            label: definition.label.to_owned(),
            owner_capability: definition.capability.as_str().to_owned(),
            count: page.as_ref().map(|page| page.items.len()),
            count_capped: page
                .as_ref()
                .is_some_and(|page| page.next.is_some() || page.items.len() == MAX_ITEMS as usize),
            oldest_since: page
                .as_ref()
                .and_then(|page| page.items.first().map(|item| item.since)),
            next_action: definition.next_action.to_owned(),
        });
    }
    Ok(result)
}

fn load_page(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    source_limit: u32,
    after: Option<&WorkQueueCursor>,
    principal: &Principal,
) -> Result<WorkQueuePage, WorkQueueError> {
    if matches!(
        definition.code,
        "review_ready" | "review_stale" | "review_blocked"
    ) {
        return review_items::page(conn, definition, limit, after);
    }
    let items = load_items(conn, definition, limit, source_limit, after, principal)?;
    let next = items
        .last()
        .filter(|item| item.has_more == Some(true))
        .map(|item| WorkQueueCursor {
            since: item.since,
            item_kind: item.item_kind.clone(),
            item_id: item.item_id.clone(),
        });
    Ok(WorkQueuePage {
        items,
        next,
        scanned: 0,
    })
}

fn load_items(
    conn: &rusqlite::Connection,
    definition: QueueDefinition,
    limit: u32,
    source_limit: u32,
    after: Option<&WorkQueueCursor>,
    principal: &Principal,
) -> Result<Vec<WorkQueueItem>, WorkQueueError> {
    let query_limit = source_limit;
    let mut items = match definition.code {
        "orders_needing_decision" => {
            owner_items::order_decision_items(conn, definition, query_limit)?
        }
        "orders_on_hold" => owner_items::order_hold_items(conn, definition, query_limit)?,
        "identity_holds" => owner_items::identity_items(conn, definition, query_limit)?,
        "collection_time_review" => {
            owner_items::collection_review_items(conn, definition, query_limit)?
        }
        "specimen_decisions" => owner_items::specimen_items(conn, definition, query_limit)?,
        "label_failures" => owner_items::label_items(conn, definition, query_limit)?,
        "qc_review" => owner_items::qc_items(conn, definition, query_limit)?,
        "open_deviations" => owner_items::deviation_items(conn, definition, query_limit)?,
        "import_quarantine" | "performed_not_ordered" => {
            owner_items::exception_items(conn, definition, query_limit)?
        }
        "ops_alerts"
        | "listening_gaps"
        | "recovery_reconciliation"
        | "version_acceptance"
        | "results_expected" => ops_items::ops_items(conn, definition, query_limit, principal)?,
        _ => reporting_items::reporting_items(conn, definition, query_limit)?,
    };
    // A bounded owner scan can contain several events for one task. Its deduplicated
    // count is still a lower bound when the source scan reached its ceiling.
    let source_capped = source_limit != u32::MAX && items.len() >= source_limit as usize;
    sort_items(&mut items, limit, after);
    if source_capped {
        for item in &mut items {
            item.has_more = Some(true);
        }
    }
    Ok(items)
}

fn sort_items(items: &mut Vec<WorkQueueItem>, limit: u32, after: Option<&WorkQueueCursor>) {
    items.sort_by(|left, right| {
        (left.since, &left.item_kind, &left.item_id).cmp(&(
            right.since,
            &right.item_kind,
            &right.item_id,
        ))
    });
    // Multiple pending owner events can lead to the same record workspace.
    // Keep one navigation task per record, including its earliest outstanding time.
    let mut seen = std::collections::BTreeSet::new();
    items.retain(|item| seen.insert((item.item_kind.clone(), item.item_id.clone())));
    if let Some(cursor) = after {
        items.retain(|item| {
            (item.since, item.item_kind.as_str(), item.item_id.as_str())
                > (
                    cursor.since,
                    cursor.item_kind.as_str(),
                    cursor.item_id.as_str(),
                )
        });
    }
    let limit = limit as usize;
    let has_more = items.len() > limit;
    items.truncate(limit);
    for item in items {
        item.has_more = Some(has_more);
    }
}

fn validation(field: &str, code: &str, message: &str) -> Failure {
    domain_failure(
        ErrorCode::Validation,
        message.to_owned(),
        vec![FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }],
        Vec::new(),
    )
}

#[cfg(test)]
mod tests;
