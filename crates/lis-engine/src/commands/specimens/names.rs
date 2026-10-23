//! Current account display names for the people specimen records name. The records retain
//! only immutable user IDs and never cache mutable names; the engine resolves them on every
//! read.

use lis_specimens::{AccessionDetail, ExcursionView, MaterialHistoryPage, SpecimenError};

use crate::commands::people::{display_names, name_of};

/// Who decided each receipt decision on the accession's specimens.
pub(super) fn decisions(
    conn: &rusqlite::Connection,
    detail: &mut AccessionDetail,
) -> Result<(), SpecimenError> {
    let decisions = detail.specimens.iter().flat_map(|s| &s.decisions);
    let names = display_names(conn, decisions.map(|d| d.decided_by.as_str()))?;
    for decision in detail.specimens.iter_mut().flat_map(|s| &mut s.decisions) {
        decision.decided_by_display = name_of(&names, &decision.decided_by);
    }
    Ok(())
}

/// Who recorded and who assessed each storage excursion.
pub(super) fn excursions(
    conn: &rusqlite::Connection,
    excursions: &mut [ExcursionView],
) -> Result<(), SpecimenError> {
    let ids = excursions
        .iter()
        .flat_map(|e| std::iter::once(&e.recorded_by).chain(&e.assessed_by))
        .map(String::as_str);
    let names = display_names(conn, ids)?;
    for excursion in excursions {
        excursion.recorded_by_display = name_of(&names, &excursion.recorded_by);
        excursion.assessed_by_display = excursion
            .assessed_by
            .as_ref()
            .and_then(|by| name_of(&names, by));
    }
    Ok(())
}

/// Who recorded each move, hold, disposal and depletion on a material history page.
pub(super) fn material_history(
    conn: &rusqlite::Connection,
    page: &mut MaterialHistoryPage,
) -> Result<(), SpecimenError> {
    let entries = || {
        page.moves
            .iter()
            .chain(&page.holds)
            .chain(&page.disposal)
            .chain(&page.depletion)
    };
    let names = display_names(conn, entries().map(|e| e.actor.as_str()))?;
    let entries = page
        .moves
        .iter_mut()
        .chain(&mut page.holds)
        .chain(&mut page.disposal)
        .chain(&mut page.depletion);
    for entry in entries {
        entry.actor_display = name_of(&names, &entry.actor);
    }
    Ok(())
}
