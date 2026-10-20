//! Financial readiness affects handoff only, never clinical scope (BILL-G02/G06).

use lis_orders::{BillingReviewStatus, OrderBillingContext, OrderId};
use rusqlite::{Connection, OptionalExtension};
use std::collections::BTreeSet;

use crate::derivation::{InputRef, Inputs};
use crate::{BillingError, BillingFlag, BillingInputKind};

pub(crate) fn gather(conn: &Connection, inputs: &mut Inputs) -> Result<(), BillingError> {
    let orders: BTreeSet<OrderId> = inputs
        .specimens
        .iter()
        .map(|specimen| specimen.order_id)
        .collect();
    let mut destinations = BTreeSet::new();
    for order in orders {
        let context = lis_orders::get_order_billing_context(conn, order)?;
        let issues = lis_orders::billing_context_readiness(context.as_ref());
        if !issues.is_empty() {
            flag(
                inputs,
                if context.is_none() {
                    "billing_context_missing"
                } else {
                    "billing_context_incomplete"
                },
                order.to_string(),
            );
        }
        if let Some(context) = context {
            inputs.input_refs.push(InputRef {
                kind: BillingInputKind::BillingContext,
                ref_id: format!("{}:{}", order, context.version),
            });
            destinations.insert(destination_key(&context)?);
            if outside_coverage_dates(&context, inputs.date_of_service.as_deref()) {
                flag(inputs, "billing_context_incomplete", order.to_string());
            }
        }
        for review in lis_orders::billing_review_items(conn, order)? {
            inputs.input_refs.push(InputRef {
                kind: BillingInputKind::BillingReview,
                ref_id: format!("{}:{}", review.id, review.version),
            });
            if review.status == BillingReviewStatus::Open {
                flag(inputs, "billing_review_open", review.id.to_string());
            }
        }
    }
    if destinations.len() > 1 {
        flag(inputs, "billing_context_conflict", String::new());
    }
    apply_resolution(conn, inputs)
}

fn outside_coverage_dates(context: &OrderBillingContext, date: Option<&str>) -> bool {
    let Some(date) = date else {
        return false;
    };
    context.selection.coverages.iter().any(|coverage| {
        coverage
            .effective_from
            .as_deref()
            .is_some_and(|from| date < from)
            || coverage
                .effective_until
                .as_deref()
                .is_some_and(|until| date > until)
    })
}

/// Comparison excludes profile/source metadata and addresses. It retains payer sequence,
/// member identity and the subscriber relationship; different clinical orderers alone do not conflict.
fn destination_key(context: &OrderBillingContext) -> Result<String, BillingError> {
    let coverages: Vec<_> = context
        .selection
        .coverages
        .iter()
        .map(|coverage| {
            let subscriber = coverage
                .subscriber
                .as_ref()
                .map(|party| (&party.name, &party.birth_date, party.relationship));
            (
                &coverage.payer_name,
                &coverage.payer_id,
                &coverage.plan_name,
                &coverage.member_id,
                &coverage.group_number,
                subscriber,
            )
        })
        .collect();
    let guarantor = context
        .selection
        .guarantor
        .as_ref()
        .map(|party| (&party.name, &party.birth_date, party.relationship));
    serde_json::to_string(&(
        context.selection.responsibility,
        context.selection.client_id,
        coverages,
        guarantor,
    ))
    .map_err(|_| BillingError::Inconsistent("financial context"))
}

fn flag(inputs: &mut Inputs, code: &str, ref_id: String) {
    inputs.billing_flags.push(BillingFlag {
        code: code.to_owned(),
        ref_id,
        blocking: true,
    });
}

fn apply_resolution(conn: &Connection, inputs: &mut Inputs) -> Result<(), BillingError> {
    if !inputs
        .billing_flags
        .iter()
        .any(|flag| flag.code == "billing_context_conflict")
    {
        return Ok(());
    }
    let unresolved = crate::derive(inputs);
    let resolution: Option<String> = conn
        .query_row(
            "SELECT id FROM bil_context_resolutions WHERE inputs_sha256 = ?1",
            [unresolved.inputs_sha256],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = resolution {
        for flag in &mut inputs.billing_flags {
            if flag.code == "billing_context_conflict" {
                flag.blocking = false;
            }
        }
        inputs.input_refs.push(InputRef {
            kind: BillingInputKind::ConflictResolution,
            ref_id: id,
        });
    }
    Ok(())
}
