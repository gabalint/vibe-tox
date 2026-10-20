//! Release gates, rows 8–12 of the final transaction (brief §3.4), in one
//! function used by the workspace, by preparation and by release (NFR-01).
//! Each gate is read through its owner's function on the connection given,
//! which is the caller's transaction for prepare and release.

use lis_catalog::{ComponentPurpose, ValidityAction};
use lis_core::GateReason;
use lis_results::ReleaseReviewStatus;
use rusqlite::Connection;

use crate::error::{ReportingError, gate};
use crate::manifest::Manifest;
use crate::snapshot::ReportSnapshot;

/// Every gate that blocks releasing `snapshot`'s reported lines now:
/// open identity holds (row 8), specimen release gate (9), order gate of
/// each reported component (10), validity action (11), and each reported
/// line's current revision, review and quarantine (12).
pub fn evaluate(
    conn: &Connection,
    snapshot: &ReportSnapshot,
) -> Result<Vec<GateReason>, ReportingError> {
    let mut gates = Vec::new();
    if !lis_identity::list_open_identity_holds(conn, snapshot.subject.subject_id)?.is_empty() {
        gates.push(gate(
            "identity_hold_open",
            "An identity hold is open for this patient.",
        ));
    }
    if let Err(reasons) = lis_specimens::release_gate(conn, snapshot.specimen.specimen_id)? {
        gates.extend(reasons);
    }
    for (line, _) in snapshot.reported() {
        if let Err(reasons) = lis_orders::testing_gate(conn, line.component_id)? {
            gates.extend(reasons);
        }
    }
    gates.extend(validity_gates(conn, snapshot)?);
    gates.extend(line_gates(conn, snapshot)?);
    gates.extend(performing_site_gates(snapshot));
    dedupe(&mut gates);
    Ok(gates)
}

/// L8: the `hold` and `recollect` validity actions block release.
fn validity_gates(
    conn: &Connection,
    snapshot: &ReportSnapshot,
) -> Result<Vec<GateReason>, ReportingError> {
    let Some(current) = lis_results::current_validity(conn, snapshot.specimen.specimen_id)? else {
        return Ok(Vec::new());
    };
    let policy = lis_catalog::validity_policy(conn, current.policy_version_id)?;
    let action = policy
        .categories
        .iter()
        .find(|c| c.code == current.category_code)
        .map(|c| c.action)
        .ok_or(ReportingError::Inconsistent("validity category"))?;
    Ok(match action {
        ValidityAction::Hold => vec![gate(
            "validity_hold",
            "The specimen-validity assessment holds release.",
        )],
        ValidityAction::Recollect => vec![gate(
            "validity_recollect",
            "The specimen-validity assessment requires recollection.",
        )],
        ValidityAction::None | ValidityAction::Qualifier | ValidityAction::Comment => Vec::new(),
    })
}

/// Row 12: each reported analyte's frozen revision is still current, its
/// review is current, and no quarantine item is open.
fn line_gates(
    conn: &Connection,
    snapshot: &ReportSnapshot,
) -> Result<Vec<GateReason>, ReportingError> {
    let lines = lis_results::specimen_release_lines(conn, snapshot.specimen.specimen_id)?;
    let mut gates = Vec::new();
    for (component, analyte) in snapshot.reported() {
        let frozen = analyte.result.as_ref().map(|r| r.revision_id);
        let line = lines.iter().find(|line| {
            line.order_component_id == component.component_id
                && line.analyte_id == analyte.analyte_id
        });
        let Some(line) = line else {
            gates.push(stale(
                "revision_superseded",
                "A reported result no longer exists.",
            ));
            continue;
        };
        if line.current.as_ref().map(|c| c.revision_id) != frozen {
            gates.push(stale(
                "revision_superseded",
                "A reported result has a newer revision.",
            ));
        }
        match &line.review {
            ReleaseReviewStatus::Current { .. } => {}
            ReleaseReviewStatus::None => {
                gates.push(stale("review_missing", "A reported result has no review."));
            }
            ReleaseReviewStatus::Stale { reasons } => {
                for reason in reasons {
                    gates.push(stale(&reason.code, &reason.message));
                }
            }
        }
        if line.open_quarantine {
            gates.push(gate(
                "quarantine_open",
                "An import quarantine item is open for a reported analyte.",
            ));
        }
    }
    Ok(gates)
}

fn stale(cause: &str, message: &str) -> GateReason {
    gate(&format!("review_stale:{cause}"), message)
}

/// Report-level gates of preparation that come from the content itself:
/// one accession per item (L1), at least one reported line, and a limit on
/// every reported analytical analyte (`limit_missing`, REQ-RPT-008).
#[must_use]
pub fn content_gates(snapshot: &ReportSnapshot, manifest: &Manifest) -> Vec<GateReason> {
    let mut gates = Vec::new();
    if !manifest.shared_items.is_empty() {
        gates.push(gate(
            "item_in_multiple_accessions",
            "An ordered item is linked to more than one accession; such reports are not supported yet.",
        ));
    }
    if snapshot.reported().next().is_none() {
        gates.push(gate(
            "nothing_to_report",
            "No line is ready to report: each needs a current, reviewed result.",
        ));
    }
    gates.extend(performing_site_gates(snapshot));
    // Validity components are exempt: they print the assessment instead.
    if snapshot.reported().any(|(line, analyte)| {
        line.purpose == ComponentPurpose::Analytical
            && crate::report_data::limits(analyte).is_none()
    }) {
        gates.push(crate::report_data::limit_missing());
    }
    gates
}

fn dedupe(gates: &mut Vec<GateReason>) {
    let mut seen = Vec::new();
    gates.retain(|g| {
        let new = !seen.contains(&g.code);
        if new {
            seen.push(g.code.clone());
        }
        new
    });
}

fn performing_site_gates(snapshot: &ReportSnapshot) -> Vec<GateReason> {
    let unknown = snapshot
        .reported()
        .filter_map(|(_, analyte)| analyte.result.as_ref())
        .any(|result| {
            result.outcome != lis_results::ResultOutcome::NotPerformed
                && (result.performing_site_unknown || result.performing_sites.is_empty())
        });
    if unknown {
        vec![gate(
            "performing_site_unresolved",
            "Confirm the actual performing laboratory on each unpinned run using source evidence before preparing or releasing a new report.",
        )]
    } else {
        Vec::new()
    }
}
