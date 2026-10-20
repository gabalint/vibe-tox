//! An order's accession membership and remaining receipt work (REQ-SPC-001/002).

use lis_core::UtcTimestamp;
use lis_orders::{OrderId, OrderItemId};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::{AccessionId, SpecimenError, repo};

/// A consistent snapshot of the material recorded for one order.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderAccessionContext {
    /// The order whose items and accessions are shown.
    pub order_id: OrderId,
    /// Every item, including items that cannot currently be accessioned.
    pub items: Vec<OrderAccessionItem>,
    /// Existing accessions, oldest first, without loading their full records.
    pub accessions: Vec<OrderAccessionSummary>,
    /// State-allowed commands; the engine also filters by the caller's capability.
    pub allowed_actions: Vec<String>,
}

/// An ordered item's eligibility and existing material association.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderAccessionItem {
    /// The order item; matches `OrderItemView.itemId`.
    pub order_item_id: OrderItemId,
    /// Whether the order-side gates pass and no accession already fulfils it.
    pub eligible: bool,
    /// A user-facing explanation when the item cannot be accessioned.
    pub reason: Option<String>,
    /// Its existing accession, even when the order is subsequently held or cancelled.
    pub accession_id: Option<AccessionId>,
    /// The existing accession's lab number.
    pub accession_number: Option<String>,
    /// Required matrices from active frozen components, empty before acceptance.
    pub required_specimen_type_codes: Vec<String>,
}

/// Receipt progress of one accession belonging to the order.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrderAccessionSummary {
    /// The linked accession record.
    pub accession_id: AccessionId,
    /// Its laboratory number.
    pub accession_number: String,
    /// When the laboratory recorded it.
    pub created_at: UtcTimestamp,
    /// Number of specimens, not containers or ordered tests.
    #[ts(type = "number")]
    pub specimen_count: i64,
    /// Specimens still received or held, requiring a receipt decision.
    #[ts(type = "number")]
    pub pending_receipt_count: i64,
}

/// Reads eligibility, membership and progress on the caller's snapshot (REQ-SPC-001/002).
/// The engine authorizes and audits this sensitive read before returning it.
pub fn get_order_accession_context(
    conn: &Connection,
    order_id: OrderId,
) -> Result<OrderAccessionContext, SpecimenError> {
    let readiness = lis_orders::accession_item_readiness(conn, order_id)?;
    let mut memberships = repo::order_accessions::memberships(conn, order_id)?;
    let items: Vec<_> = readiness
        .into_iter()
        .map(|item| {
            let membership = memberships.remove(&item.item_id);
            let reason = item.reason.or_else(|| {
                membership
                    .as_ref()
                    .map(|_| "The ordered test already has an accession.")
            });
            let (accession_id, accession_number) = match membership {
                Some((id, number)) => (Some(id), Some(number)),
                None => (None, None),
            };
            OrderAccessionItem {
                order_item_id: item.item_id,
                eligible: reason.is_none(),
                reason: reason.map(str::to_owned),
                accession_id,
                accession_number,
                required_specimen_type_codes: item.required_specimen_type_codes,
            }
        })
        .collect();
    let allowed_actions = if items.iter().any(|item| item.eligible) {
        vec!["create_accession".to_owned()]
    } else {
        Vec::new()
    };
    Ok(OrderAccessionContext {
        order_id,
        items,
        accessions: repo::order_accessions::summaries(conn, order_id)?,
        allowed_actions,
    })
}
