//! Delivery destinations as configuration staff see them: their state,
//! lists and the queue of destinations to verify or replace (REQ-RPT-012,
//! REQ-OPS-004). Configuration, no PHI.

use lis_core::UtcTimestamp;
use lis_identity::ClientId;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ReportingError;
use crate::model::DestinationId;
use crate::queries::candidate::user_view;
use crate::repo::delivery::{self, DestinationRow};
use crate::views::{DestinationVerificationView, DestinationView};

lis_core::text_enum! {
    /// A destination's verification state.
    #[ts(export)]
    DestinationState {
        /// Not yet verified.
        Unverified = "unverified",
        /// Verified and active.
        Verified = "verified",
        /// An attempt found the address wrong; replace or retire it.
        ReportedWrong = "reported_wrong",
        /// Retired.
        Retired = "retired",
    }
}

/// Input of `list_delivery_destinations`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListDeliveryDestinationsInput {
    /// Only this client's destinations.
    #[serde(default)]
    #[ts(optional)]
    pub client_id: Option<ClientId>,
    /// Only destinations in this state.
    #[serde(default)]
    #[ts(optional)]
    pub state: Option<DestinationState>,
}

/// Destinations, by client and label. Configuration, no PHI.
pub fn list_destinations(
    conn: &Connection,
    input: ListDeliveryDestinationsInput,
) -> Result<Vec<DestinationView>, ReportingError> {
    let mut views = Vec::new();
    for row in delivery::destinations(conn, input.client_id)? {
        if input
            .state
            .is_none_or(|state| state == destination_state(&row))
        {
            views.push(destination_view(conn, &row)?);
        }
    }
    Ok(views)
}

/// An active destination that needs configuration staff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DestinationQueueItem {
    pub destination_id: DestinationId,
    pub label: String,
    /// `unverified` or `reported_wrong`.
    pub state: DestinationState,
    /// Creation, or the first attempt that found the address wrong.
    pub since: UtcTimestamp,
}

/// Active destinations awaiting verification or reported wrong, oldest
/// first (REQ-RPT-012, REQ-OPS-004). Configuration, no PHI.
pub fn destinations_needing_attention(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<DestinationQueueItem>, ReportingError> {
    let mut items: Vec<DestinationQueueItem> = delivery::destinations(conn, None)?
        .into_iter()
        .filter_map(|row| {
            let state = destination_state(&row);
            let since = match state {
                DestinationState::Unverified => row.created_at,
                DestinationState::ReportedWrong => row.reported_wrong_at?,
                DestinationState::Verified | DestinationState::Retired => return None,
            };
            Some(DestinationQueueItem {
                destination_id: row.id,
                label: row.label,
                state,
                since,
            })
        })
        .collect();
    items.sort_by_key(|item| (item.since, item.destination_id));
    items.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    Ok(items)
}

fn destination_state(row: &DestinationRow) -> DestinationState {
    match (row.retired_at, row.reported_wrong_at, &row.verification) {
        (Some(_), _, _) => DestinationState::Retired,
        (None, Some(_), _) => DestinationState::ReportedWrong,
        (None, None, Some(_)) => DestinationState::Verified,
        (None, None, None) => DestinationState::Unverified,
    }
}

pub(crate) fn destination_view(
    conn: &Connection,
    row: &DestinationRow,
) -> Result<DestinationView, ReportingError> {
    let state = destination_state(row);
    let allowed_actions = match state {
        DestinationState::Unverified => vec![
            "verify_delivery_destination",
            "replace_delivery_destination",
            "retire_delivery_destination",
        ],
        DestinationState::Verified | DestinationState::ReportedWrong => {
            vec![
                "replace_delivery_destination",
                "retire_delivery_destination",
            ]
        }
        DestinationState::Retired => Vec::new(),
    };
    Ok(DestinationView {
        destination_id: row.id,
        client_id: row.client_id,
        client_location_id: row.client_location_id,
        channel: row.channel,
        address: row.address.clone(),
        label: row.label.clone(),
        verified: row
            .verification
            .as_ref()
            .map(|v| -> Result<_, ReportingError> {
                Ok(DestinationVerificationView {
                    by: user_view(conn, v.verified_by)?,
                    at: v.verified_at,
                    method: v.method.clone(),
                    by_creator: v.verified_by == row.created_by,
                })
            })
            .transpose()?,
        state,
        retired: row.retired_at.is_some(),
        retire_reason: row.retire_reason.clone(),
        replaces_id: row.replaces_id,
        created_by: user_view(conn, row.created_by)?,
        revision: row.revision,
        allowed_actions: allowed_actions.into_iter().map(str::to_owned).collect(),
    })
}
