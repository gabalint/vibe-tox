//! Accessioning (REQ-SPC-001, REQ-SPC-002, REQ-SPC-008).

use std::collections::BTreeSet;

use lis_audit::AuditDetail;
use lis_core::{Capability, DecimalText};
use lis_identity::SubjectId;
use lis_orders::{OrderId, OrderItemId};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{actor, clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, invalid};
use crate::input::{
    self, CODE_MAX, CollectionTime, CollectionTimeInput, NAME_MAX, TEXT_MAX, Volume,
};
use crate::model::{AccessionId, CollectionId, ContainerId, SpecimenId, VolumeKind};
use crate::numbering;
use crate::queries;
use crate::repo::accessions::{self as repo, NewAccession, NewCollection, NewObservation};
use crate::repo::containers::{NewContainer, insert_container};
use crate::views::AccessionDetail;

/// Most specimens, containers per specimen and observations in one accession.
const MAX_SPECIMENS: usize = 10;
const MAX_CONTAINERS: u8 = 10;
const MAX_OBSERVATIONS: usize = 20;

/// Input of [`create_accession`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateAccessionInput {
    /// The accepted order the material is for.
    pub order_id: OrderId,
    /// The accepted items of that order this accession fulfils.
    pub order_item_ids: Vec<OrderItemId>,
    /// The collection event.
    pub collection: CollectionInput,
    /// The specimens received, one per matrix.
    pub specimens: Vec<SpecimenInput>,
}

/// The collection event, as supplied by the collection site.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionInput {
    /// When the specimen was collected; unknown stays explicit.
    pub time: CollectionTimeInput,
    /// Who collected, as supplied.
    #[serde(default)]
    #[ts(optional)]
    pub collector: Option<String>,
    /// Collection-site observations (REQ-SPC-008).
    #[serde(default)]
    pub observations: Vec<CollectionObservationInput>,
}

/// One collection-site observation, attributed to its observer.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionObservationInput {
    /// Lab-defined observation code, such as `temperature_in_range`.
    pub code: String,
    /// The value as reported.
    pub value_text: String,
    /// Who observed it, as reported.
    pub observed_by: String,
}

/// One specimen and its primary containers.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SpecimenInput {
    /// Its matrix; must be one the accessioned items require.
    pub specimen_type_code: String,
    /// Primary containers received, 1–10. Each gets its own label code.
    pub container_count: u8,
    /// Volume per container, if known.
    #[serde(default)]
    #[ts(optional)]
    pub volume: Option<DecimalText>,
    /// Unit text of the volume.
    #[serde(default)]
    #[ts(optional)]
    pub volume_unit: Option<String>,
    /// Estimated, measured or unknown.
    pub volume_kind: VolumeKind,
}

struct ParsedSpecimen {
    specimen_type_code: String,
    container_count: u8,
    volume: Volume,
}

struct ParsedObservation {
    code: String,
    value_text: String,
    observed_by: String,
}

/// Creates an accession with its collection, specimens and primary
/// containers (REQ-SPC-001). The order items must be accepted, with no open
/// clarification hold, and not yet accessioned; this is rechecked here,
/// inside the transaction. Specimen matrices must match the items' matrices.
pub fn create_accession(
    ctx: &mut CommandContext<'_>,
    input: CreateAccessionInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let time = input::collection_time(&input.collection.time, ctx.now())?;
    let collector = input::optional_text(
        "collection.collector",
        input.collection.collector.as_deref(),
        NAME_MAX,
    )?;
    let observations = parse_observations(&input.collection.observations)?;
    let specimens = parse_specimens(&input.specimens)?;
    let subject_id = check_order_items(ctx, &input, &specimens)?;
    let id = AccessionId::generate();
    let number = numbering::next_accession_number(ctx.tx(), ctx.now())?;
    let by = actor(ctx);
    repo::insert_accession(
        ctx.tx(),
        &NewAccession {
            id,
            number: &number,
            order_id: input.order_id,
            subject_id,
            by: &by,
            at: ctx.now(),
        },
    )?;
    for item_id in &input.order_item_ids {
        repo::insert_accession_item(ctx.tx(), id, *item_id)?;
    }
    let collection_id = insert_collection(ctx, id, &time, collector.as_deref(), &observations)?;
    let containers = insert_specimens(ctx, collection_id, &specimens)?;
    ctx.audit(
        clinical(actions::CREATE_ACCESSION)
            .record("accession", id)
            .detail("order", AuditDetail::id(input.order_id))
            .detail(
                "order_items",
                AuditDetail::count(input.order_item_ids.len()),
            )
            .detail("specimens", AuditDetail::count(specimens.len()))
            .detail("containers", AuditDetail::count(containers)),
    )?;
    queries::get_accession(ctx.tx(), id)
}

