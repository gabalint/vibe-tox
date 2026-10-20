//! Billing work (REQ-OPS-004, brief §6): subjects and dates of service
//! that need a derivation, held or ready derivations, and handoff items
//! outstanding or rejected, oldest first and bounded. Items carry opaque
//! IDs and accession numbers only, never demographics.

use std::collections::{BTreeMap, BTreeSet};

use lis_core::{FieldError, UtcTimestamp, text_enum};
use lis_identity::SubjectId;
use lis_specimens::SpecimenId;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{BillingError, invalid};
use crate::model::{
    BillingDerivationId, BillingDerivationState, BillingHandoffId, BillingHandoffItemId,
    HandoffStatus,
};
use crate::policy;
use crate::{derivation_repo as repo, derivations::is_stale, inputs::specimens_on};

const LIMIT_MAX: u32 = 500;

text_enum! {
    /// Which billing work to list.
    BillingWorkFilter {
        /// No derivation exists for the group, or its latest digest is stale.
        NeedsDerivation = "needs_derivation",
        /// Held by a blocking flag.
        Held = "held",
        /// Awaiting approval or handoff.
        Ready = "ready",
        /// Handed off without an outcome.
        Outstanding = "outstanding",
        /// Rejected by the billing owner and not yet derived again.
        Rejected = "rejected",
    }
}

text_enum! {
    /// Where one billing work item stands.
    BillingWorkState {
        NeedsDerivation = "needs_derivation",
        Held = "held",
        Candidate = "candidate",
        Approved = "approved",
        Outstanding = "outstanding",
        Rejected = "rejected",
    }
}

/// Input of `list_billing_work`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListBillingWork {
    pub filter: BillingWorkFilter,
    pub limit: u32,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after: Option<BillingWorkCursor>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub target: Option<BillingWorkTarget>,
}
/// Exact patient/date grouping for queue navigation; unknown date remains explicit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingWorkTarget {
    pub subject_id: SubjectId,
    pub date_of_service: Option<String>,
}
/// Stable continuation through equal timestamps and multiple dates for one patient.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingWorkCursor {
    pub since: UtcTimestamp,
    pub subject_id: SubjectId,
    pub date_of_service: Option<String>,
}

/// One subject and date of service with billing work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingWorkItem {
    pub subject_id: SubjectId,
    /// `YYYY-MM-DD`; `null` when the collection date is unknown.
    pub date_of_service: Option<String>,
    pub accession_numbers: Vec<String>,
    pub derivation_id: Option<BillingDerivationId>,
    pub handoff_id: Option<BillingHandoffId>,
    /// Sequential handoff number for reconciliation lists.
    #[ts(type = "number | null")]
    pub handoff_no: Option<i64>,
    pub item_id: Option<BillingHandoffItemId>,
    pub state: BillingWorkState,
    pub since: UtcTimestamp,
    /// The command the state calls for; the command still checks everything.
    pub allowed_actions: Vec<String>,
    /// Another matching row exists beyond the requested limit.
    #[ts(optional)]
    pub has_more: Option<bool>,
}

/// Billing work of one kind, oldest first, at most `limit` (1 to 500).
pub fn list_billing_work(
    conn: &Connection,
    input: ListBillingWork,
) -> Result<Vec<BillingWorkItem>, BillingError> {
    if input.limit == 0 || input.limit > LIMIT_MAX {
        return Err(invalid(FieldError {
            field: "limit".to_owned(),
            code: "out_of_range".to_owned(),
            message: "Ask for between 1 and 500 items.".to_owned(),
        }));
    }
    let items: Vec<_> = work_groups(conn)?
        .into_iter()
        .filter(|item| filter_matches(input.filter, item.state))
        .filter(|item| {
            input.target.as_ref().is_none_or(|t| {
                item.subject_id == t.subject_id && item.date_of_service == t.date_of_service
            })
        })
        .filter(|item| {
            input.after.as_ref().is_none_or(|c| {
                (item.since, item.subject_id, &item.date_of_service)
                    > (c.since, c.subject_id, &c.date_of_service)
            })
        })
        .collect();
    finish(conn, items, input.limit)
}

