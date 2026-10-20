//! Container holds and storage excursions (REQ-SPC-005).

use std::collections::BTreeSet;

use lis_audit::AuditDetail;
use lis_core::{Capability, UtcTimestamp};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{actor, clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, ensure_revision, invalid};
use crate::input::{self, TEXT_MAX};
use crate::model::{
    ContainerHoldKind, ContainerHoldState, ContainerId, ExcursionId, ExcursionOutcome,
    ExcursionState, HoldId, LocationId,
};
use crate::queries;
use crate::repo::holds::{self as repo, Assessment, NewExcursion, NewHold};
use crate::repo::{config, containers};
use crate::views::{ContainerHoldView, ExcursionView};

/// Most containers one excursion assessment may hold.
const MAX_EXCURSION_HOLDS: usize = 500;

/// Input of [`place_hold`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct PlaceContainerHoldInput {
    /// The container.
    pub container_id: ContainerId,
    /// Retention, legal or quality; excursion holds come from an assessment.
    pub kind: ContainerHoldKind,
    /// Why.
    pub reason: String,
}

/// Places a hold on a container that is not disposed (REQ-SPC-005). An open
/// hold blocks disposal and aliquoting.
pub fn place_hold(
    ctx: &mut CommandContext<'_>,
    input: PlaceContainerHoldInput,
) -> Result<ContainerHoldView, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    if input.kind == ContainerHoldKind::Excursion {
        return Err(invalid(
            "kind",
            "excursion_hold",
            "Excursion holds are placed by assessing the excursion.",
        ));
    }
    let id = insert_hold(ctx, input.container_id, input.kind, None, &reason)?;
    repo::hold(ctx.tx(), id)?.ok_or(SpecimenError::NotFound("hold"))
}

/// Inserts an open hold on a container that is not disposed, and audits it.
fn insert_hold(
    ctx: &CommandContext<'_>,
    container_id: ContainerId,
    kind: ContainerHoldKind,
    excursion_id: Option<ExcursionId>,
    reason: &str,
) -> Result<HoldId, SpecimenError> {
    let container = containers::container(ctx.tx(), container_id)?
        .ok_or(SpecimenError::NotFound("container"))?;
    container.state.ensure_not_disposed()?;
    let id = HoldId::generate();
    let by = actor(ctx);
    repo::insert_hold(
        ctx.tx(),
        &NewHold {
            id,
            container_id,
            kind,
            excursion_id,
            reason,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.audit(
        clinical(actions::PLACE_HOLD)
            .record("hold", id)
            .new_revision(1)
            .detail("container", AuditDetail::id(container_id))
            .detail("kind", AuditDetail::code(kind.as_str()))
            .reason(reason),
    )?;
    Ok(id)
}

/// Input of [`release_hold`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ReleaseContainerHoldInput {
    /// The hold.
    pub hold_id: HoldId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is released.
    pub reason: String,
}

/// Releases an open hold, with a reason (REQ-SPC-005).
pub fn release_hold(
    ctx: &mut CommandContext<'_>,
    input: ReleaseContainerHoldInput,
) -> Result<ContainerHoldView, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    let hold = repo::hold(ctx.tx(), input.hold_id)?.ok_or(SpecimenError::NotFound("hold"))?;
    ensure_revision(hold.revision, input.expected_revision)?;
    if hold.state != ContainerHoldState::Open {
        return Err(SpecimenError::AlreadyClosed("hold"));
    }
    let by = actor(ctx);
    repo::release_hold(
        ctx.tx(),
        hold.hold_id,
        &reason,
        &by,
        ctx.now(),
        hold.revision + 1,
    )?;
    ctx.audit(
        clinical(actions::RELEASE_HOLD)
            .record("hold", hold.hold_id)
            .old_revision(hold.revision)
            .new_revision(hold.revision + 1)
            .detail("container", AuditDetail::id(hold.container_id))
            .reason(reason),
    )?;
    repo::hold(ctx.tx(), hold.hold_id)?.ok_or(SpecimenError::NotFound("hold"))
}

/// Input of [`record_excursion`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RecordExcursionInput {
    /// Where it happened.
    pub location_id: LocationId,
    /// When it started.
    pub observed_from: UtcTimestamp,
    /// When it ended, if known.
    #[serde(default)]
    #[ts(optional)]
    pub observed_to: Option<UtcTimestamp>,
    /// What was observed.
    pub description: String,
}

