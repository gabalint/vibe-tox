//! State-changing use cases. Each checks its capability first, validates its
//! input, rechecks its gates inside the transaction, changes state and
//! appends a clinical-stream audit event in the same transaction.

pub(crate) mod accessioning;
pub(crate) mod collection;
pub(crate) mod configuration;
pub(crate) mod containers;
pub(crate) mod floors;
pub(crate) mod holds;
pub(crate) mod join;
pub(crate) mod labels;
pub(crate) mod receipt;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Actor, Capability};
use lis_store::CommandContext;

use crate::error::SpecimenError;

const LABEL_PRINTER_COMPONENT: &str = "label-printer";

/// Human specimen work must stay attributable to a signed-in person.
fn require_user(ctx: &mut CommandContext<'_>, capability: Capability) -> Result<(), SpecimenError> {
    ctx.require(capability)?;
    match ctx.principal().actor() {
        Actor::User { .. } => Ok(()),
        Actor::Instrument { .. } | Actor::System { .. } => Err(SpecimenError::ActorNotAllowed),
    }
}

/// Only the label delivery worker may record printer outcomes.
fn require_label_printer(ctx: &mut CommandContext<'_>) -> Result<(), SpecimenError> {
    ctx.require(Capability::Accession)?;
    match ctx.principal().actor() {
        Actor::System { component } if *component == LABEL_PRINTER_COMPONENT => Ok(()),
        Actor::User { .. } | Actor::Instrument { .. } | Actor::System { .. } => {
            Err(SpecimenError::ActorNotAllowed)
        }
    }
}

/// The stored actor text of the command's principal.
fn actor(ctx: &CommandContext<'_>) -> String {
    ctx.principal().actor().id_text()
}

/// A succeeded clinical-stream audit event.
fn clinical(action: &'static str) -> AuditEvent {
    AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
}
