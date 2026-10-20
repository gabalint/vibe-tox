//! Gathers a subject's [`Inputs`] for one date of service through owner
//! functions only (brief §5.2): `specimens_for_subject`, the reporting
//! `manifest` and `current_final_release`, `get_order`,
//! `specimen_release_lines`, `component_executions`,
//! `out_of_scope_detections`, the current class mapping and laboratory
//! profile. Usable inside the command's transaction, so every derivation,
//! approval and handoff sees the inputs as they are at commit.

use std::collections::BTreeMap;

use lis_catalog::AnalyteId;
use lis_identity::SubjectId;
use lis_orders::{ComponentState, ItemState, OrderComponentId};
use lis_reporting::{CurrentFinalRelease, ExpectedComponent};
use lis_results::{ReleaseLine, ReleaseReviewStatus};
use lis_specimens::{SpecimenAcceptance, SubjectSpecimen};
use rusqlite::Connection;

use crate::derivation::{
    AnalyteInput, ComponentInput, ExecutionInput, InputRef, Inputs, ReviewInput, SpecimenInput,
};
use crate::error::BillingError;
use crate::mapping::current_class_mapping;
use crate::model::BillingInputKind;
use crate::policy::{self, DATE_OF_SERVICE_RULE};

/// The subject's specimens whose date of service is `date_of_service`;
/// rejected specimens have no billable work.
pub(crate) fn specimens_on(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Vec<SubjectSpecimen>, BillingError> {
    let mut on_date = Vec::new();
    for specimen in lis_specimens::specimens_for_subject(conn, subject_id)? {
        if specimen.acceptance != SpecimenAcceptance::Rejected
            && policy::date_of_service(&specimen)?.as_deref() == date_of_service
        {
            on_date.push(specimen);
        }
    }
    Ok(on_date)
}

/// The canonical inputs of `subject_id` on `date_of_service`.
pub(crate) fn gather(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Inputs, BillingError> {
    let subject = lis_identity::subject_summary(conn, subject_id)?;
    let mapping = current_class_mapping(conn)?;
    let profile = lis_identity::current_laboratory_profile(conn)?;
    let mut input_refs = Vec::new();
    let mut specimens = Vec::new();
    for specimen in specimens_on(conn, subject_id, date_of_service)? {
        specimens.push(specimen_input(conn, &specimen, &mut input_refs)?);
    }
    let mut inputs = Inputs {
        subject_id,
        date_of_service: date_of_service.map(str::to_owned),
        dos_rule: DATE_OF_SERVICE_RULE.to_owned(),
        mapping_version_id: mapping.as_ref().map(|view| view.summary.version_id),
        mapping_entries: mapping.map(|view| view.entries).unwrap_or_default(),
        laboratory_profile_id: profile.as_ref().map(|view| view.profile_id),
        laboratory_profile_version: profile.as_ref().map(|view| view.version),
        laboratory_billing_identity_complete: policy::billing_identity_complete(profile.as_ref()),
        subject_revision: subject.revision,
        specimens,
        input_refs,
        billing_flags: Vec::new(),
    };
    collection_evidence(conn, &mut inputs)?;
    crate::financial_context::gather(conn, &mut inputs)?;
    Ok(inputs)
}

fn specimen_input(
    conn: &Connection,
    specimen: &SubjectSpecimen,
    refs: &mut Vec<InputRef>,
) -> Result<SpecimenInput, BillingError> {
    let specimen_id = specimen.specimen_id;
    let manifest = lis_reporting::manifest(conn, specimen_id)?;
    let order = lis_orders::get_order(conn, manifest.order_id)?;
    push(
        refs,
        BillingInputKind::OrderVersion,
        format!("{}:{}", order.order_id, order.version),
    );
    let release = lis_reporting::current_final_release(conn, specimen_id)?;
    if let Some(release) = &release {
        push(
            refs,
            BillingInputKind::ReportRevision,
            release.revision_id.to_string(),
        );
    }
    let lines: BTreeMap<(OrderComponentId, AnalyteId), ReleaseLine> =
        lis_results::specimen_release_lines(conn, specimen_id)?
            .into_iter()
            .map(|line| ((line.order_component_id, line.analyte_id), line))
            .collect();
    let mut components = Vec::new();
    for expected in &manifest.components {
        if let Some(component) = component_input(conn, expected, &lines, release.as_ref(), refs)? {
            components.push(component);
        }
    }
    let mut performed_not_ordered_refs = Vec::new();
    for detection in lis_results::out_of_scope_detections(conn, specimen_id)? {
        push(
            refs,
            BillingInputKind::QuarantineItem,
            format!("{}:{}", detection.item_id, detection.state.as_str()),
        );
        performed_not_ordered_refs.push(detection.item_id.to_string());
    }
    Ok(SpecimenInput {
        specimen_id,
        order_id: order.order_id,
        order_version: order.version,
        report_revision_id: release.as_ref().map(|release| release.revision_id),
        report_is_current_final: release.is_some(),
        components,
        performed_not_ordered_refs,
    })
}

/// One expected component of an accepted or cancelled item; components of
/// items never accepted are not ordered services.
fn component_input(
    conn: &Connection,
    expected: &ExpectedComponent,
    lines: &BTreeMap<(OrderComponentId, AnalyteId), ReleaseLine>,
    release: Option<&CurrentFinalRelease>,
    refs: &mut Vec<InputRef>,
) -> Result<Option<ComponentInput>, BillingError> {
    let item_cancelled = match expected.item_state {
        ItemState::Accepted => false,
        ItemState::Cancelled => true,
        ItemState::Requested | ItemState::Declined => return Ok(None),
    };
    let component_id = expected.component_id;
    push(
        refs,
        BillingInputKind::ComponentState,
        format!(
            "{component_id}:{}:{}",
            expected.item_state.as_str(),
            expected.state.as_str()
        ),
    );
    let mut executions = Vec::new();
    for execution in lis_analytical::component_executions(conn, component_id)? {
        push(
            refs,
            BillingInputKind::Execution,
            format!("{}:{}", execution.execution_id, execution.state.as_str()),
        );
        executions.push(ExecutionInput {
            execution_id: execution.execution_id,
            state: execution.state,
        });
    }
    let analytes = expected
        .catalog
        .analytes
        .iter()
        .map(|analyte| {
            let line = lines.get(&(component_id, analyte.analyte_id));
            analyte_input(expected, analyte.analyte_id, line, release, refs)
        })
        .collect();
    Ok(Some(ComponentInput {
        component_id,
        purpose: expected.catalog.purpose,
        cancelled: item_cancelled || expected.state == ComponentState::Cancelled,
        analytes,
        executions,
    }))
}

fn analyte_input(
    expected: &ExpectedComponent,
    analyte_id: AnalyteId,
    line: Option<&ReleaseLine>,
    release: Option<&CurrentFinalRelease>,
    refs: &mut Vec<InputRef>,
) -> AnalyteInput {
    let current = line.and_then(|line| line.current.as_ref());
    if let Some(current) = current {
        push(
            refs,
            BillingInputKind::ResultRevision,
            current.revision_id.to_string(),
        );
    }
    let review = match line.map(|line| &line.review) {
        Some(ReleaseReviewStatus::Current { review_id, .. }) => {
            push(refs, BillingInputKind::Review, review_id.to_string());
            ReviewInput::Current {
                review_id: *review_id,
            }
        }
        Some(ReleaseReviewStatus::Stale { .. }) => ReviewInput::Stale,
        Some(ReleaseReviewStatus::None) | None => ReviewInput::None,
    };
    let reported_on_current_final = release.is_some_and(|release| {
        release.reported_lines.iter().any(|reported| {
            reported.order_component_id == expected.component_id
                && reported.analyte_id == analyte_id
        })
    });
    AnalyteInput {
        analyte_id,
        certainty: expected.catalog.certainty,
        result_revision_id: current.map(|current| current.revision_id),
        outcome: current.map(|current| current.outcome),
        review,
        reported_on_current_final,
    }
}

fn push(refs: &mut Vec<InputRef>, kind: BillingInputKind, ref_id: String) {
    refs.push(InputRef { kind, ref_id });
}

fn collection_evidence(conn: &Connection, inputs: &mut Inputs) -> Result<(), BillingError> {
    for specimen in &inputs.specimens {
        for correction in
            lis_specimens::collection_corrections_for_specimen(conn, specimen.specimen_id)?
        {
            inputs.input_refs.push(InputRef {
                kind: BillingInputKind::CollectionRevision,
                ref_id: format!(
                    "{}:{}:{}",
                    correction.correction_id,
                    correction.revision,
                    correction.resolved_at.is_some()
                ),
            });
            if correction.resolved_at.is_none() {
                inputs.billing_flags.push(crate::BillingFlag {
                    code: "collection_review_pending".to_owned(),
                    ref_id: specimen.specimen_id.to_string(),
                    blocking: true,
                });
            }
        }
    }
    Ok(())
}
