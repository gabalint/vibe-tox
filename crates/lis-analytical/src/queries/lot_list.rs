//! Lots with expiry derived at read time (REQ-ANA-007), and the active lots that expire within
//! the laboratory's warning period (#4557). The period is an operations setting; the engine
//! reads it and passes it in.

use lis_core::UtcTimestamp;
use rusqlite::Connection;

use crate::error::AnalyticalError;
use crate::model::{CivilDate, LotId, LotState, is_expired};
use crate::repo::lots::{self, LotRow};
use crate::views::LotView;

/// An active lot expiring soon, with the time it entered the warning period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiringLot {
    /// The lot.
    pub lot: LotView,
    /// The start (UTC) of the warning period, or the lot's recording if later.
    pub since: UtcTimestamp,
}

/// Every lot, or the active ones, with expiry derived at `now`. With `warn_days`, an active
/// lot that expires within that many days is marked `expiringSoon`.
pub fn list_lots(
    conn: &Connection,
    active_only: bool,
    now: UtcTimestamp,
    warn_days: Option<u32>,
) -> Result<Vec<LotView>, AnalyticalError> {
    lots::lots(conn, active_only)?
        .into_iter()
        .map(|row| lot_view_of(row, now, warn_days))
        .collect()
}

/// Active lots that expire within `warn_days` of `now`, unordered; expired lots are excluded.
pub fn lots_expiring_soon(
    conn: &Connection,
    now: UtcTimestamp,
    warn_days: u32,
) -> Result<Vec<ExpiringLot>, AnalyticalError> {
    let mut expiring = Vec::new();
    for lot in list_lots(conn, true, now, Some(warn_days))? {
        if !lot.expiring_soon {
            continue;
        }
        let expires_on = CivilDate::parse("expiresOn", &lot.expires_on)?;
        let since = warning_start(expires_on, warn_days)
            .map_or(lot.recorded_at, |start| start.max(lot.recorded_at));
        expiring.push(ExpiringLot { lot, since });
    }
    Ok(expiring)
}

pub(crate) fn lot_view(
    conn: &Connection,
    id: LotId,
    now: UtcTimestamp,
) -> Result<LotView, AnalyticalError> {
    lot_view_of(
        lots::lot(conn, id)?.ok_or(AnalyticalError::NotFound("lot"))?,
        now,
        None,
    )
}

fn lot_view_of(
    row: LotRow,
    now: UtcTimestamp,
    warn_days: Option<u32>,
) -> Result<LotView, AnalyticalError> {
    let expires_on = CivilDate::parse("expiresOn", &row.expires_on)?;
    let expired = is_expired(expires_on, now);
    let days_left = days_left(expires_on, now);
    let expiring_soon = row.state == LotState::Active
        && matches!((days_left, warn_days), (Some(left), Some(warn)) if left <= warn);
    Ok(LotView {
        lot_id: row.id,
        material_kind: row.material_kind,
        product_name: row.product_name,
        lot_number: row.lot_number,
        level_text: row.level_text,
        sample_label: row.sample_label,
        expires_on: row.expires_on,
        expired,
        days_left,
        expiring_soon,
        state: row.state,
        recorded_at: row.recorded_at,
        revision: row.revision,
    })
}

/// Whole days from the UTC date of `at` to `expires_on`: 0 on the last day of use, `None`
/// once the lot has expired.
fn days_left(expires_on: CivilDate, at: UtcTimestamp) -> Option<u32> {
    let today = CivilDate::utc_date_of(at);
    let days = expires_on.0.since(today.0).ok()?.get_days();
    u32::try_from(days).ok()
}

/// Midnight UTC `warn_days` before `expires_on`, if in range.
fn warning_start(expires_on: CivilDate, warn_days: u32) -> Option<UtcTimestamp> {
    let day = expires_on
        .0
        .checked_sub(jiff::Span::new().days(i64::from(warn_days)))
        .ok()?;
    UtcTimestamp::parse(&format!("{day}T00:00:00.000000000Z")).ok()
}
