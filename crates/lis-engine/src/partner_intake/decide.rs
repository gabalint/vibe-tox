//! What one read message becomes, decided and recorded in one transaction with the order
//! it makes (REQ-INT-003): an exact replay is a duplicate with no effect; a message that
//! cannot be read, comes from another sending facility or names an unknown test is
//! rejected; the same message ID with other content is held for a person; otherwise it
//! becomes an order in Received status, or waits for its patient.

use lis_interfaces::partner::hl7_order::read_order_message;
use lis_interfaces::partner::{
    IntakeSource, NewReceived, PartnerReceivedId, ReceivedOutcome, UnknownPatient, WaitingMessage,
};
use lis_store::CommandContext;

use super::IntakeError;
use super::order_input::{Problem, order_input};

/// One message as read, with its stored bytes' digest: a file, or a message received over
/// MLLP, which has no name and comes from a partner's address.
pub(crate) struct ReadFile {
    pub(crate) name: String,
    pub(crate) sha256: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) peer: Option<String>,
}

/// The order's source document: the stored original message, by digest.
pub(super) fn source_ref(sha256: &str) -> String {
    format!("hl7-message:sha256:{sha256}")
}

/// Decides and records `file`; a new order is made in the same transaction.
pub(super) fn receive(
    ctx: &mut CommandContext<'_>,
    source: &IntakeSource,
    file: &ReadFile,
) -> Result<(PartnerReceivedId, ReceivedOutcome), IntakeError> {
    let connection_id = source.connection_id;
    if let Some(earlier) =
        lis_interfaces::partner::earlier_read(ctx.tx(), connection_id, &file.sha256)?
    {
        let outcome = ReceivedOutcome::Duplicate { of: earlier.id };
        return record(ctx, source, file, (None, None), outcome);
    }
    let message = match read_order_message(&file.bytes) {
        Ok(message) => message,
        Err(problem) => {
            let outcome = ReceivedOutcome::Rejected {
                reason: problem.code(),
            };
            return record(ctx, source, file, (None, None), outcome);
        }
    };
    let keys = (
        Some(message.control_id.clone()),
        Some(message.placer_order.clone()),
    );
    let outcome = if message.sending_facility != source.intake.accept_from {
        ReceivedOutcome::Rejected {
            reason: "not_accepted_sender",
        }
    } else if lis_interfaces::partner::control_id_seen(
        ctx.tx(),
        connection_id,
        &message.control_id,
    )? {
        ReceivedOutcome::Held {
            reason: "changed_content",
        }
    } else {
        match order_input(ctx.tx(), source, &message, &source_ref(&file.sha256))? {
            Ok(input) => ReceivedOutcome::OrderCreated {
                order_id: lis_orders::register_order(ctx, input)?.order_id.to_string(),
            },
            Err(problem) => unordered(source, problem),
        }
    };
    record(ctx, source, file, keys, outcome)
}

/// A message that is not an order yet: its patient may still be registered, or a person
/// decides, or it is refused.
fn unordered(source: &IntakeSource, problem: Problem) -> ReceivedOutcome {
    match problem {
        Problem::UnknownPatient => match source.intake.unknown_patient {
            UnknownPatient::Hold => ReceivedOutcome::WaitingPatient {
                reason: "patient_unknown",
            },
            UnknownPatient::Reject => ReceivedOutcome::Rejected {
                reason: "patient_unknown",
            },
        },
        Problem::Held(reason) => ReceivedOutcome::Held { reason },
        Problem::Rejected(reason) => ReceivedOutcome::Rejected { reason },
    }
}

fn record(
    ctx: &mut CommandContext<'_>,
    source: &IntakeSource,
    file: &ReadFile,
    (control_id, placer_order): (Option<String>, Option<String>),
    outcome: ReceivedOutcome,
) -> Result<(PartnerReceivedId, ReceivedOutcome), IntakeError> {
    let id = lis_interfaces::partner::record_received(
        ctx,
        NewReceived {
            connection_id: source.connection_id,
            file_name: file.name.clone(),
            artifact_sha256: file.sha256.clone(),
            control_id,
            placer_order,
            outcome: outcome.clone(),
            peer: file.peer.clone(),
        },
    )?;
    Ok((id, outcome))
}

/// Tries a waiting message again: once its patient is registered it becomes an order;
/// details that differ hold it for a person; a test no longer mapped rejects it. `None`
/// while its patient is still unknown.
pub(super) fn retry_waiting(
    ctx: &mut CommandContext<'_>,
    source: &IntakeSource,
    waiting: &WaitingMessage,
    bytes: &[u8],
) -> Result<Option<ReceivedOutcome>, IntakeError> {
    let outcome = match read_order_message(bytes) {
        Err(problem) => ReceivedOutcome::Rejected {
            reason: problem.code(),
        },
        Ok(message) => {
            let source_ref = source_ref(&waiting.artifact_sha256);
            match order_input(ctx.tx(), source, &message, &source_ref)? {
                Ok(input) => ReceivedOutcome::OrderCreated {
                    order_id: lis_orders::register_order(ctx, input)?.order_id.to_string(),
                },
                Err(Problem::UnknownPatient) => return Ok(None),
                Err(problem) => unordered(source, problem),
            }
        }
    };
    lis_interfaces::partner::resolve_waiting(ctx, waiting.id, outcome.clone())?;
    Ok(Some(outcome))
}
