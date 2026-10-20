//! Builds the snapshot from owner queries only; works on a read connection
//! (workspace) and inside a write transaction (prepare, release, NFR-01).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use lis_catalog::{
    AnalyteId, ComponentAnalyteView, MethodAnalyte, MethodVersionId, MethodVersionView,
};
use lis_core::{GateReason, UserId};
use lis_orders::{ComponentState, ItemState, OrderComponentId};
use lis_results::{
    ReleaseLine, ReleaseReviewStatus, ResultId, ResultRevisionId, TechnicalReviewId,
};
use lis_specimens::SpecimenId;
use rusqlite::Connection;

use super::sections::{laboratory, order_section, specimen_section, subject_section, validity};
use super::{AnalyteLine, ComponentLine, ReportSnapshot, ResultSection, TriggerLine};
use crate::dispositions::{
    AnalyteState, ComponentFacts, component_disposition, trigger_disposition,
};
use crate::error::{ReportingError, gate};
use crate::manifest::{ExpectedComponent, Manifest, manifest_of};
use crate::model::LineDisposition;

/// A current approved review of one analyte's current revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewFacts {
    pub(crate) review_id: TechnicalReviewId,
    pub(crate) reviewed_by: UserId,
}

/// One analyte line as stored with a candidate: its current revision and,
/// when reported, its review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineFacts {
    pub(crate) component_id: OrderComponentId,
    pub(crate) analyte_id: AnalyteId,
    pub(crate) result: Option<(ResultId, ResultRevisionId)>,
    pub(crate) review: Option<ReviewFacts>,
}

/// A built snapshot with the facts that are stored beside it.
pub struct Built {
    /// The content.
    pub snapshot: ReportSnapshot,
    /// The manifest it was built from.
    pub manifest: Manifest,
    pub(crate) lines: Vec<LineFacts>,
    /// What holds each component that is not reported, by component.
    pub(crate) holds: Vec<(OrderComponentId, Vec<GateReason>)>,
}

/// Builds the report snapshot of `specimen_id` as of now. Refuses with the
/// gate `laboratory_profile_missing` until a laboratory profile is set.
pub fn build(conn: &Connection, specimen_id: SpecimenId) -> Result<Built, ReportingError> {
    let accession_id = lis_specimens::specimen_accession(conn, specimen_id)?;
    let accession = lis_specimens::get_accession(conn, accession_id)?;
    let specimen = accession
        .specimens
        .iter()
        .find(|s| s.specimen_id == specimen_id)
        .ok_or(ReportingError::NotFound("specimen"))?;
    let order = lis_orders::get_order(conn, accession.order_id)?;
    let manifest = manifest_of(conn, &accession, specimen_id, &order)?;
    let laboratory = laboratory(conn)?;
    let release_lines = lis_results::specimen_release_lines(conn, specimen_id)?;
    let mut methods = HashMap::new();
    let mut lines = Vec::new();
    let mut facts = Vec::new();
    let mut holds = Vec::new();
    for expected in &manifest.components {
        let method = method(conn, &mut methods, expected.catalog.method_version_id)?;
        let built = component_line(conn, expected, method, &release_lines)?;
        holds.push((expected.component_id, built.holds));
        lines.push(built.line);
        facts.extend(built.facts);
    }
    super::provenance::populate(conn, specimen_id, &mut lines)?;
    let snapshot = ReportSnapshot {
        specimen: specimen_section(conn, &accession, specimen)?,
        subject: subject_section(&lis_identity::subject_summary(conn, accession.subject_id)?),
        order: order_section(conn, &order)?,
        triggers: trigger_lines(conn, &manifest)?,
        lines,
        validity: validity(conn, specimen_id)?,
        laboratory,
    };
    Ok(Built {
        snapshot,
        manifest,
        lines: facts,
        holds,
    })
}

fn method<'a>(
    conn: &Connection,
    cache: &'a mut HashMap<MethodVersionId, MethodVersionView>,
    id: MethodVersionId,
) -> Result<&'a MethodVersionView, ReportingError> {
    match cache.entry(id) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => Ok(entry.insert(lis_catalog::get_method_version(conn, id)?)),
    }
}

/// One component's line, its stored line facts and what holds it.
struct BuiltLine {
    line: ComponentLine,
    facts: Vec<LineFacts>,
    holds: Vec<GateReason>,
}

