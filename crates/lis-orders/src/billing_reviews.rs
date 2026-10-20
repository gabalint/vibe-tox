//! Append-only financial review actions and a bounded staff follow-up queue.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Phi, Principal};
use lis_identity::require_financial_access;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use crate::billing_context::{financial_actor, validate_source};
use crate::billing_model::{
    BillingReviewRecord, BillingReviewStatus, ListBillingReviewItemsInput,
    UpdateBillingReviewItemInput,
};
use crate::{BillingReviewItemId, OrderError, OrderId};

/// Assigns, resolves or reopens a review without changing clinical authorization.
pub fn update_billing_review_item(
    ctx: &mut CommandContext<'_>,
    input: UpdateBillingReviewItemInput,
) -> Result<BillingReviewRecord, OrderError> {
    ctx.require(require_financial_access(ctx.principal())?)?;
    let actor = financial_actor(ctx)?;
    validate_source("reason", &input.reason)?;
    let owner = lis_core::optional_text("owner", input.owner.as_deref(), 200)
        .map_err(OrderError::Invalid)?;
    if let Some(source) = &input.source_ref {
        validate_source("sourceRef", source)?;
    }
    if input.status == BillingReviewStatus::Resolved
        && (owner.is_none() || input.source_ref.is_none())
    {
        return Err(crate::error::invalid(
            "resolution",
            "evidence_required",
            "Resolution needs an owner and retained evidence reference.",
        ));
    }
    let current = review_item(ctx.tx(), input.item_id)?;
    if current.version != input.expected_version {
        return Err(OrderError::RevisionConflict);
    }
    ctx.tx().execute("INSERT INTO ord_billing_review_actions (item_id, version, status, owner, source_ref, reason, recorded_by, recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![input.item_id, current.version + 1, input.status, owner, input.source_ref.as_ref().map(Phi::expose), input.reason.expose(), actor, ctx.now()])?;
    ctx.tx().execute(
        "UPDATE ord_orders SET version = version + 1, revision = revision + 1 WHERE id = ?1",
        [current.order_id],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "order.update_billing_review",
            Outcome::Succeeded,
        )
        .record("billing_review_item", input.item_id)
        .old_revision(current.version)
        .new_revision(current.version + 1)
        .reason(input.reason.expose().clone()),
    )?;
    review_item(ctx.tx(), input.item_id)
}

/// Effective financial review items for internal derivation and audited order reads.
pub fn billing_review_items(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Vec<BillingReviewRecord>, OrderError> {
    read_items(conn, Some(order_id), None, None, None)
}

/// Authorized, bounded financial queue. Engine audits this sensitive read.
pub fn list_billing_review_items(
    conn: &Connection,
    principal: &Principal,
    input: ListBillingReviewItemsInput,
) -> Result<Vec<BillingReviewRecord>, OrderError> {
    require_financial_access(principal)?;
    if !(1..=100).contains(&input.limit) {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Request between one and 100 review items.",
        ));
    }
    if let Some(id) = input.after_item_id {
        review_item(conn, id)?;
    }
    read_items(
        conn,
        None,
        input.status,
        Some(input.limit),
        input.after_item_id,
    )
}

fn review_item(
    conn: &Connection,
    id: BillingReviewItemId,
) -> Result<BillingReviewRecord, OrderError> {
    let order_id = conn
        .query_row(
            "SELECT order_id FROM ord_billing_review_items WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(OrderError::NotFound("billing_review_item"))?;
    billing_review_items(conn, order_id)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or(OrderError::NotFound("billing_review_item"))
}

fn read_items(
    conn: &Connection,
    order_id: Option<OrderId>,
    status: Option<BillingReviewStatus>,
    limit: Option<u32>,
    after_item_id: Option<BillingReviewItemId>,
) -> Result<Vec<BillingReviewRecord>, OrderError> {
    let mut stmt = conn.prepare_cached("SELECT i.id,i.order_id,i.element,i.note,i.opened_at,i.opened_by,
        coalesce(a.version,1),coalesce(a.status,'open'),a.owner,a.source_ref,a.reason,a.recorded_at,a.recorded_by,o.placer_order_id
        FROM ord_billing_review_items i JOIN ord_orders o ON o.id=i.order_id
        LEFT JOIN ord_billing_review_actions a ON a.item_id=i.id
          AND a.version=(SELECT max(version) FROM ord_billing_review_actions WHERE item_id=i.id)
        WHERE (?1 IS NULL OR i.order_id=?1) AND (?2 IS NULL OR coalesce(a.status,'open')=?2)
        AND (?4 IS NULL OR (i.opened_at,i.id) > (SELECT opened_at,id FROM ord_billing_review_items WHERE id=?4))
        ORDER BY i.opened_at,i.id LIMIT ?3")?;
    let rows = stmt.query_map(
        params![
            order_id,
            status,
            limit.map(i64::from).unwrap_or(-1),
            after_item_id
        ],
        |row| {
            Ok(BillingReviewRecord {
                id: row.get(0)?,
                order_id: row.get(1)?,
                order_reference: row.get::<_, Option<String>>(13)?.map(Phi::new),
                element: row.get(2)?,
                note: row.get::<_, Option<String>>(3)?.map(Phi::new),
                opened_at: row.get(4)?,
                opened_by: row.get(5)?,
                version: row.get(6)?,
                status: row.get(7)?,
                owner: row.get(8)?,
                source_ref: row.get::<_, Option<String>>(9)?.map(Phi::new),
                reason: row.get::<_, Option<String>>(10)?.map(Phi::new),
                updated_at: row.get(11)?,
                updated_by: row.get(12)?,
            })
        },
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(OrderError::from)
}
