//! Financial completeness signals; no result, testing or report gates.

use lis_core::Principal;
use lis_identity::require_financial_access;
use rusqlite::{Connection, OptionalExtension};

use crate::billing_model::{OrderBillingContext, OrderBillingView};
use crate::{BillingResponsibility, OrderError, OrderId};

/// Missing financial capture fields for handoff review, never clinical decisions.
#[must_use]
pub fn billing_context_readiness(context: Option<&OrderBillingContext>) -> Vec<String> {
    let Some(context) = context else {
        return vec!["financial_context_missing".to_owned()];
    };
    let selection = &context.selection;
    let mut issues = Vec::new();
    match selection.responsibility {
        BillingResponsibility::NotSupplied => issues.push("billing_responsibility_missing"),
        BillingResponsibility::Client => {
            if selection.client_id.is_none() {
                issues.push("bill_to_client_missing");
            }
        }
        BillingResponsibility::ThirdParty => {
            if selection.coverages.is_empty() {
                issues.push("coverage_missing");
            }
            if selection
                .coverages
                .iter()
                .any(|coverage| coverage.member_id.is_none())
            {
                issues.push("member_id_missing");
            }
            if selection
                .coverages
                .iter()
                .any(|coverage| coverage.subscriber.is_none())
            {
                issues.push("subscriber_missing");
            }
        }
        BillingResponsibility::PatientSelfPay => {}
    }
    if matches!(
        selection.responsibility,
        BillingResponsibility::ThirdParty | BillingResponsibility::PatientSelfPay
    ) && !selection.contact.as_ref().is_some_and(|contact| {
        !contact.address_lines.is_empty() && contact.city.is_some() && contact.postal_code.is_some()
    }) {
        issues.push("billing_address_missing");
    }
    issues.into_iter().map(str::to_owned).collect()
}

/// Authorized order financial read model for ordering, accessioning and billing.
/// The engine must record sensitive access before returning the model.
pub fn order_billing_view(
    conn: &Connection,
    principal: &Principal,
    order_id: OrderId,
) -> Result<OrderBillingView, OrderError> {
    require_financial_access(principal)?;
    let (order_revision, subject_id) = conn
        .query_row(
            "SELECT revision, subject_id FROM ord_orders WHERE id = ?1",
            [order_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    let subject = lis_identity::subject_summary(conn, subject_id)?;
    let context = crate::get_order_billing_context(conn, order_id)?;
    let mut readiness_issues = billing_context_readiness(context.as_ref());
    let review_items = crate::billing_review_items(conn, order_id)?;
    if review_items
        .iter()
        .any(|item| item.status == crate::BillingReviewStatus::Open)
    {
        readiness_issues.push("billing_review_open".to_owned());
    }
    Ok(OrderBillingView {
        order_id,
        subject,
        order_revision,
        context,
        readiness_issues,
        review_items,
        allowed_actions: vec![
            "set_order_billing_context".to_owned(),
            "create_billing_review_item".to_owned(),
            "update_billing_review_item".to_owned(),
        ],
    })
}
