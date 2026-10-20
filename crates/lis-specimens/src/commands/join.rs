//! Joining an accepted add-on item to the accession whose material was
//! already measured (result files brief F17, §13; REQ-SPC-001).

use lis_orders::OrderItemId;
use lis_store::CommandContext;

use crate::error::{SpecimenError, invalid};
use crate::model::AccessionId;
use crate::queries;
use crate::repo::accessions as repo;

/// Links the accepted order item `order_item_id` to the accession
/// `accession_id` (append-only `spc_accession_items`), so the material the
/// accession already holds can serve it. Refused for an item of another
/// order, an item that is not accepted (or whose order is on hold), an item
/// already accessioned, and an item whose specimen type the accession has no
/// specimen of. The gates are read inside the caller's transaction.
///
/// It checks no capability and appends no audit event: the caller's command
/// (`use_measured_result`) does both in the same transaction.
pub fn join_accession(
    ctx: &mut CommandContext<'_>,
    accession_id: AccessionId,
    order_item_id: OrderItemId,
) -> Result<(), SpecimenError> {
    let accession = queries::get_accession(ctx.tx(), accession_id)?;
    let ready = lis_orders::items_ready_for_accession(ctx.tx(), accession.order_id)?;
    let matrices: Vec<&str> = ready
        .iter()
        .filter(|item| item.item_id == order_item_id)
        .map(|item| item.specimen_type_code.as_str())
        .collect();
    if matrices.is_empty() {
        let order = lis_orders::get_order(ctx.tx(), accession.order_id)?;
        if order.items.iter().any(|item| item.item_id == order_item_id) {
            return Err(SpecimenError::OrderItemNotReady);
        }
        return Err(invalid(
            "orderItemId",
            "other_order",
            "The test belongs to another order.",
        ));
    }
    if repo::item_accessioned(ctx.tx(), order_item_id)? {
        return Err(SpecimenError::ItemAccessioned);
    }
    let held = |matrix: &&str| {
        accession
            .specimens
            .iter()
            .any(|specimen| specimen.specimen_type_code == **matrix)
    };
    if !matrices.iter().all(held) {
        return Err(invalid(
            "orderItemId",
            "matrix_mismatch",
            "The accession has no specimen of the type this test requires.",
        ));
    }
    repo::insert_accession_item(ctx.tx(), accession_id, order_item_id)?;
    Ok(())
}
