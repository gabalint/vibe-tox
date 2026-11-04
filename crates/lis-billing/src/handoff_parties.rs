//! Who and what a handoff covers, for routing its file to outside systems:
//! the ordering clients, practitioners, client locations and ordered tests of
//! every specimen in it, by code and display name. No patient data.

use std::collections::BTreeSet;

use lis_orders::OrderParties;
use rusqlite::Connection;

use crate::BillingHandoffId;
use crate::derivation_repo as repo;
use crate::derivation_view::specimen_views;
use crate::error::BillingError;

/// The parties of handoff `handoff_id`, read in the caller's transaction.
pub fn handoff_parties(
    conn: &Connection,
    handoff_id: BillingHandoffId,
) -> Result<OrderParties, BillingError> {
    let derivations: Vec<crate::BillingDerivationId> = conn
        .prepare_cached("SELECT derivation_id FROM bil_handoff_items WHERE handoff_id = ?1")?
        .query_map([handoff_id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut orders = BTreeSet::new();
    for derivation_id in derivations {
        let row = repo::load(conn, derivation_id)?;
        orders.extend(
            specimen_views(conn, &row)?
                .into_iter()
                .map(|specimen| specimen.order_id),
        );
    }
    let mut parties = OrderParties::default();
    for order_id in orders {
        parties.add_order(conn, order_id, None)?;
    }
    Ok(parties)
}
