//! The `spc.label.print` outbox handler (REQ-SPC-001, NFR-02).
//!
//! For each leased job it reads the label request on a read connection,
//! renders and sends the label on a blocking thread outside any transaction,
//! then records the outcome as the `label-printer` system principal, which
//! holds only `accession`. `handed_off` means the bytes reached the printer,
//! not that a label printed. A job whose outcome is already recorded is not
//! sent again; a crash between sending and recording can still print a
//! label twice (at-least-once). A failed hand-off is a recorded outcome, not
//! a retry; only database failures retry.

use std::sync::Arc;

use lis_core::{Capability, CorrelationId, ErrorCode, Principal};
use lis_labels::{Marking, PrinterDestinations};
use lis_specimens::{LABEL_PRINT_TOPIC, LabelOutcome, SpecimenError};
use lis_store::Database;
use lis_store::outbox::LeasedItem;

use crate::outbox_worker::{HandlerError, TopicHandler};

/// The worker's principal.
const COMPONENT: &str = "label-printer";

/// The handler of [`LABEL_PRINT_TOPIC`], sending only to printers
/// `destinations` admits, with the installation's label `marking`.
pub(crate) fn handler(
    db: Arc<Database>,
    destinations: PrinterDestinations,
    marking: Marking,
) -> TopicHandler {
    TopicHandler {
        topic: LABEL_PRINT_TOPIC,
        handle: Arc::new(move |item| Box::pin(print(Arc::clone(&db), item, destinations, marking))),
    }
}

async fn print(
    db: Arc<Database>,
    item: LeasedItem,
    destinations: PrinterDestinations,
    marking: Marking,
) -> Result<(), HandlerError> {
    let job_id = lis_specimens::label_job_id(&item.payload)
        .ok_or(HandlerError::Permanent("invalid_payload"))?;
    let request = db
        .read(move |conn| lis_specimens::label_request(conn, job_id))
        .await
        .map_err(|error| failure(&error))?;
    let Some(request) = request else {
        return Ok(());
    };
    let delivery = tokio::task::spawn_blocking(move || {
        lis_specimens::deliver_label(&request, destinations, marking)
    })
    .await
    .map_err(|_| HandlerError::Retry("label_task_failed"))?;
    if delivery.outcome == LabelOutcome::Failed {
        tracing::warn!(
            label_job_id = %job_id,
            error_code = delivery.error_code,
            "label not handed to the printer"
        );
    }
    let recorded = db
        .write(
            Principal::system(COMPONENT, [Capability::Accession]),
            CorrelationId::generate(),
            move |ctx| lis_specimens::record_label_outcome(ctx, delivery),
        )
        .await;
    match recorded {
        Ok(()) | Err(SpecimenError::OutcomeRecorded) => Ok(()),
        Err(error) => Err(failure(&error)),
    }
}

/// A missing job never appears later; anything else may be transient.
fn failure(error: &SpecimenError) -> HandlerError {
    if error.code() == ErrorCode::NotFound {
        HandlerError::Permanent("label_job_not_found")
    } else {
        HandlerError::Retry("label_outcome_not_recorded")
    }
}
