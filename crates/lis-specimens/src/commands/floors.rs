//! Number floors (REQ-OPS-002; M4 design brief §2.2, §3.8): a quality manager raises
//! them by hand, and recording a restore raises them by a fixed skip, so numbers the
//! lost interval may have put on physical labels are never issued again.

use lis_audit::{AuditDetail, AuditEvent};
use lis_core::{Actor, Capability, Id, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{actor, clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, invalid};
use crate::input;
use crate::numbering::{self, Position, Sequence};

/// Marker type of number floor IDs.
pub enum NumberFloor {}
/// Identifies one raised floor.
pub type NumberFloorId = Id<NumberFloor>;

/// Input of [`raise_number_floors`]: at least one floor, and why.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RaiseNumberFloorsInput {
    /// The new accession number floor, such as `A26004000`.
    #[serde(default)]
    #[ts(optional)]
    pub accession_floor: Option<String>,
    /// The new container code floor, such as `C00020000`.
    #[serde(default)]
    #[ts(optional)]
    pub container_floor: Option<String>,
    /// Why, for example the highest number found on a physical label.
    pub reason: String,
}

/// Where both sequences stand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NumberFloors {
    /// Accession numbers of the current year.
    pub accession: NumberPosition,
    /// Container codes.
    pub container: NumberPosition,
}

/// Where one sequence stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NumberPosition {
    /// The highest number issued.
    pub highest_issued: Option<String>,
    /// The highest floor.
    pub floor: Option<String>,
    /// The number the next allocation issues.
    pub next: String,
}

impl From<Position> for NumberPosition {
    fn from(position: Position) -> Self {
        Self {
            highest_issued: position.highest_issued,
            floor: position.floor,
            next: position.next,
        }
    }
}

/// Where accession numbers (for the year of `now`) and container codes stand.
pub fn number_floors(conn: &Connection, now: UtcTimestamp) -> Result<NumberFloors, SpecimenError> {
    Ok(NumberFloors {
        accession: numbering::position(conn, Sequence::Accession, now)?.into(),
        container: numbering::position(conn, Sequence::Container, now)?.into(),
    })
}

/// Raises the accession and container number floors by hand (`manage_quality`; one
/// `specimen.raise_number_floor` event per floor). A floor must be in the sequence's
/// format and above both the highest issued number and the highest floor of its prefix.
pub fn raise_number_floors(
    ctx: &mut CommandContext<'_>,
    input: RaiseNumberFloorsInput,
) -> Result<NumberFloors, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    let requested = [
        (Sequence::Accession, "accessionFloor", input.accession_floor),
        (Sequence::Container, "containerFloor", input.container_floor),
    ];
    let mut floors = Vec::new();
    for (sequence, field, text) in requested {
        let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
            continue;
        };
        let (floor, prefix) = numbering::parse_floor(sequence, field, &text)?;
        let position = numbering::position_for_prefix(ctx.tx(), sequence, &prefix)?;
        if floor < position.next {
            return Err(invalid(
                field,
                "not_above",
                "A floor only goes up: enter a number above the highest issued number and the current floor.",
            ));
        }
        floors.push((sequence, floor));
    }
    if floors.is_empty() {
        return Err(invalid(
            "accessionFloor",
            "required",
            "Enter an accession number floor, a container code floor, or both.",
        ));
    }
    for (sequence, floor) in floors {
        record(ctx, sequence, &floor, "manual", Some(&reason))?;
    }
    number_floors(ctx.tx(), ctx.now())
}

/// Raises both floors `accession_skip` and `container_skip` numbers above where they
/// stand, when a restore is recorded (system principal with `manage_quality`).
pub fn raise_number_floors_after_restore(
    ctx: &mut CommandContext<'_>,
    accession_skip: u32,
    container_skip: u32,
) -> Result<NumberFloors, SpecimenError> {
    ctx.require(Capability::ManageQuality)?;
    if !matches!(ctx.principal().actor(), Actor::System { .. }) {
        return Err(SpecimenError::ActorNotAllowed);
    }
    let skips = [
        (Sequence::Accession, accession_skip),
        (Sequence::Container, container_skip),
    ];
    for (sequence, skip) in skips {
        let floor = numbering::floor_after(ctx.tx(), sequence, ctx.now(), skip)?;
        record(ctx, sequence, &floor, "restore", None)?;
    }
    number_floors(ctx.tx(), ctx.now())
}

fn record(
    ctx: &CommandContext<'_>,
    sequence: Sequence,
    floor: &str,
    source: &'static str,
    reason: Option<&str>,
) -> Result<(), SpecimenError> {
    let id = NumberFloorId::generate();
    numbering::insert_floor(
        ctx.tx(),
        &id.to_string(),
        sequence,
        floor,
        source,
        (&actor(ctx), ctx.now()),
    )?;
    let mut event: AuditEvent = clinical(actions::RAISE_NUMBER_FLOOR)
        .record("number_floor", id)
        .new_revision(1)
        .detail("kind", AuditDetail::code(sequence.as_str()))
        .detail("source", AuditDetail::code(source))
        .detail(
            "floor",
            // Machine tokens are lowercase; the floor keeps its digits and prefix.
            AuditDetail::machine_token(&floor.to_ascii_lowercase())
                .map_err(lis_store::StoreError::from)?,
        );
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    Ok(())
}
