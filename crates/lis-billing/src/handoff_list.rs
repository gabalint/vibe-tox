//! Past billing handoffs (REQ-BIL-002): every handoff, newest first and
//! bounded, so one stays reachable after each of its items is
//! acknowledged and it leaves billing work. Rows carry opaque IDs,
//! accession numbers and dates of service only, never demographics.

use lis_core::{FieldError, UtcTimestamp};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::derivation_repo as repo;
use crate::error::{BillingError, invalid};
use crate::model::{BillingDerivationId, BillingHandoffId};

const LIMIT_MAX: u32 = 500;

/// Input of `list_billing_handoffs`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListBillingHandoffs {
    pub limit: u32,
    /// A handoff number, or the start of an accession number or of a
    /// `YYYY-MM-DD` date of service, ignoring case.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub search: Option<String>,
    /// Continue after the handoff with this number.
    #[serde(default)]
    #[ts(optional = nullable)]
    #[ts(type = "number | null")]
    pub before_handoff_no: Option<i64>,
}

/// One handoff and where its items stand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingHandoffSummary {
    pub handoff_id: BillingHandoffId,
    #[ts(type = "number")]
    pub handoff_no: i64,
    pub created_at: UtcTimestamp,
    #[ts(type = "number")]
    pub item_count: i64,
    #[ts(type = "number")]
    pub outstanding_count: i64,
    #[ts(type = "number")]
    pub rejected_count: i64,
    #[ts(type = "number")]
    pub acknowledged_count: i64,
    pub accession_numbers: Vec<String>,
    /// `YYYY-MM-DD`; `null` for a collection date that is unknown.
    pub dates_of_service: Vec<Option<String>>,
    /// Another matching handoff exists beyond the requested limit.
    pub has_more: bool,
}

/// Handoffs matching `search`, newest first, at most `limit` (1 to 500).
pub fn list_billing_handoffs(
    conn: &Connection,
    input: ListBillingHandoffs,
) -> Result<Vec<BillingHandoffSummary>, BillingError> {
    if input.limit == 0 || input.limit > LIMIT_MAX {
        return Err(invalid(FieldError {
            field: "limit".to_owned(),
            code: "out_of_range".to_owned(),
            message: "Ask for between 1 and 500 handoffs.".to_owned(),
        }));
    }
    let search = input
        .search
        .as_deref()
        .map(|text| text.trim().to_lowercase())
        .filter(|text| !text.is_empty());
    let limit = usize::try_from(input.limit).unwrap_or(usize::MAX);
    let mut statement = conn.prepare_cached(
        "SELECT handoff.id, handoff.handoff_no, handoff.created_at, handoff.item_count, \
                sum(status.status = 'outstanding'), sum(status.status = 'rejected'), \
                sum(status.status = 'acknowledged') \
         FROM bil_handoffs handoff \
         JOIN bil_v_handoff_status status ON status.handoff_id = handoff.id \
         WHERE ?1 IS NULL OR handoff.handoff_no < ?1 \
         GROUP BY handoff.id ORDER BY handoff.handoff_no DESC",
    )?;
    let mut rows = statement.query([input.before_handoff_no])?;
    let mut handoffs = Vec::new();
    // Matching needs each handoff's accessions, so stop at one past the limit.
    while handoffs.len() <= limit {
        let Some(row) = rows.next()? else { break };
        let mut handoff = BillingHandoffSummary {
            handoff_id: row.get(0)?,
            handoff_no: row.get(1)?,
            created_at: row.get(2)?,
            item_count: row.get(3)?,
            outstanding_count: row.get(4)?,
            rejected_count: row.get(5)?,
            acknowledged_count: row.get(6)?,
            accession_numbers: Vec::new(),
            dates_of_service: Vec::new(),
            has_more: false,
        };
        add_specimen_groups(conn, &mut handoff)?;
        if search.as_deref().is_none_or(|text| matches(&handoff, text)) {
            handoffs.push(handoff);
        }
    }
    let has_more = handoffs.len() > limit;
    handoffs.truncate(limit);
    for handoff in &mut handoffs {
        handoff.has_more = has_more;
    }
    Ok(handoffs)
}

fn matches(handoff: &BillingHandoffSummary, text: &str) -> bool {
    handoff.handoff_no.to_string() == text
        || handoff
            .accession_numbers
            .iter()
            .any(|number| number.to_lowercase().starts_with(text))
        || handoff
            .dates_of_service
            .iter()
            .flatten()
            .any(|date| date.starts_with(text))
}

/// The accession numbers and dates of service the handoff's items cover.
fn add_specimen_groups(
    conn: &Connection,
    handoff: &mut BillingHandoffSummary,
) -> Result<(), BillingError> {
    let items: Vec<(BillingDerivationId, Option<String>)> = conn
        .prepare_cached(
            "SELECT d.id, d.date_of_service FROM bil_handoff_items item \
             JOIN bil_derivations d ON d.id = item.derivation_id \
             WHERE item.handoff_id = ?1",
        )?
        .query_map([handoff.handoff_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (derivation_id, date_of_service) in items {
        handoff
            .accession_numbers
            .extend(derivation_accessions(conn, derivation_id)?);
        handoff.dates_of_service.push(date_of_service);
    }
    handoff.accession_numbers.sort();
    handoff.accession_numbers.dedup();
    handoff.dates_of_service.sort();
    handoff.dates_of_service.dedup();
    Ok(())
}

/// The accession numbers of a derivation's specimens. An adjustment that
/// removed every specimen names those of the derivation it adjusts.
pub(crate) fn derivation_accessions(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<String>, BillingError> {
    let mut numbers = Vec::new();
    let mut next = Some(id);
    while let Some(id) = next {
        for specimen in repo::specimens(conn, id)? {
            let accession = lis_specimens::specimen_accession(conn, specimen.specimen_id)?;
            numbers.push(lis_specimens::get_accession(conn, accession)?.accession_number);
        }
        if !numbers.is_empty() {
            break;
        }
        next = repo::load(conn, id)?.adjustment_of_id;
    }
    Ok(numbers)
}
