//! Impact after release, computed rather than stored, so nothing can be
//! lost (brief §4.3, REQ-RPT-002, REQ-ANA-003 released part). Each reason
//! has a stable key; an assessment covers a set of keys, and a new key
//! reopens the impact. Records are append-only, so a condition that has
//! since cleared still counts if it arose after release.

use std::collections::BTreeSet;

use lis_audit::{Stream, succeeded_record_events};
use lis_orders::{ComponentState, ItemState};
use lis_specimens::{ContainerHoldKind, ReceiptDecision};
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::model::{ImpactDecision, LineDisposition};
use crate::repo::quality::{self, AssessmentRow};
use crate::repo::reports::RevisionRow;
use crate::snapshot::ReportSnapshot;

/// One reason a released revision may need a correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImpactReason {
    pub(crate) key: String,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

/// A revision's impact: reasons not yet covered by an assessment, and
/// whether any assessment on this revision requires a correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Impact {
    pub(crate) open: Vec<ImpactReason>,
    pub(crate) correction_due: bool,
    pub(crate) assessments: Vec<AssessmentRow>,
}

impl Impact {
    /// L13: delivery of the revision is paused.
    pub(crate) fn pauses_delivery(&self) -> bool {
        !self.open.is_empty() || self.correction_due
    }
}

/// The impact on `revision`, which must be current.
pub(crate) fn impact(conn: &Connection, revision: &RevisionRow) -> Result<Impact, ReportingError> {
    let assessments = quality::assessments(conn, revision.id)?;
    let covered: BTreeSet<&str> = assessments
        .iter()
        .flat_map(|a| a.keys.iter().map(String::as_str))
        .collect();
    let open = reasons(conn, revision)?
        .into_iter()
        .filter(|reason| !covered.contains(reason.key.as_str()))
        .collect();
    let correction_due = assessments
        .iter()
        .any(|a| a.decision == ImpactDecision::CorrectionRequired);
    Ok(Impact {
        open,
        correction_due,
        assessments,
    })
}