fn component_line(
    conn: &Connection,
    expected: &ExpectedComponent,
    method: &MethodVersionView,
    release_lines: &[ReleaseLine],
) -> Result<BuiltLine, ReportingError> {
    let mut analytes = Vec::new();
    let mut states = Vec::new();
    let mut facts = Vec::new();
    let mut holds = Vec::new();
    let mut quarantine_open = false;
    for analyte in &expected.catalog.analytes {
        let limits = method
            .analytes
            .iter()
            .find(|m| m.limits.analyte_id == analyte.analyte_id)
            .map(|m| &m.limits);
        let line = release_lines.iter().find(|line| {
            line.order_component_id == expected.component_id
                && line.analyte_id == analyte.analyte_id
        });
        quarantine_open |= line.is_some_and(|line| line.open_quarantine);
        if let Some(ReleaseReviewStatus::Stale { reasons }) = line.map(|line| &line.review) {
            holds.extend(reasons.iter().cloned());
        }
        states.push(analyte_state(line));
        facts.push(LineFacts {
            component_id: expected.component_id,
            analyte_id: analyte.analyte_id,
            result: line.and_then(|l| l.current.as_ref().map(|c| (l.result_id, c.revision_id))),
            review: line.and_then(current_review),
        });
        analytes.push(analyte_line(analyte, limits, line));
    }
    if quarantine_open {
        holds.push(gate(
            "quarantine_open",
            "An import quarantine item is open for this component.",
        ));
    }
    let order_gate = lis_orders::testing_gate(conn, expected.component_id)?;
    let order_gate_passes = order_gate.is_ok();
    if let Err(reasons) = order_gate {
        holds.extend(reasons);
    }
    let disposition = component_disposition(&ComponentFacts {
        cancelled: expected.state == ComponentState::Cancelled
            || matches!(
                expected.item_state,
                ItemState::Cancelled | ItemState::Declined
            ),
        analytes: states,
        quarantine_open,
        order_gate_passes,
    });
    let line = ComponentLine {
        component_id: expected.component_id,
        item_id: expected.item_id,
        orderable_id: expected.orderable_id,
        orderable_name: expected.orderable_name.clone(),
        method_name: method.method_name.clone(),
        method_version: method.version,
        report_notes: method.report_notes.clone(),
        certainty: expected.catalog.certainty,
        output_scale: expected.catalog.output_scale,
        sequence_role: expected.sequence_role,
        purpose: expected.catalog.purpose,
        disposition,
        analytes,
    };
    if disposition == LineDisposition::Reported {
        holds.clear();
    }
    Ok(BuiltLine { line, facts, holds })
}

fn analyte_line(
    analyte: &ComponentAnalyteView,
    limits: Option<&MethodAnalyte>,
    line: Option<&ReleaseLine>,
) -> AnalyteLine {
    AnalyteLine {
        analyte_id: analyte.analyte_id,
        name: analyte.name.clone(),
        unit: limits.and_then(|l| l.unit.clone()),
        cutoff: limits.and_then(|l| l.cutoff.clone()),
        limit_of_detection: limits.and_then(|l| l.limit_of_detection.clone()),
        limit_of_quantitation: limits.and_then(|l| l.limit_of_quantitation.clone()),
        reference_interval: limits.and_then(|l| l.reference_interval.clone()),
        result: line.and_then(|line| {
            line.current.as_ref().map(|c| ResultSection {
                result_id: line.result_id,
                revision_id: c.revision_id,
                revision_no: c.revision_no,
                outcome: c.outcome,
                comparator: c.comparator,
                magnitude: c.magnitude.clone(),
                units_text: c.units_text.clone(),
                code_text: c.code_text.clone(),
                classification: c.classification.clone(),
                recorded_at: Some(c.created_at),
                first_reported_at: None,
                performing_sites: Vec::new(),
                performing_site_unknown: false,
            })
        }),
    }
}

fn analyte_state(line: Option<&ReleaseLine>) -> AnalyteState {
    match line {
        Some(line) if line.current.is_some() => match line.review {
            ReleaseReviewStatus::None => AnalyteState::Unreviewed,
            ReleaseReviewStatus::Stale { .. } => AnalyteState::ReviewStale,
            ReleaseReviewStatus::Current { .. } => AnalyteState::Reviewed,
        },
        Some(_) | None => AnalyteState::NoResult,
    }
}

fn current_review(line: &ReleaseLine) -> Option<ReviewFacts> {
    match line.review {
        ReleaseReviewStatus::Current {
            review_id,
            reviewed_by,
            ..
        } => Some(ReviewFacts {
            review_id,
            reviewed_by,
        }),
        ReleaseReviewStatus::None | ReleaseReviewStatus::Stale { .. } => None,
    }
}

fn trigger_lines(
    conn: &Connection,
    manifest: &Manifest,
) -> Result<Vec<TriggerLine>, ReportingError> {
    let mut lines = Vec::new();
    for trigger in &manifest.triggers {
        let Some(disposition) = trigger_disposition(trigger.outcome.as_deref()) else {
            continue;
        };
        let orderable = lis_catalog::list_orderables(conn)?
            .into_iter()
            .find(|o| o.id == trigger.orderable_id)
            .ok_or(ReportingError::Inconsistent("reflex orderable"))?;
        lines.push(TriggerLine {
            parent_item_id: trigger.parent_item_id,
            orderable_id: trigger.orderable_id,
            orderable_name: orderable.name,
            disposition,
        });
    }
    Ok(lines)
}
