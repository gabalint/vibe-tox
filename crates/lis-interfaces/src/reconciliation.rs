use lis_astm::{Action, EOT, ReceiveDriver, ReceiveEvent};
use lis_core::{Capability, CorrelationId, Principal};
use lis_store::Database;

use crate::commands::{complete_recovered_receipt, interrupt_receipt};
use crate::model::ReceiptFrame;
use crate::session::assembled_message;
use crate::{InterfaceError, repo};

/// Outcome of one blocking startup receipt reconciliation pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ReconciliationSummary {
    /// Complete H…L messages recovered from durable frames.
    pub recovered: u32,
    /// Ambiguous or incomplete transfers left visible as interrupted.
    pub interrupted: u32,
}

/// Rebuilds complete durable transfers and interrupts ambiguous ones at startup.
///
/// This blocking function must run before listeners start (REQ-OPS-013).
pub fn reconcile_receipts(database: &Database) -> Result<ReconciliationSummary, InterfaceError> {
    let receipt_ids = database.read_blocking(repo::receiving_receipts)?;
    let mut summary = ReconciliationSummary {
        recovered: 0,
        interrupted: 0,
    };
    for receipt_id in receipt_ids {
        let frames =
            database.read_blocking(move |conn| repo::frames_for_receipt(conn, receipt_id))?;
        let profile = database.read_blocking(move |conn| repo::au_profile(conn, receipt_id))?;
        let message = match profile {
            Some(profile) => crate::au_evidence::recoverable(&profile, &frames),
            None => recoverable_message(&frames),
        };
        if let Some(message) = message {
            let artifact = database.artifacts().put(&message)?;
            database.write_blocking(
                reconciliation_principal(),
                CorrelationId::generate(),
                move |ctx| complete_recovered_receipt(ctx, receipt_id, artifact),
            )?;
            summary.recovered = increment(summary.recovered)?;
        } else {
            database.write_blocking(
                reconciliation_principal(),
                CorrelationId::generate(),
                move |ctx| interrupt_receipt(ctx, receipt_id),
            )?;
            summary.interrupted = increment(summary.interrupted)?;
        }
    }
    Ok(summary)
}

fn recoverable_message(frames: &[ReceiptFrame]) -> Option<Vec<u8>> {
    let mut driver = ReceiveDriver::default();
    driver
        .handle(ReceiveEvent::Bytes(vec![lis_astm::ENQ]))
        .ok()?;
    for (ordinal, stored) in frames.iter().enumerate() {
        if stored.ordinal != u32::try_from(ordinal).ok()? {
            return None;
        }
        let actions = driver
            .handle(ReceiveEvent::Bytes(stored.bytes.clone()))
            .ok()?;
        let frame_id = actions.iter().find_map(Action::persistence_id)?;
        let persisted = driver.handle(ReceiveEvent::Persisted(frame_id)).ok()?;
        if !persisted.contains(&Action::Send(lis_astm::ControlResponse::Ack)) {
            return None;
        }
    }
    let complete = driver.handle(ReceiveEvent::Bytes(vec![EOT])).ok()?;
    let records = complete.into_iter().find_map(|action| match action {
        Action::MessageComplete(records) => Some(records),
        Action::Send(_)
        | Action::PersistFrame { .. }
        | Action::TransferAborted(_)
        | Action::StartTimer { .. }
        | Action::StopTimer => None,
    })?;
    let message = assembled_message(records);
    let records = lis_astm::parse_message_records(&message).ok()?;
    lis_astm::assemble_message(records).ok()?;
    Some(message)
}

fn increment(value: u32) -> Result<u32, InterfaceError> {
    value
        .checked_add(1)
        .ok_or(InterfaceError::InvalidReceiptState)
}

fn reconciliation_principal() -> Principal {
    Principal::system(
        "interface-startup-reconciliation",
        [Capability::InstrumentIntake],
    )
}
