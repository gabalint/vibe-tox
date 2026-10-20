//! A person's decision on a changed QC row the source registration cannot
//! classify (REQ-ANA-003 re-export part for QC rows, REQ-ANA-004, R5).

use lis_audit::AuditDetail;
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{clinical, reason, require_user};
use crate::actions;
use crate::error::{AnalyticalError, ensure_revision};
use crate::model::{QcConflictId, QcConflictResolution, QcConflictState, QcObservationId};
use crate::reexport::SourceRow;
use crate::repo::conflicts::{self as repo, ConflictRow, Resolution};
use crate::repo::qc::{self as qc_repo, NewObservation};
use crate::views::QcConflictView;

/// Input of [`resolve_qc_conflict`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ResolveQcConflictInput {
    /// The open conflict.
    pub conflict_id: QcConflictId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Apply the row as a correction, or keep the stored observation.
    pub resolution: QcConflictResolution,
    /// Why, as entered.
    pub reason: String,
}

/// Resolves an open QC conflict, by a signed-in user (REQ-ANA-004).
/// Accepting appends the row as an observation that supersedes exactly the
/// observation it contradicted, mapped as the registration in force at
/// receipt mapped it; the run × analyte assessment then becomes `qc_stale`
/// until QC is assessed again. Refused with `EvidenceChanged` when that
/// observation is no longer current. Rejecting keeps the stored observation.
pub fn resolve_qc_conflict(
    ctx: &mut CommandContext<'_>,
    input: ResolveQcConflictInput,
) -> Result<QcConflictView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let by = require_user(ctx)?;
    let reason = reason(&input.reason)?;
    let found = repo::conflict(ctx.tx(), input.conflict_id)?
        .ok_or(AnalyticalError::NotFound("QC conflict"))?;
    let conflict = &found.view;
    ensure_revision(conflict.revision, input.expected_revision)?;
    if conflict.state != QcConflictState::Open {
        return Err(AnalyticalError::WrongState("QC conflict"));
    }
    let (state, observation_id) = match input.resolution {
        QcConflictResolution::Reject => (QcConflictState::Rejected, None),
        QcConflictResolution::AcceptAsCorrection => (
            QcConflictState::Accepted,
            Some(append_correction(ctx, &found)?),
        ),
    };
    repo::resolve(
        ctx.tx(),
        conflict.conflict_id,
        conflict.revision + 1,
        &Resolution {
            state,
            observation_id,
            reason: &reason,
            by: &by,
            at: ctx.now(),
        },
    )?;
    let mut event = clinical(actions::RESOLVE_QC_CONFLICT)
        .record("qc_conflict", conflict.conflict_id)
        .old_revision(conflict.revision)
        .new_revision(conflict.revision + 1)
        .detail("resolution", AuditDetail::code(input.resolution.as_str()))
        .reason(reason);
    if let Some(id) = observation_id {
        event = event
            .detail("observation", AuditDetail::id(id))
            .detail("supersedes", AuditDetail::id(conflict.conflicts_with_id));
    }
    ctx.audit(event)?;
    Ok(repo::conflict(ctx.tx(), conflict.conflict_id)?
        .ok_or(AnalyticalError::NotFound("QC conflict"))?
        .view)
}

/// Appends the conflicting row as the observation superseding exactly the
/// one it contradicted, which must still be current.
fn append_correction(
    ctx: &CommandContext<'_>,
    found: &ConflictRow,
) -> Result<QcObservationId, AnalyticalError> {
    let conflict = &found.view;
    let current =
        qc_repo::current_observation(ctx.tx(), conflict.position_id, &conflict.source_test_code)?;
    if current.map(|(id, _)| id) != Some(conflict.conflicts_with_id) {
        return Err(AnalyticalError::EvidenceChanged);
    }
    let row = SourceRow {
        value_text: conflict.value_text.clone(),
        units_text: conflict.units_text.clone(),
        flags_text: conflict.flags_text.clone(),
        status_text: conflict.status_text.clone(),
    };
    Ok(qc_repo::insert_observation(
        ctx.tx(),
        &NewObservation {
            position_id: conflict.position_id,
            receipt_id: found.receipt_id,
            record_path: &found.record_path,
            source_test_code: &conflict.source_test_code,
            analyte_id: conflict.analyte_id,
            row: &row,
            supersedes: Some(conflict.conflicts_with_id),
            at: ctx.now(),
        },
    )?)
}