/// Records a storage excursion at a location, open for assessment (REQ-SPC-005).
pub fn record_excursion(
    ctx: &mut CommandContext<'_>,
    input: RecordExcursionInput,
) -> Result<ExcursionView, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let description = input::required_text("description", &input.description, TEXT_MAX)?;
    if input.observed_from > ctx.now() {
        return Err(invalid(
            "observedFrom",
            "in_future",
            "The excursion start is later than now.",
        ));
    }
    if input.observed_to.is_some_and(|to| to < input.observed_from) {
        return Err(invalid(
            "observedTo",
            "before_start",
            "The excursion end is before its start.",
        ));
    }
    config::location(ctx.tx(), input.location_id)?.ok_or(SpecimenError::NotFound("location"))?;
    let id = ExcursionId::generate();
    let by = actor(ctx);
    repo::insert_excursion(
        ctx.tx(),
        &NewExcursion {
            id,
            location_id: input.location_id,
            observed_from: input.observed_from,
            observed_to: input.observed_to,
            description: &description,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.audit(
        clinical(actions::RECORD_EXCURSION)
            .record("excursion", id)
            .new_revision(1)
            .detail("location", AuditDetail::id(input.location_id)),
    )?;
    queries::excursion(ctx.tx(), id)
}

/// Input of [`assess_excursion`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AssessExcursionInput {
    /// The excursion.
    pub excursion_id: ExcursionId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// No impact, or holds placed on `containerIds`.
    pub outcome: ExcursionOutcome,
    /// The containers to hold; empty for no impact.
    #[serde(default)]
    pub container_ids: Vec<ContainerId>,
    /// The assessment's rationale.
    pub reason: String,
}

/// Assesses an open excursion (REQ-SPC-005). `holds_placed` places an
/// excursion hold on each listed container in the same transaction.
pub fn assess_excursion(
    ctx: &mut CommandContext<'_>,
    input: AssessExcursionInput,
) -> Result<ExcursionView, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    let containers = assessed_containers(&input)?;
    let excursion = repo::excursion(ctx.tx(), input.excursion_id)?
        .ok_or(SpecimenError::NotFound("excursion"))?;
    ensure_revision(excursion.revision, input.expected_revision)?;
    if excursion.state != ExcursionState::Open {
        return Err(SpecimenError::AlreadyClosed("excursion"));
    }
    let by = actor(ctx);
    repo::assess_excursion(
        ctx.tx(),
        &Assessment {
            id: input.excursion_id,
            outcome: input.outcome,
            reason: &reason,
            by: &by,
            at: ctx.now(),
            revision: excursion.revision + 1,
        },
    )?;
    for container_id in &containers {
        insert_hold(
            ctx,
            *container_id,
            ContainerHoldKind::Excursion,
            Some(input.excursion_id),
            &reason,
        )?;
    }
    ctx.audit(
        clinical(actions::ASSESS_EXCURSION)
            .record("excursion", input.excursion_id)
            .old_revision(excursion.revision)
            .new_revision(excursion.revision + 1)
            .detail("outcome", AuditDetail::code(input.outcome.as_str()))
            .detail("holds", AuditDetail::count(containers.len()))
            .reason(reason),
    )?;
    queries::excursion(ctx.tx(), input.excursion_id)
}

/// The containers to hold: none for no impact, at least one otherwise.
fn assessed_containers(
    input: &AssessExcursionInput,
) -> Result<BTreeSet<ContainerId>, SpecimenError> {
    let containers: BTreeSet<ContainerId> = input.container_ids.iter().copied().collect();
    let valid = match input.outcome {
        ExcursionOutcome::NoImpact => containers.is_empty(),
        ExcursionOutcome::HoldsPlaced => {
            !containers.is_empty()
                && containers.len() == input.container_ids.len()
                && containers.len() <= MAX_EXCURSION_HOLDS
        }
    };
    if valid {
        Ok(containers)
    } else {
        Err(invalid(
            "containerIds",
            "invalid_selection",
            "List each affected container once when holds are placed, and none for no impact.",
        ))
    }
}
