//! Aliquots, depletion, moves and disposal (REQ-SPC-001, REQ-SPC-003, REQ-SPC-005).

use lis_audit::AuditDetail;
use lis_core::{Capability, Decimal, DecimalText};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{actor, clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, ensure_revision};
use crate::input;
use crate::material::remaining_volume;
use crate::model::{ContainerId, ContainerState, LocationId, VolumeKind};
use crate::numbering;
use crate::queries;
use crate::repo::config;
use crate::repo::containers::{self as repo, ContainerRow, NewContainer};
use crate::views::AccessionDetail;

/// Input of [`create_aliquot`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateAliquotInput {
    /// The container the material is taken from.
    pub parent_container_id: ContainerId,
    /// The parent revision the aliquot was planned on.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The volume taken, if known.
    #[serde(default)]
    #[ts(optional)]
    pub volume_taken: Option<DecimalText>,
    /// Unit text of the volume taken.
    #[serde(default)]
    #[ts(optional)]
    pub volume_unit: Option<String>,
    /// Estimated, measured or unknown.
    pub volume_kind: VolumeKind,
}

/// Takes an aliquot into a new labelled container (REQ-SPC-003). Refused if
/// the parent is depleted, disposed or on hold, if its specimen is held or
/// rejected, or if the volume taken exceeds the parent's known remaining
/// volume. Volume arithmetic only uses identical unit text; when the known
/// remainder reaches zero the parent becomes depleted.
pub fn create_aliquot(
    ctx: &mut CommandContext<'_>,
    input: CreateAliquotInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let volume = input::volume(
        "volumeTaken",
        input.volume_taken.as_ref(),
        input.volume_unit.as_deref(),
        input.volume_kind,
    )?;
    let parent = load(ctx, input.parent_container_id, input.expected_revision)?;
    parent.state.ensure_available()?;
    if parent.open_holds > 0 {
        return Err(SpecimenError::HoldOpen);
    }
    parent.acceptance.ensure_usable()?;
    let earlier = repo::taken_volumes(ctx.tx(), parent.id)?;
    let remaining = remaining_volume(parent.volume.as_ref(), &earlier, volume.amount.as_ref())?;
    let depleted = remaining == Some(Decimal::ZERO);
    let next_state = if depleted {
        ContainerState::Depleted
    } else {
        parent.state
    };
    change_state(ctx, &parent, next_state)?;
    let id = ContainerId::generate();
    let label_code = numbering::next_container_code(ctx.tx())?;
    let by = actor(ctx);
    repo::insert_container(
        ctx.tx(),
        &NewContainer {
            id,
            specimen_id: parent.specimen_id,
            label_code: &label_code,
            parent: Some(parent.id),
            volume: &volume,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.audit(
        clinical(actions::CREATE_ALIQUOT)
            .record("container", id)
            .new_revision(1)
            .detail("parent_container", AuditDetail::id(parent.id))
            .detail(
                "parent_new_revision",
                AuditDetail::integer(parent.revision + 1),
            )
            .detail("parent_depleted", AuditDetail::boolean(depleted)),
    )?;
    accession_of(ctx, parent.id)
}

/// Input of [`mark_container_depleted`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct MarkContainerDepletedInput {
    /// The container.
    pub container_id: ContainerId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is depleted.
    pub reason: String,
}

/// Marks an available container depleted, with a reason (REQ-SPC-003).
pub fn mark_container_depleted(
    ctx: &mut CommandContext<'_>,
    input: MarkContainerDepletedInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let reason = input::reason(Some(&input.reason))?;
    let container = load(ctx, input.container_id, input.expected_revision)?;
    container.state.ensure_available()?;
    change_state(ctx, &container, ContainerState::Depleted)?;
    ctx.audit(
        clinical(actions::MARK_DEPLETED)
            .record("container", container.id)
            .old_revision(container.revision)
            .new_revision(container.revision + 1)
            .reason(reason),
    )?;
    accession_of(ctx, container.id)
}

/// Input of [`move_container`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct MoveContainerInput {
    /// The container.
    pub container_id: ContainerId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The new storage location; must be active.
    pub location_id: LocationId,
    /// Why, if the lab records it.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// Moves a container that is not disposed to an active location, with actor
/// and time evidence (REQ-SPC-003).
pub fn move_container(
    ctx: &mut CommandContext<'_>,
    input: MoveContainerInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let reason = input::optional_text("reason", input.reason.as_deref(), input::TEXT_MAX)?;
    let container = load(ctx, input.container_id, input.expected_revision)?;
    container.state.ensure_not_disposed()?;
    let location = config::location(ctx.tx(), input.location_id)?
        .ok_or(SpecimenError::NotFound("location"))?;
    if !location.active {
        return Err(SpecimenError::Inactive("location"));
    }
    change_state(ctx, &container, container.state)?;
    let by = actor(ctx);
    repo::insert_move(
        ctx.tx(),
        container.id,
        location.location_id,
        reason.as_deref(),
        &by,
        ctx.now(),
    )?;
    let mut event = clinical(actions::MOVE_CONTAINER)
        .record("container", container.id)
        .old_revision(container.revision)
        .new_revision(container.revision + 1)
        .detail("location", AuditDetail::id(location.location_id));
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    accession_of(ctx, container.id)
}

/// Input of [`dispose_container`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DisposeContainerInput {
    /// The container.
    pub container_id: ContainerId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is disposed.
    pub reason: String,
}

/// Records physical disposal (REQ-SPC-005). Refused while any hold on the
/// container is open; the container row and its lineage stay.
pub fn dispose_container(
    ctx: &mut CommandContext<'_>,
    input: DisposeContainerInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let reason = input::reason(Some(&input.reason))?;
    let container = load(ctx, input.container_id, input.expected_revision)?;
    container.state.ensure_not_disposed()?;
    // REQ-SPC-005: a held container cannot be disposed.
    if container.open_holds > 0 {
        return Err(SpecimenError::HoldOpen);
    }
    change_state(ctx, &container, ContainerState::Disposed)?;
    let by = actor(ctx);
    repo::insert_disposal(ctx.tx(), container.id, &reason, &by, ctx.now())?;
    ctx.audit(
        clinical(actions::DISPOSE_CONTAINER)
            .record("container", container.id)
            .old_revision(container.revision)
            .new_revision(container.revision + 1)
            .reason(reason),
    )?;
    accession_of(ctx, container.id)
}

/// Loads a container for a change and checks the caller's revision.
fn load(
    ctx: &CommandContext<'_>,
    id: ContainerId,
    expected_revision: i64,
) -> Result<ContainerRow, SpecimenError> {
    let container = repo::container(ctx.tx(), id)?.ok_or(SpecimenError::NotFound("container"))?;
    ensure_revision(container.revision, expected_revision)?;
    Ok(container)
}

fn change_state(
    ctx: &CommandContext<'_>,
    container: &ContainerRow,
    state: ContainerState,
) -> Result<(), SpecimenError> {
    Ok(repo::update_state(
        ctx.tx(),
        container.id,
        state,
        container.revision + 1,
    )?)
}

fn accession_of(
    ctx: &CommandContext<'_>,
    container_id: ContainerId,
) -> Result<AccessionDetail, SpecimenError> {
    let accession_id = queries::container_accession(ctx.tx(), container_id)?;
    queries::get_accession(ctx.tx(), accession_id)
}
