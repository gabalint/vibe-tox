//! Outbox consumers for durable receipt import: ASTM messages and result
//! files (REQ-ANA-003, REQ-INT-003, REQ-INT-005, NFR-02). Artifact I/O and
//! parsing happen outside a database transaction; one typed `import_receipt`
//! write applies the semantic effect. The reader follows the registration: a
//! file template reads the bytes as a result file (result files brief §5).

use std::str::FromStr;
use std::sync::Arc;

use lis_analytical::SourceRegistrationView;
use lis_core::{Capability, CorrelationId, ErrorCode, Principal};
use lis_interfaces::{
    InstrumentTransport, RECEIPT_COMPLETED_TOPIC, Receipt, ReceiptId, ReceiptState,
};
use lis_results::astm::{SampleIdField, SourceProfile};
use lis_results::{ImportReceiptInput, ImportSource, ResultError};
use lis_store::outbox::LeasedItem;
use lis_store::{ArtifactDigest, Database};

use crate::outbox_worker::{HandlerError, TopicHandler};

const COMPONENT: &str = "result-import";

/// Built-in receipt-completion and reviewer-retry handlers.
pub(crate) fn handlers(db: Arc<Database>) -> [TopicHandler; 2] {
    [
        handler(Arc::clone(&db), RECEIPT_COMPLETED_TOPIC, false),
        handler(db, lis_results::RETRY_IMPORT_TOPIC, true),
    ]
}

fn handler(db: Arc<Database>, topic: &'static str, retry: bool) -> TopicHandler {
    TopicHandler {
        topic,
        handle: Arc::new(move |item| Box::pin(process(Arc::clone(&db), item, retry))),
    }
}

async fn process(db: Arc<Database>, item: LeasedItem, retry: bool) -> Result<(), HandlerError> {
    let receipt_id = item
        .payload
        .get("receipt_id")
        .and_then(|value| value.as_str())
        .and_then(|text| text.parse::<ReceiptId>().ok())
        .ok_or(HandlerError::Permanent("invalid_payload"))?;
    let attempt_no = if retry {
        item.payload
            .get("attempt_no")
            .and_then(|value| value.as_i64())
            .filter(|attempt| *attempt >= 1)
            .ok_or(HandlerError::Permanent("invalid_payload"))?
    } else {
        1
    };
    let principal = Principal::system(COMPONENT, [Capability::InstrumentIntake]);
    let receipt = db
        .read({
            let principal = principal.clone();
            move |conn| lis_interfaces::get_receipt(conn, &principal, receipt_id)
        })
        .await
        .map_err(interface_failure)?;
    if receipt.state != ReceiptState::Complete || receipt.duplicate_of.is_some() {
        return Ok(());
    }
    let registration = db
        .read(move |conn| lis_analytical::active_registration(conn, receipt.instrument_id))
        .await
        .map_err(result_dependency_failure)?;
    let source = read_source(&db, &receipt, registration.as_ref()).await?;
    let instrument_id = receipt.instrument_id;
    db.write(principal, CorrelationId::generate(), move |ctx| {
        lis_results::import_receipt(
            ctx,
            ImportReceiptInput {
                receipt_id,
                attempt_no,
                instrument_id,
                registration,
                source,
            },
        )
    })
    .await
    .map(drop)
    .map_err(import_failure)
}

/// Reads exact bytes with the frozen AU profile, approved file template, or
/// ASTM source profile selected by the receipt transport. A file
/// receipt without a file registration is not read (`unregistered_source`).
async fn read_source(
    db: &Arc<Database>,
    receipt: &Receipt,
    registration: Option<&SourceRegistrationView>,
) -> Result<ImportSource, HandlerError> {
    let layout = registration.and_then(|item| item.file_layout.clone());
    if layout.is_none() && receipt.transport == InstrumentTransport::ResultFile {
        return Ok(ImportSource::UnregisteredFile);
    }
    let digest = receipt
        .artifact_digest
        .as_deref()
        .ok_or(HandlerError::Permanent("receipt_artifact_missing"))?;
    let digest = ArtifactDigest::from_str(digest)
        .map_err(|_| HandlerError::Permanent("receipt_digest_invalid"))?;
    let read_db = Arc::clone(db);
    let bytes = tokio::task::spawn_blocking(move || read_db.artifacts().read(&digest))
        .await
        .map_err(|_| HandlerError::Retry("artifact_task_failed"))?
        .map_err(|_| HandlerError::Retry("artifact_unavailable"))?;
    if receipt.transport.is_au() {
        let receipt_id = receipt.id;
        let profile = db
            .read(move |conn| {
                lis_interfaces::receipt_au_profile(
                    conn,
                    &Principal::system(COMPONENT, [Capability::InstrumentIntake]),
                    receipt_id,
                )
            })
            .await
            .map_err(interface_failure)?
            .ok_or(HandlerError::Permanent("au_profile_missing"))?;
        let extracted = lis_results::au::extract_artifact(&bytes, &profile);
        return Ok(ImportSource::Au { profile, extracted });
    }
    if let Some(layout) = layout {
        return Ok(ImportSource::File(lis_results::delimited::extract_file(
            &bytes, &layout,
        )));
    }
    let sample_id_field = match registration.map(|item| item.sample_id_field) {
        Some(lis_analytical::SampleIdField::InstrumentSpecimenId) => {
            SampleIdField::InstrumentSpecimenId
        }
        Some(lis_analytical::SampleIdField::SpecimenId) | None => SampleIdField::SpecimenId,
    };
    Ok(
        match lis_results::astm::extract_artifact(&bytes, &SourceProfile { sample_id_field }) {
            Ok(extracted) => ImportSource::Extracted(extracted),
            Err(error) => ImportSource::Malformed(error),
        },
    )
}

fn interface_failure(error: lis_interfaces::InterfaceError) -> HandlerError {
    if error.code() == ErrorCode::Internal {
        HandlerError::Retry("receipt_read_failed")
    } else {
        HandlerError::Permanent("receipt_unavailable")
    }
}

fn result_dependency_failure(error: lis_analytical::AnalyticalError) -> HandlerError {
    if error.code() == ErrorCode::Internal {
        HandlerError::Retry("registration_read_failed")
    } else {
        HandlerError::Permanent("registration_unavailable")
    }
}

fn import_failure(error: ResultError) -> HandlerError {
    match error.code() {
        ErrorCode::Internal => HandlerError::Retry("result_import_failed"),
        ErrorCode::Conflict => HandlerError::Retry("result_registration_changed"),
        _ => HandlerError::Permanent("result_import_refused"),
    }
}
