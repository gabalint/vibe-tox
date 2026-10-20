//! Demo values for the blank cutoffs of method drafts (starter data brief, decision 3): the
//! demo laboratory's results classify, and every such value is labeled "demo value, not a
//! recommended cutoff" where it is shown. Drafts are edited through `update_method_draft`,
//! as their author would; a value the laboratory entered is never replaced.

use lis_catalog::{
    DemoValues, MethodAnalyte, MethodVersionContent, MethodVersionView, MethodView,
    UpdateMethodDraft,
};
use lis_core::Actor;
use lis_store::CommandContext;
use rusqlite::Connection;

use super::DemoError;
use super::load::DemoDataLoaded;

/// Fills each blank cutoff that has a demo value in the current draft of its method. Drafts
/// another user authored are counted, not edited: only a draft's author may edit it.
pub(super) fn fill_cutoffs(
    ctx: &mut CommandContext<'_>,
    values: &DemoValues,
    loaded: &mut DemoDataLoaded,
) -> Result<(), DemoError> {
    let caller = match ctx.principal().actor() {
        Actor::User { user_id, .. } => Some(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => None,
    };
    for method in lis_catalog::list_methods(ctx.tx())? {
        let Some(draft) = current_draft(ctx.tx(), &method)? else {
            continue;
        };
        let analytes = with_demo_values(&method.code, &draft, values);
        let filled = analytes
            .iter()
            .zip(&draft.analytes)
            .filter(|(new, old)| new.cutoff != old.limits.cutoff)
            .count();
        if filled == 0 {
            continue;
        }
        if caller != Some(draft.lifecycle.created_by) {
            loaded.demo_cutoffs_not_filled += filled;
            continue;
        }
        let input = UpdateMethodDraft {
            method_version_id: draft.id,
            expected_revision: draft.lifecycle.revision,
            content: MethodVersionContent {
                report_notes: draft.report_notes.clone(),
                specimen_type_id: draft.specimen_type_id,
                certainty: draft.certainty,
                output_scale: draft.output_scale,
                threshold_rule: draft.threshold_rule,
                threshold_rounding: draft.threshold_rounding,
                analytes,
            },
        };
        lis_catalog::update_method_draft(ctx, input)?;
        loaded.demo_cutoffs_filled += filled;
    }
    Ok(())
}

/// The method's draft awaiting approval, if it has one.
fn current_draft(
    conn: &Connection,
    method: &MethodView,
) -> Result<Option<MethodVersionView>, DemoError> {
    let Some(number) = method.draft_version else {
        return Ok(None);
    };
    let summary = lis_catalog::list_method_versions(conn, method.id)?
        .into_iter()
        .find(|version| version.version == number);
    match summary {
        Some(summary) => Ok(Some(lis_catalog::get_method_version(conn, summary.id)?)),
        None => Ok(None),
    }
}

/// The draft's analytes with each blank cutoff set to its demo value, if it has one.
fn with_demo_values(
    method_code: &str,
    draft: &MethodVersionView,
    values: &DemoValues,
) -> Vec<MethodAnalyte> {
    draft
        .analytes
        .iter()
        .map(|analyte| {
            let mut limits = analyte.limits.clone();
            if limits.cutoff.is_none() {
                let key = (method_code.to_owned(), analyte.analyte_code.clone());
                limits.cutoff = values.get(&key).cloned();
            }
            limits
        })
        .collect()
}