fn parse_observations(
    inputs: &[CollectionObservationInput],
) -> Result<Vec<ParsedObservation>, SpecimenError> {
    if inputs.len() > MAX_OBSERVATIONS {
        return Err(invalid(
            "collection.observations",
            "too_many",
            "Record at most 20 observations.",
        ));
    }
    inputs
        .iter()
        .map(|observation| {
            Ok(ParsedObservation {
                code: input::code("collection.observations.code", &observation.code)?,
                value_text: input::required_text(
                    "collection.observations.valueText",
                    &observation.value_text,
                    TEXT_MAX,
                )?,
                observed_by: input::required_text(
                    "collection.observations.observedBy",
                    &observation.observed_by,
                    NAME_MAX,
                )?,
            })
        })
        .collect()
}

fn parse_specimens(inputs: &[SpecimenInput]) -> Result<Vec<ParsedSpecimen>, SpecimenError> {
    if inputs.is_empty() || inputs.len() > MAX_SPECIMENS {
        return Err(invalid(
            "specimens",
            "count",
            "Enter from 1 to 10 specimens.",
        ));
    }
    inputs
        .iter()
        .map(|specimen| {
            if !(1..=MAX_CONTAINERS).contains(&specimen.container_count) {
                return Err(invalid(
                    "specimens.containerCount",
                    "count",
                    "Enter from 1 to 10 containers.",
                ));
            }
            Ok(ParsedSpecimen {
                specimen_type_code: input::required_text(
                    "specimens.specimenTypeCode",
                    &specimen.specimen_type_code,
                    CODE_MAX,
                )?,
                container_count: specimen.container_count,
                volume: input::volume(
                    "specimens.volume",
                    specimen.volume.as_ref(),
                    specimen.volume_unit.as_deref(),
                    specimen.volume_kind,
                )?,
            })
        })
        .collect()
}

/// The order-side gates, read inside this transaction (REQ-ORD-001): every
/// item is ready for accessioning and not yet accessioned, and the specimen
/// matrices are exactly the matrices the items require. Returns the order's subject.
fn check_order_items(
    ctx: &CommandContext<'_>,
    input: &CreateAccessionInput,
    specimens: &[ParsedSpecimen],
) -> Result<SubjectId, SpecimenError> {
    let requested: BTreeSet<OrderItemId> = input.order_item_ids.iter().copied().collect();
    if requested.is_empty() || requested.len() != input.order_item_ids.len() {
        return Err(invalid(
            "orderItemIds",
            "invalid_selection",
            "Select each order item once.",
        ));
    }
    let ready = lis_orders::items_ready_for_accession(ctx.tx(), input.order_id)?;
    let mut required = BTreeSet::new();
    for item_id in &requested {
        let matrices: Vec<&str> = ready
            .iter()
            .filter(|item| item.item_id == *item_id)
            .map(|item| item.specimen_type_code.as_str())
            .collect();
        if matrices.is_empty() {
            return Err(SpecimenError::OrderItemNotReady);
        }
        if repo::item_accessioned(ctx.tx(), *item_id)? {
            return Err(SpecimenError::ItemAccessioned);
        }
        required.extend(matrices);
    }
    let supplied: BTreeSet<&str> = specimens
        .iter()
        .map(|specimen| specimen.specimen_type_code.as_str())
        .collect();
    if supplied != required || supplied.len() != specimens.len() {
        return Err(invalid(
            "specimens",
            "matrix_mismatch",
            "Enter one specimen for each specimen type the selected tests require.",
        ));
    }
    repo::order_subject(ctx.tx(), input.order_id)?.ok_or(SpecimenError::NotFound("order"))
}

fn insert_collection(
    ctx: &CommandContext<'_>,
    accession_id: AccessionId,
    time: &CollectionTime,
    collector: Option<&str>,
    observations: &[ParsedObservation],
) -> Result<CollectionId, SpecimenError> {
    let id = CollectionId::generate();
    let by = actor(ctx);
    repo::insert_collection(
        ctx.tx(),
        &NewCollection {
            id,
            accession_id,
            time,
            collector,
            by: &by,
            at: ctx.now(),
        },
    )?;
    for observation in observations {
        repo::insert_observation(
            ctx.tx(),
            &NewObservation {
                collection_id: id,
                code: &observation.code,
                value_text: &observation.value_text,
                observed_by: &observation.observed_by,
                by: &by,
                at: ctx.now(),
            },
        )?;
    }
    Ok(id)
}

/// Inserts the specimens and their primary containers; returns the container count.
fn insert_specimens(
    ctx: &CommandContext<'_>,
    collection_id: CollectionId,
    specimens: &[ParsedSpecimen],
) -> Result<usize, SpecimenError> {
    let by = actor(ctx);
    let mut containers = 0;
    for specimen in specimens {
        let specimen_id = SpecimenId::generate();
        repo::insert_specimen(
            ctx.tx(),
            specimen_id,
            collection_id,
            &specimen.specimen_type_code,
            &by,
            ctx.now(),
        )?;
        for _ in 0..specimen.container_count {
            let label_code = numbering::next_container_code(ctx.tx())?;
            insert_container(
                ctx.tx(),
                &NewContainer {
                    id: ContainerId::generate(),
                    specimen_id,
                    label_code: &label_code,
                    parent: None,
                    volume: &specimen.volume,
                    by: &by,
                    at: ctx.now(),
                },
            )?;
            containers += 1;
        }
    }
    Ok(containers)
}
