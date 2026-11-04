//! Who and what orders cover, for routing messages about them to outside
//! systems: the ordering clients, practitioners, client locations and ordered
//! tests, by code and display name. No patient data.

use std::collections::BTreeSet;

use rusqlite::Connection;

use crate::error::OrderError;
use crate::model::{OrderId, OrderItemId};
use crate::queries::get_order;

/// One value with its display label, as `(code, label)`.
pub type Party = (String, String);

/// The distinct values of one or more orders, each sorted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderParties {
    /// Ordering clients, by client code.
    pub clients: BTreeSet<Party>,
    /// Ordering practitioners, by NPI.
    pub practitioners: BTreeSet<Party>,
    /// Client locations, by location code.
    pub locations: BTreeSet<Party>,
    /// Ordered tests and panels, by orderable code.
    pub tests: BTreeSet<Party>,
}

impl OrderParties {
    /// Adds order `order_id`'s values, read in the caller's transaction. With
    /// `items`, only those items' tests count, such as the ones on one specimen.
    pub fn add_order(
        &mut self,
        conn: &Connection,
        order_id: OrderId,
        items: Option<&[OrderItemId]>,
    ) -> Result<(), OrderError> {
        let order = get_order(conn, order_id)?;
        let client = lis_identity::get_client(conn, order.client_id)?;
        if let Some(location) = client
            .locations
            .iter()
            .find(|location| Some(location.id) == order.client_location_id)
        {
            self.locations
                .insert((location.code.clone(), location.name.clone()));
        }
        self.clients.insert((client.code, client.name));
        let practitioner = lis_identity::get_practitioner(conn, order.practitioner_id)?;
        let name = match practitioner.given_names {
            Some(given) => format!("{}, {given}", practitioner.family_name),
            None => practitioner.family_name,
        };
        self.practitioners
            .insert((practitioner.npi.as_str().to_owned(), name));
        for item in order.items {
            if items.is_none_or(|items| items.contains(&item.item_id)) {
                self.tests
                    .insert((item.orderable_code, item.orderable_name));
            }
        }
        Ok(())
    }
}
