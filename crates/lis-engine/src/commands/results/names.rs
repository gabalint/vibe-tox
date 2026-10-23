//! Current account display names for the people a specimen's results record. The views keep
//! only the immutable user ID; the engine looks up the name on every read.

use lis_results::{ResultError, ResultRevisionView, SpecimenResults, ValidityAssessmentView};

use crate::commands::people::{display_names, name_of};

/// A specimen's results, naming who created each revision, assessed validity and decided
/// each reflex.
pub(crate) fn specimen_results(
    conn: &rusqlite::Connection,
    specimen_id: lis_specimens::SpecimenId,
) -> Result<SpecimenResults, ResultError> {
    let mut results = lis_results::get_specimen_results(conn, specimen_id)?;
    name_specimen_results(conn, &mut results)?;
    Ok(results)
}

/// Uses a measured result for an ordered component, returning the specimen's named results.
pub(crate) fn use_measured_result(
    ctx: &mut lis_store::CommandContext<'_>,
    input: lis_results::UseMeasuredResultInput,
) -> Result<SpecimenResults, ResultError> {
    let mut results = lis_results::use_measured_result(ctx, input)?;
    name_specimen_results(ctx.tx(), &mut results)?;
    Ok(results)
}

fn name_specimen_results(
    conn: &rusqlite::Connection,
    results: &mut SpecimenResults,
) -> Result<(), ResultError> {
    let mut ids: Vec<String> = revisions(results).map(|r| r.created_by.clone()).collect();
    ids.extend(validity(results).map(|v| v.assessed_by.clone()));
    ids.extend(
        results
            .reflex_history
            .iter()
            .map(|r| r.assessment.decided_by.clone()),
    );
    let names = display_names(conn, ids.iter().map(String::as_str))?;
    for revision in revisions(results) {
        revision.created_by_display = name_of(&names, &revision.created_by);
    }
    for assessment in validity(results) {
        assessment.assessed_by_display = name_of(&names, &assessment.assessed_by);
    }
    for entry in &mut results.reflex_history {
        entry.assessment.decided_by_display = name_of(&names, &entry.assessment.decided_by);
    }
    Ok(())
}

fn revisions(results: &mut SpecimenResults) -> impl Iterator<Item = &mut ResultRevisionView> {
    results
        .lines
        .iter_mut()
        .flat_map(|line| line.current.iter_mut().chain(line.history.iter_mut()))
}

fn validity(results: &mut SpecimenResults) -> impl Iterator<Item = &mut ValidityAssessmentView> {
    results
        .current_validity
        .iter_mut()
        .chain(results.validity_history.iter_mut())
}