/// Every impact reason of `revision`, covered or not.
pub(crate) fn reasons(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<Vec<ImpactReason>, ReportingError> {
    let frozen = ReportSnapshot::of_revision(conn, revision)?;
    let released_after = release_sequence(conn, revision)?;
    let mut reasons = review_reasons(conn, &frozen, released_after)?;
    reasons.extend(order_reasons(conn, &frozen, released_after)?);
    reasons.extend(identity_reasons(conn, &frozen, released_after)?);
    reasons.extend(specimen_reasons(conn, &frozen, released_after)?);
    Ok(reasons)
}

pub(crate) fn release_sequence(
    conn: &Connection,
    revision: &RevisionRow,
) -> Result<i64, ReportingError> {
    succeeded_record_events(
        conn,
        Stream::Clinical,
        "report.release",
        "report_revision",
        &revision.id.to_string(),
    )?
    .into_iter()
    .next()
    .map(|event| event.seq)
    .ok_or(ReportingError::Inconsistent("report release audit"))
}

fn reason(key: String, code: &'static str, message: &str) -> ImpactReason {
    ImpactReason {
        key,
        code,
        message: message.to_owned(),
    }
}

/// `review_stale:<revision>:<cause>:<ref>` for every append-only result,
/// review or analytical invalidation committed after release.
fn review_reasons(
    conn: &Connection,
    frozen: &ReportSnapshot,
    released_after: i64,
) -> Result<Vec<ImpactReason>, ReportingError> {
    let mut reasons = Vec::new();
    for (_, analyte) in frozen.reported() {
        let Some(reported) = analyte.result.as_ref().map(|r| r.revision_id) else {
            continue;
        };
        for cause in lis_results::stale_reasons_after(conn, reported, released_after)? {
            reasons.push(reason(
                format!("review_stale:{reported}:{}:{}", cause.code, cause.ref_id),
                "review_stale",
                "A reported result or its review dependencies changed after release.",
            ));
        }
    }
    Ok(reasons)
}

/// Cancelled reported components, changed medications and clarification
/// holds opened after release.
fn order_reasons(
    conn: &Connection,
    frozen: &ReportSnapshot,
    released_after: i64,
) -> Result<Vec<ImpactReason>, ReportingError> {
    let order = lis_orders::get_order(conn, frozen.order.order_id)?;
    let mut reasons = Vec::new();
    for line in frozen
        .lines
        .iter()
        .filter(|l| l.disposition == LineDisposition::Reported)
    {
        let cancelled = order.items.iter().any(|item| {
            item.components.iter().any(|c| {
                c.component_id == line.component_id
                    && (c.state == ComponentState::Cancelled || item.state == ItemState::Cancelled)
            })
        });
        if cancelled {
            reasons.push(reason(
                format!("component_cancelled:{}", line.component_id),
                "component_cancelled",
                "A reported component was cancelled after release.",
            ));
        }
    }
    let medications: Vec<(&str, Option<&str>)> = order
        .evidence
        .medications
        .iter()
        .filter(|m| m.removed_in_version.is_none())
        .map(|m| (m.received_text.as_str(), m.as_of_date.as_deref()))
        .collect();
    let frozen_medications: Vec<(&str, Option<&str>)> = frozen
        .order
        .medications
        .iter()
        .map(|m| (m.name.as_str(), m.as_of.as_deref()))
        .collect();
    if medications != frozen_medications {
        reasons.push(reason(
            format!("medications_changed:{}", order.version),
            "medications_changed",
            "The order's medication list changed after release.",
        ));
    }
    let mut user_holds: Vec<_> = order
        .holds
        .iter()
        .filter(|hold| hold.origin == "user")
        .collect();
    // IDs use the process clock, not the mutable clinical clock, and are
    // generated inside the serialized writer transaction.
    user_holds.sort_unstable_by_key(|hold| hold.hold_id);
    let hold_events = succeeded_record_events(
        conn,
        Stream::Clinical,
        "order.hold",
        "order",
        &order.order_id.to_string(),
    )?;
    if user_holds.len() != hold_events.len() {
        return Err(ReportingError::Inconsistent(
            "clarification hold audit history",
        ));
    }
    for (hold, event) in user_holds.into_iter().zip(hold_events) {
        if event.seq <= released_after {
            continue;
        }
        reasons.push(reason(
            format!("clarification_hold:{}", hold.hold_id),
            "clarification_hold",
            "An order clarification hold was opened after release.",
        ));
    }
    Ok(reasons)
}

/// A changed subject identity and identity holds opened after release.
fn identity_reasons(
    conn: &Connection,
    frozen: &ReportSnapshot,
    released_after: i64,
) -> Result<Vec<ImpactReason>, ReportingError> {
    let subject_id = frozen.subject.subject_id;
    let mut reasons = Vec::new();
    let revision = lis_identity::subject_summary(conn, subject_id)?.revision;
    if revision != frozen.subject.revision {
        reasons.push(reason(
            format!("identity_changed:{revision}"),
            "identity_changed",
            "The patient's identity record changed after release.",
        ));
    }
    for record in lis_identity::identity_holds_touching(conn, subject_id)? {
        if record_event_after(
            conn,
            "identity.open_hold",
            "identity_hold",
            &record.hold.id.to_string(),
            released_after,
        )? {
            reasons.push(reason(
                format!("identity_hold:{}", record.hold.id),
                "identity_hold",
                "An identity hold touching this patient was opened after release.",
            ));
        }
    }
    Ok(reasons)
}

/// Held or rejected receipt decisions and quality or excursion holds
/// placed after release (L9).
fn specimen_reasons(
    conn: &Connection,
    frozen: &ReportSnapshot,
    released_after: i64,
) -> Result<Vec<ImpactReason>, ReportingError> {
    let specimen_id = frozen.specimen.specimen_id;
    let accession = lis_specimens::get_accession(conn, frozen.specimen.accession_id)?;
    let mut reasons = Vec::new();
    let decisions = accession
        .specimens
        .iter()
        .filter(|s| s.specimen_id == specimen_id)
        .flat_map(|s| s.decisions.iter());
    let mut decisions: Vec<_> = decisions.collect();
    decisions.sort_unstable_by_key(|decision| decision.decision_id);
    let decision_events = succeeded_record_events(
        conn,
        Stream::Clinical,
        "specimen.record_receipt_decision",
        "specimen",
        &specimen_id.to_string(),
    )?;
    if decisions.len() != decision_events.len() {
        return Err(ReportingError::Inconsistent(
            "receipt decision audit history",
        ));
    }
    for (decision, event) in decisions.into_iter().zip(decision_events) {
        if event.seq > released_after
            && matches!(
                decision.decision,
                ReceiptDecision::Held | ReceiptDecision::Rejected
            )
        {
            reasons.push(reason(
                format!("specimen_decision:{}", decision.decision_id),
                "specimen_decision",
                "The specimen was held or rejected after release.",
            ));
        }
    }
    for record in lis_specimens::container_hold_history(conn, specimen_id)? {
        let blocking = matches!(
            record.hold.kind,
            ContainerHoldKind::Quality | ContainerHoldKind::Excursion
        );
        if blocking
            && record_event_after(
                conn,
                "specimen.place_hold",
                "hold",
                &record.hold.hold_id.to_string(),
                released_after,
            )?
        {
            reasons.push(reason(
                format!("container_hold:{}", record.hold.hold_id),
                "container_hold",
                "A quality or excursion hold was placed on the specimen after release.",
            ));
        }
    }
    Ok(reasons)
}

fn record_event_after(
    conn: &Connection,
    action: &str,
    record_type: &str,
    record_id: &str,
    after: i64,
) -> Result<bool, ReportingError> {
    Ok(
        succeeded_record_events(conn, Stream::Clinical, action, record_type, record_id)?
            .into_iter()
            .any(|event| event.seq > after),
    )
}
