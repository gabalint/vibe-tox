//! The deviation targets a position's results depend on (REQ-ANA-004): its instrument, run,
//! lots, preparation batch and the position itself, and the deviations recorded on them.

use lis_core::InstrumentId;
use rusqlite::Connection;

use crate::error::AnalyticalError;
use crate::model::{DeviationScope, LotId, RunId};
use crate::repo::runs::PositionRow;
use crate::repo::{deviations, lots, runs};
use crate::views::DeviationView;

/// A run's instrument, the run and the run's lots.
pub(crate) fn run_level_targets(
    conn: &Connection,
    instrument_id: InstrumentId,
    run_id: RunId,
) -> Result<Vec<(DeviationScope, String)>, AnalyticalError> {
    let mut targets = vec![
        (DeviationScope::Instrument, instrument_id.to_string()),
        (DeviationScope::Run, run_id.to_string()),
    ];
    targets.extend(lot_targets(runs::run_lots(conn, run_id)?));
    Ok(targets)
}

/// The position, its own lot, its preparation batch and the lots that
/// batch's steps used.
pub(crate) fn position_level_targets(
    conn: &Connection,
    position: &PositionRow,
) -> Result<Vec<(DeviationScope, String)>, AnalyticalError> {
    let mut targets = vec![(DeviationScope::Position, position.id.to_string())];
    targets.extend(lot_targets(position.lot_id));
    if let Some(batch) = position.prep_batch_id {
        targets.push((DeviationScope::PrepBatch, batch.to_string()));
        targets.extend(lot_targets(lots::prep_batch_lots(conn, batch)?));
    }
    Ok(targets)
}

fn lot_targets(
    lots: impl IntoIterator<Item = LotId>,
) -> impl Iterator<Item = (DeviationScope, String)> {
    lots.into_iter()
        .map(|lot| (DeviationScope::Lot, lot.to_string()))
}

/// Every deviation on the targets, without repeats, newest first per target.
pub(crate) fn deviations_on_targets(
    conn: &Connection,
    targets: &[(DeviationScope, String)],
) -> Result<Vec<DeviationView>, AnalyticalError> {
    let mut found: Vec<DeviationView> = Vec::new();
    for (scope, id) in targets {
        for deviation in deviations::on_target(conn, *scope, id)? {
            if !found
                .iter()
                .any(|seen| seen.deviation_id == deviation.deviation_id)
            {
                found.push(deviation);
            }
        }
    }
    Ok(found)
}