/// The billing owner's work queue: every kind of billing work, oldest
/// first, at most `limit`. PHI-free; the engine's work queues compose it.
pub fn billing_work_queue(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<BillingWorkItem>, BillingError> {
    finish(conn, work_groups(conn)?, limit)
}

/// One queue row per subject/date group. Only the newest derivation governs,
/// and its digest is rebuilt before any action is offered.
fn work_groups(conn: &Connection) -> Result<Vec<BillingWorkItem>, BillingError> {
    let report_groups = current_final_groups(conn)?;
    let mut derived_groups = BTreeSet::new();
    let mut items = Vec::new();
    for row in repo::latest_by_group(conn)? {
        let key = (row.subject_id, row.date_of_service.clone());
        derived_groups.insert(key.clone());
        if let Some(mut item) = work_from_derivation(conn, &row)? {
            if let Some(released_at) = report_groups.get(&key) {
                item.since = item.since.min(*released_at);
            }
            items.push(item);
        }
    }
    for ((subject_id, date_of_service), since) in report_groups {
        if !derived_groups.contains(&(subject_id, date_of_service.clone())) {
            items.push(work_item(
                subject_id,
                date_of_service,
                None,
                BillingWorkState::NeedsDerivation,
                since,
            ));
        }
    }
    Ok(items)
}

fn work_from_derivation(
    conn: &Connection,
    row: &repo::DerivationRow,
) -> Result<Option<BillingWorkItem>, BillingError> {
    // A corrected collection may leave no work on this date. Only an already
    // handed-off group needs an explicit zero-service adjustment.
    if crate::inputs::specimens_on(conn, row.subject_id, row.date_of_service.as_deref())?.is_empty()
        && repo::latest_handed_off(conn, row.subject_id, row.date_of_service.as_deref())?.is_none()
    {
        return Ok(None);
    }
    if is_stale(conn, row)? {
        return Ok(Some(work_item(
            row.subject_id,
            row.date_of_service.clone(),
            Some(row.id),
            BillingWorkState::NeedsDerivation,
            row.derived_at,
        )));
    }
    let item = match row.state {
        BillingDerivationState::Held => Some(work_item(
            row.subject_id,
            row.date_of_service.clone(),
            Some(row.id),
            BillingWorkState::Held,
            row.derived_at,
        )),
        BillingDerivationState::Candidate => Some(work_item(
            row.subject_id,
            row.date_of_service.clone(),
            Some(row.id),
            BillingWorkState::Candidate,
            row.derived_at,
        )),
        BillingDerivationState::Approved => Some(work_item(
            row.subject_id,
            row.date_of_service.clone(),
            Some(row.id),
            BillingWorkState::Approved,
            row.approved_at.unwrap_or(row.derived_at),
        )),
        BillingDerivationState::HandedOff => handed_off_work(conn, row)?,
        BillingDerivationState::Superseded => Some(work_item(
            row.subject_id,
            row.date_of_service.clone(),
            Some(row.id),
            BillingWorkState::NeedsDerivation,
            row.derived_at,
        )),
    };
    Ok(item)
}

fn handed_off_work(
    conn: &Connection,
    row: &repo::DerivationRow,
) -> Result<Option<BillingWorkItem>, BillingError> {
    let status = conn
        .prepare_cached(
            "SELECT item.handoff_id, handoff.handoff_no, item.id, \
                    CASE WHEN outcome.outcome IS NULL THEN 'outstanding' ELSE outcome.outcome END, \
                    COALESCE(outcome.recorded_at, handoff.created_at) \
             FROM bil_handoff_items item \
             JOIN bil_handoffs handoff ON handoff.id = item.handoff_id \
             LEFT JOIN bil_v_latest_handoff_outcomes outcome ON outcome.item_id = item.id \
             WHERE item.derivation_id = ?1",
        )?
        .query_row([row.id], |sql_row| {
            Ok((
                sql_row.get(0)?,
                sql_row.get(1)?,
                sql_row.get(2)?,
                sql_row.get(3)?,
                sql_row.get(4)?,
            ))
        })
        .optional()?
        .ok_or(BillingError::Inconsistent("handed-off derivation"))?;
    let (handoff_id, handoff_no, item_id, state, since): (_, _, _, HandoffStatus, _) = status;
    let work_state = match state {
        HandoffStatus::Outstanding => BillingWorkState::Outstanding,
        HandoffStatus::Rejected => BillingWorkState::Rejected,
        HandoffStatus::Acknowledged => return Ok(None),
    };
    let mut item = work_item(
        row.subject_id,
        row.date_of_service.clone(),
        Some(row.id),
        work_state,
        since,
    );
    item.handoff_id = Some(handoff_id);
    item.handoff_no = Some(handoff_no);
    item.item_id = Some(item_id);
    Ok(Some(item))
}

/// Current final releases grouped before a queue limit is applied.
fn current_final_groups(
    conn: &Connection,
) -> Result<BTreeMap<(SubjectId, Option<String>), UtcTimestamp>, BillingError> {
    let rows: Vec<(SpecimenId, UtcTimestamp)> = conn
        .prepare_cached(
            "SELECT v.specimen_id, v.released_at FROM rpt_v_revisions v \
             WHERE v.availability = 'current' AND v.completeness = 'final' \
             ORDER BY v.released_at, v.specimen_id",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut groups = BTreeMap::new();
    for (specimen_id, released_at) in rows {
        let accession = lis_specimens::get_accession(
            conn,
            lis_specimens::specimen_accession(conn, specimen_id)?,
        )?;
        let specimen = lis_specimens::specimens_for_subject(conn, accession.subject_id)?
            .into_iter()
            .find(|s| s.specimen_id == specimen_id)
            .ok_or(BillingError::Inconsistent("released specimen"))?;
        let date_of_service = policy::date_of_service(&specimen)?;
        groups
            .entry((accession.subject_id, date_of_service))
            .and_modify(|since: &mut UtcTimestamp| *since = (*since).min(released_at))
            .or_insert(released_at);
    }
    Ok(groups)
}

fn work_item(
    subject_id: SubjectId,
    date_of_service: Option<String>,
    derivation_id: Option<BillingDerivationId>,
    state: BillingWorkState,
    since: UtcTimestamp,
) -> BillingWorkItem {
    BillingWorkItem {
        subject_id,
        date_of_service,
        accession_numbers: Vec::new(),
        derivation_id,
        handoff_id: None,
        handoff_no: None,
        item_id: None,
        state,
        since,
        allowed_actions: allowed_actions(state),
        has_more: Some(false),
    }
}

fn finish(
    conn: &Connection,
    mut items: Vec<BillingWorkItem>,
    limit: u32,
) -> Result<Vec<BillingWorkItem>, BillingError> {
    items.sort_by_key(|item| (item.since, item.subject_id, item.date_of_service.clone()));
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = items.len() > limit;
    items.truncate(limit);
    for item in items.iter_mut() {
        item.accession_numbers =
            accession_numbers(conn, item.subject_id, item.date_of_service.as_deref())?;
        item.has_more = Some(has_more);
    }
    Ok(items)
}

fn filter_matches(filter: BillingWorkFilter, state: BillingWorkState) -> bool {
    match filter {
        BillingWorkFilter::NeedsDerivation => state == BillingWorkState::NeedsDerivation,
        BillingWorkFilter::Held => state == BillingWorkState::Held,
        BillingWorkFilter::Ready => matches!(
            state,
            BillingWorkState::Candidate | BillingWorkState::Approved
        ),
        BillingWorkFilter::Outstanding => state == BillingWorkState::Outstanding,
        BillingWorkFilter::Rejected => state == BillingWorkState::Rejected,
    }
}

/// The next step for each state: derive while nothing usable exists or the
/// billing owner rejected the item, approve a candidate, hand off an
/// approved derivation, and record the outcome of an outstanding item.
fn allowed_actions(state: BillingWorkState) -> Vec<String> {
    let action = match state {
        BillingWorkState::NeedsDerivation | BillingWorkState::Held => "derive_services",
        BillingWorkState::Candidate => "approve_service_derivation",
        BillingWorkState::Approved => "create_billing_handoff",
        BillingWorkState::Outstanding => "record_handoff_outcome",
        BillingWorkState::Rejected => "record_handoff_followup",
    };
    vec![action.to_owned()]
}

fn accession_numbers(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Vec<String>, BillingError> {
    let mut numbers: Vec<String> = specimens_on(conn, subject_id, date_of_service)?
        .into_iter()
        .map(|s| s.accession_number)
        .collect();
    if numbers.is_empty() {
        let mut previous = repo::latest_handed_off(conn, subject_id, date_of_service)?;
        while let Some(id) = previous {
            for specimen in repo::specimens(conn, id)? {
                let accession = lis_specimens::specimen_accession(conn, specimen.specimen_id)?;
                numbers.push(lis_specimens::get_accession(conn, accession)?.accession_number);
            }
            if !numbers.is_empty() {
                break;
            }
            previous = repo::load(conn, id)?.adjustment_of_id;
        }
    }
    numbers.sort();
    numbers.dedup();
    Ok(numbers)
}
