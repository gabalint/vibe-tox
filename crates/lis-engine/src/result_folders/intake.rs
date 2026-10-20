//! Receiving a ready file from a watched folder as the instrument (F13, F14, F16;
//! REQ-INT-009): the instrument is read before any bytes are stored, the bytes are
//! durable before the receipt, and the receipt transaction rechecks the folder.

use std::path::Path;

use lis_core::{CorrelationId, ErrorCode, InstrumentId, Phi, Principal};
use lis_interfaces::{
    FileFacts, FileSource, InterfaceError, MAX_RESULT_FILE_BYTES, Receipt, ReceiveResultFileInput,
};

use super::ResultFolders;
use super::files::{Candidate, read_unchanged};

/// What happened to a ready file.
pub(super) enum Taken {
    /// It has a receipt now, or had one already, or was in the folder when it was
    /// chosen; `too_large` when this pickup recorded it as over the size limit.
    Received { too_large: bool },
    /// It changed while it was read, or could not be opened yet.
    Changed,
    /// The receipt failed and is tried again after a back-off.
    Retry(&'static str),
    /// The receipt was refused and is not tried again until the file changes.
    Refused(&'static str),
}

impl ResultFolders {
    /// Whether the file is in the ledger of files received, or was in `folder` when
    /// it was chosen for the instrument (§6): either way it is not picked up.
    pub(super) fn known(
        &self,
        instrument_id: InstrumentId,
        folder: &str,
        candidate: &Candidate,
    ) -> bool {
        let (folder, name) = (folder.to_owned(), candidate.name.clone());
        let (byte_len, modified_at) = (candidate.byte_len, candidate.modified_at);
        self.db
            .read_blocking(move |conn| {
                Ok::<_, InterfaceError>(
                    lis_interfaces::seen_file(conn, instrument_id, &name, byte_len, modified_at)?
                        .is_some()
                        || lis_interfaces::present_at_selection(
                            conn,
                            instrument_id,
                            &folder,
                            &name,
                            byte_len,
                            modified_at,
                        )?,
                )
            })
            .unwrap_or(false)
    }

    /// Receives a ready file as the instrument (F16), once a read shows the instrument
    /// still enabled, still result files and still watching `folder`: too large, not read
    /// (F14); else its bytes durable in the artifact store first, then the receipt
    /// (REQ-INT-009), whose transaction checks the folder again.
    pub(super) fn take(
        &self,
        instrument_id: InstrumentId,
        folder: &str,
        candidate: &Candidate,
    ) -> Taken {
        let watching = self
            .db
            .read_blocking(move |conn| lis_interfaces::receivable_instrument(conn, instrument_id));
        match watching {
            Ok(instrument) if instrument.result_folder.as_deref() == Some(folder) => {}
            Ok(_) => return taken(Err(InterfaceError::ResultFolderChanged), false),
            Err(error) => return taken(Err(error), false),
        }
        let file = FileFacts {
            instrument_id,
            source: FileSource::Folder(folder.to_owned()),
            file_name: Phi::new(candidate.name.clone()),
            byte_len: candidate.byte_len,
            modified_at: candidate.modified_at,
        };
        let principal = Principal::instrument(instrument_id);
        if candidate.byte_len > MAX_RESULT_FILE_BYTES {
            let recorded =
                self.db
                    .write_blocking(principal, CorrelationId::generate(), move |ctx| {
                        lis_interfaces::record_oversize_file(ctx, file)
                    });
            return taken(recorded, true);
        }
        let Some(bytes) = read_unchanged(&Path::new(folder).join(&candidate.name), candidate)
        else {
            return Taken::Changed;
        };
        let Ok(artifact) = self.db.artifacts().put(&bytes) else {
            return Taken::Retry("artifact_not_stored");
        };
        let digest = artifact.digest.to_string();
        let received = self
            .db
            .write_blocking(principal, CorrelationId::generate(), move |ctx| {
                lis_interfaces::receive_result_file(
                    ctx,
                    ReceiveResultFileInput {
                        file,
                        sha256: digest.clone(),
                        artifact_digest: digest,
                    },
                )
            });
        taken(received, false)
    }
}

/// A file already received or present at selection needs nothing more. A refusal
/// that a later scan cannot change is not retried: invalid facts, or a principal the
/// domain refuses. Anything else, such as a configuration change or a database failure,
/// is tried again after a back-off. `too_large` marks a new oversize receipt.
fn taken(result: Result<Receipt, InterfaceError>, too_large: bool) -> Taken {
    match result {
        Ok(_) => Taken::Received { too_large },
        Err(InterfaceError::FileAlreadyReceived | InterfaceError::FilePresentAtSelection) => {
            Taken::Received { too_large: false }
        }
        Err(error) => match error.code() {
            code @ (ErrorCode::Validation | ErrorCode::Forbidden) => Taken::Refused(code.as_str()),
            code => Taken::Retry(code.as_str()),
        },
    }
}
