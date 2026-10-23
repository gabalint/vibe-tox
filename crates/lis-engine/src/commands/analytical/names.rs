//! Current account display names for the people analytical views record. The views keep
//! only the immutable user ID; the engine looks up the name on every response.

use std::collections::HashMap;

use lis_analytical::{
    AnalyticalError, DeviationView, PrepBatchView, QcAssessmentHistory, QcAssessmentView,
    RunDetail, RunQcView, SourceRegistrationView,
};

use crate::commands::people::{display_names, name_of};

fn optional_name(names: &HashMap<String, String>, id: Option<&str>) -> Option<String> {
    id.and_then(|id| name_of(names, id))
}

/// Who created the run.
pub(crate) fn name_run(
    conn: &rusqlite::Connection,
    run: &mut RunDetail,
) -> Result<(), AnalyticalError> {
    let names = display_names(conn, [run.created_by.as_str()])?;
    run.created_by_display = name_of(&names, &run.created_by);
    Ok(())
}

/// Who registered and who approved each source.
pub(crate) fn name_sources(
    conn: &rusqlite::Connection,
    sources: &mut [SourceRegistrationView],
) -> Result<(), AnalyticalError> {
    let ids = sources
        .iter()
        .flat_map(|s| std::iter::once(s.authored_by.as_str()).chain(s.approved_by.as_deref()));
    let names = display_names(conn, ids)?;
    for source in sources {
        source.authored_by_display = name_of(&names, &source.authored_by);
        source.approved_by_display = optional_name(&names, source.approved_by.as_deref());
    }
    Ok(())
}

/// Who started and completed the batch, and who performed each step.
pub(crate) fn name_prep_batch(
    conn: &rusqlite::Connection,
    batch: &mut PrepBatchView,
) -> Result<(), AnalyticalError> {
    let ids = [batch.created_by.as_str()]
        .into_iter()
        .chain(batch.completed_by.as_deref())
        .chain(batch.steps.iter().map(|step| step.performed_by.as_str()));
    let names = display_names(conn, ids)?;
    batch.created_by_display = name_of(&names, &batch.created_by);
    batch.completed_by_display = optional_name(&names, batch.completed_by.as_deref());
    for step in &mut batch.steps {
        step.performed_by_display = name_of(&names, &step.performed_by);
    }
    Ok(())
}

/// Who opened and closed one deviation.
pub(crate) fn name_deviation(
    conn: &rusqlite::Connection,
    deviation: &mut DeviationView,
) -> Result<(), AnalyticalError> {
    let names = display_names(conn, deviation_actors(deviation))?;
    set_deviation_names(&names, deviation);
    Ok(())
}

/// Who assessed, decided conflicts and opened or closed deviations on a run's QC.
pub(crate) fn name_run_qc(
    conn: &rusqlite::Connection,
    view: &mut RunQcView,
) -> Result<(), AnalyticalError> {
    let mut ids: Vec<String> = assessments(view).map(|a| a.assessed_by.clone()).collect();
    ids.extend(view.conflicts.iter().filter_map(|c| c.resolved_by.clone()));
    ids.extend(
        view.deviations
            .iter()
            .flat_map(deviation_actors)
            .map(str::to_owned),
    );
    let names = display_names(conn, ids.iter().map(String::as_str))?;
    for assessment in assessments(view) {
        assessment.assessed_by_display = name_of(&names, &assessment.assessed_by);
    }
    for conflict in &mut view.conflicts {
        conflict.resolved_by_display = optional_name(&names, conflict.resolved_by.as_deref());
    }
    for deviation in &mut view.deviations {
        set_deviation_names(&names, deviation);
    }
    Ok(())
}

fn assessments(view: &mut RunQcView) -> impl Iterator<Item = &mut QcAssessmentView> {
    view.analytes
        .iter_mut()
        .map(|analyte| &mut analyte.assessments)
        .chain(std::iter::once(&mut view.run_assessments))
        .chain(
            view.prep_batches
                .iter_mut()
                .map(|batch| &mut batch.assessments),
        )
        .flat_map(|history: &mut QcAssessmentHistory| {
            history.current.iter_mut().chain(history.history.iter_mut())
        })
}

fn deviation_actors(deviation: &DeviationView) -> impl Iterator<Item = &str> {
    std::iter::once(deviation.opened_by.as_str()).chain(deviation.closed_by.as_deref())
}

fn set_deviation_names(names: &HashMap<String, String>, deviation: &mut DeviationView) {
    deviation.opened_by_display = name_of(names, &deviation.opened_by);
    deviation.closed_by_display = optional_name(names, deviation.closed_by.as_deref());
}
