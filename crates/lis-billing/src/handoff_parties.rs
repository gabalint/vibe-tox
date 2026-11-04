//! Who and what a handoff covers, for routing its file to outside systems:
//! the ordering clients, practitioners, client locations and ordered tests of
//! every specimen in it, by code and display name. No patient data.

use std::collections::BTreeSet;

use rusqlite::Connection;

use crate::BillingHandoffId;
use crate::derivation_repo as repo;
use crate::derivation_view::specimen_views;
use crate::error::BillingError;

/// One value with its display label, as `(code, label)`.
pub type Party = (String, String);

/// The distinct values of a handoff, each sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandoffParties {
    /// Ordering clients, by client code.
    pub clients: Vec<Party>,
    /// Ordering practitioners, by NPI.
    pub practitioners: Vec<Party>,
    /// Client locations, by location code.
    pub locations: Vec<Party>,
    /// Ordered tests and panels, by orderable code.
    pub tests: Vec<Party>,
}

/// The parties of handoff `handoff_id`, read in the caller's transaction.
pub fn handoff_parties(
    conn: &Connection,
    handoff_id: BillingHandoffId,
) -> Result<HandoffParties, BillingError> {
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
    let mut parties = [(); 4].map(|()| BTreeSet::new());
    let [clients, practitioners, locations, tests] = &mut parties;
    for order_id in orders {
        let order = lis_orders::get_order(conn, order_id)?;
        let client = lis_identity::get_client(conn, order.client_id)?;
        if let Some(location) = client
            .locations
            .iter()
            .find(|location| Some(location.id) == order.client_location_id)
        {
            locations.insert((location.code.clone(), location.name.clone()));
        }
        clients.insert((client.code, client.name));
        let practitioner = lis_identity::get_practitioner(conn, order.practitioner_id)?;
        let name = match practitioner.given_names {
            Some(given) => format!("{}, {given}", practitioner.family_name),
            None => practitioner.family_name,
        };
        practitioners.insert((practitioner.npi.as_str().to_owned(), name));
        for item in order.items {
            let orderable = lis_catalog::get_orderable_version(conn, item.orderable_version_id)?;
            tests.insert((orderable.code, orderable.name));
        }
    }
    let [clients, practitioners, locations, tests] = parties.map(|set| set.into_iter().collect());
    Ok(HandoffParties {
        clients,
        practitioners,
        locations,
        tests,
    })
}
