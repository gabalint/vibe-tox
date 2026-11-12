//! The order worklist (REQ-ORD-001): bounded pages, newest first, by filter, patient
//! and find text. PHI: the engine authorizes and audits the read.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use lis_catalog::OrderableVersionId;
use lis_core::{Phi, UtcTimestamp};
use lis_identity::{ClientId, SubjectId, SubjectSummary};
use rusqlite::{Connection, params};

use crate::error::OrderError;
use crate::inputs::ListOrdersInput;
use crate::model::{OrderId, OrderStatus};
use crate::queries::ensure_order_exists;
use crate::views::{OrderSummary, OrderedTestRef};

/// Longest find text, in characters.
const SEARCH_MAX: usize = 200;

/// What the find text names in other modules, resolved by the engine through their
/// published queries: patients by name or identifier, clients by name, and orders by
/// accession number or sample barcode. Empty when there is no find text.
#[derive(Debug, Default)]
pub struct OrderMatches {
    pub subject_ids: Vec<SubjectId>,
    pub client_ids: Vec<ClientId>,
    pub order_ids: Vec<OrderId>,
}

/// The trimmed find text of `input`, `None` when absent or blank.
pub fn order_search(input: &ListOrdersInput) -> Result<Option<Phi<String>>, OrderError> {
    let text = input.search.as_ref().map(|text| text.expose().as_str());
    Ok(lis_core::optional_text("search", text, SEARCH_MAX)
        .map_err(OrderError::Invalid)?
        .map(Phi::new))
}

/// Bounded order worklist, newest first, with each order's patient name, placer order
/// number, client name, item count and ordered tests (REQ-ORD-001). With find text, only orders whose
/// placer order number starts with it, ignoring case, or that `matches` names are listed.
/// PHI: the engine audits the read.
pub fn list_orders(
    conn: &Connection,
    input: &ListOrdersInput,
    matches: &OrderMatches,
) -> Result<Vec<OrderSummary>, OrderError> {
    if input.limit == 0 || input.limit > 500 {
        return Err(crate::error::invalid(
            "limit",
            "out_of_range",
            "Use a limit from 1 to 500.",
        ));
    }
    let search = order_search(input)?;
    if let Some(id) = input.before_order_id {
        ensure_order_exists(conn, id)?;
    }
    let rows = conn
        .prepare(&page_sql(input))?
        .query_map(
            params![
                input.subject_id,
                input.before_received_at,
                input.limit,
                input.before_order_id,
                search.as_ref().map(Phi::expose),
                id_array(&matches.order_ids),
                id_array(&matches.subject_ids),
                id_array(&matches.client_ids),
            ],
            |row| {
                Ok(OrderRow {
                    order_id: row.get(0)?,
                    subject_id: row.get(1)?,
                    client_id: row.get(2)?,
                    placer_order_id: row.get(3)?,
                    status: row.get(4)?,
                    received_at: row.get(5)?,
                    version: row.get(6)?,
                    revision: row.get(7)?,
                    open_hold_count: row.get(8)?,
                    item_count: row.get(9)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let order_ids: Vec<OrderId> = rows.iter().map(|row| row.order_id).collect();
    let tests = ordered_tests(conn, &order_ids)?;
    with_names(conn, rows, tests)
}

/// The test of each item of `orders`, in request order, by order: one query for the
/// items, then each ordered version once through the catalog's published query.
fn ordered_tests(
    conn: &Connection,
    orders: &[OrderId],
) -> Result<BTreeMap<OrderId, Vec<OrderedTestRef>>, OrderError> {
    let items = conn
        .prepare_cached(
            "SELECT order_id, orderable_version_id FROM ord_order_items \
             WHERE order_id IN (SELECT value FROM json_each(?1)) \
             ORDER BY order_id, requested_at, id",
        )?
        .query_map([id_array(orders)], |row| {
            Ok((
                row.get::<_, OrderId>(0)?,
                row.get::<_, OrderableVersionId>(1)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut versions: BTreeMap<OrderableVersionId, OrderedTestRef> = BTreeMap::new();
    let mut tests: BTreeMap<OrderId, Vec<OrderedTestRef>> = BTreeMap::new();
    for (order_id, version_id) in items {
        let test = match versions.entry(version_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let version = lis_catalog::get_orderable_version(conn, version_id)?;
                entry.insert(OrderedTestRef {
                    code: version.code,
                    name: version.name,
                })
            }
        };
        tests.entry(order_id).or_default().push(test.clone());
    }
    Ok(tests)
}

/// A JSON array of IDs for `json_each`; hyphenated UUIDs need no escaping.
fn id_array<T: std::fmt::Display>(ids: &[T]) -> String {
    let quoted: Vec<_> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    format!("[{}]", quoted.join(","))
}

/// The page query: filter, patient, continuation and find text, then the per-order counts.
fn page_sql(input: &ListOrdersInput) -> String {
    let filter = match input.filter {
        crate::OrderFilter::All => "1",
        crate::OrderFilter::NeedsDecision => "o.status = 'received'",
        crate::OrderFilter::OnHold => {
            "EXISTS (SELECT 1 FROM ord_clarification_holds h \
            WHERE h.order_id = o.id AND h.state = 'open')"
        }
        crate::OrderFilter::Accepted => "o.status = 'accepted'",
    };
    let subject = if input.subject_id.is_some() {
        "o.subject_id = ?1"
    } else {
        "1"
    };
    let before = "(?4 IS NULL OR (o.received_at, o.id) < (SELECT received_at, id FROM ord_orders WHERE id=?4)) AND (?4 IS NOT NULL OR ?2 IS NULL OR o.received_at < ?2)";
    // substr rather than LIKE keeps %, _ and escape characters literal.
    let found = "(?5 IS NULL OR lower(substr(o.placer_order_id, 1, length(?5))) = lower(?5) \
         OR o.id IN (SELECT value FROM json_each(?6)) \
         OR o.subject_id IN (SELECT value FROM json_each(?7)) \
         OR o.client_id IN (SELECT value FROM json_each(?8)))";
    format!(
        "WITH page AS MATERIALIZED (SELECT o.id, o.subject_id, o.client_id, o.placer_order_id, \
         o.status, o.received_at, o.version, o.revision FROM ord_orders o \
         WHERE {filter} AND {subject} AND {before} AND {found} \
         ORDER BY o.received_at DESC, o.id DESC LIMIT ?3) \
         SELECT o.id, o.subject_id, o.client_id, o.placer_order_id, o.status, o.received_at, \
         o.version, o.revision, \
         (SELECT count(*) FROM ord_clarification_holds h WHERE h.order_id = o.id AND h.state = 'open'), \
         (SELECT count(*) FROM ord_order_items i WHERE i.order_id = o.id) \
         FROM page o ORDER BY o.received_at DESC, o.id DESC"
    )
}
/// One row of the order worklist query, before the names are read.
struct OrderRow {
    order_id: OrderId,
    subject_id: SubjectId,
    client_id: ClientId,
    placer_order_id: Option<String>,
    status: OrderStatus,
    received_at: UtcTimestamp,
    version: i64,
    revision: i64,
    open_hold_count: i64,
    item_count: i64,
}

/// Adds the patient and client names, read through the identity module's published
/// queries (never from its tables), once per distinct patient and client, and the
/// ordered tests.
fn with_names(
    conn: &Connection,
    rows: Vec<OrderRow>,
    mut tests: BTreeMap<OrderId, Vec<OrderedTestRef>>,
) -> Result<Vec<OrderSummary>, OrderError> {
    let mut subjects: BTreeMap<SubjectId, SubjectSummary> = BTreeMap::new();
    let mut clients: BTreeMap<ClientId, String> = BTreeMap::new();
    rows.into_iter()
        .map(|row| {
            let subject = match subjects.entry(row.subject_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    entry.insert(lis_identity::subject_summary(conn, row.subject_id)?)
                }
            };
            let client_name = match clients.entry(row.client_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    entry.insert(lis_identity::get_client(conn, row.client_id)?.name)
                }
            };
            Ok(OrderSummary {
                order_id: row.order_id,
                subject_id: row.subject_id,
                patient_family_name: subject.family_name.clone(),
                patient_given_names: subject.given_names.clone(),
                patient_birth_date: subject.birth_date.clone(),
                patient_identifiers: subject.identifiers.clone(),
                placer_order_id: row.placer_order_id,
                client_name: client_name.clone(),
                sample_barcodes: Vec::new(),
                accessions: Vec::new(),
                item_count: row.item_count,
                ordered_tests: tests.remove(&row.order_id).unwrap_or_default(),
                status: row.status,
                received_at: row.received_at,
                version: row.version,
                revision: row.revision,
                open_hold_count: row.open_hold_count,
            })
        })
        .collect()
}
