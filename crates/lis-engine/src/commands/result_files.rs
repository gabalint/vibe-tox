//! Result files (REQ-INT-009; result-files brief §6, §9, F13, F14, F16): load a file by
//! hand, and choose or clear an instrument's watched folder. The shell opens the file and
//! folder dialogs in Rust and passes the chosen path; the UI never sends one. A cancelled
//! dialog answers `None` after the capability check. Choosing a folder records the files
//! already in it, which the poller then skips (§6, "Present at selection").

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lis_core::{Capability, CommandError, ErrorCode, InstrumentId, Phi, UtcTimestamp};
use lis_interfaces::{
    FileFacts, FileSource, Instrument, MAX_RESULT_FILE_BYTES, PresentFile, Receipt, ReceiptId,
    ReceiptState, ReceiveResultFileInput, SetResultFolder, WatchedFolder,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Call, Failure};
use crate::registry::{Access, CommandSpec, spec};
use crate::result_folders::{candidates, modified_at, open_for_reading};

const MANAGE: Access = Access::Capability(Capability::ManageConfiguration);

pub(crate) const LOAD_RESULT_FILE: CommandSpec = spec(
    "load_result_file",
    "interface.receive_result_file",
    Access::Capability(Capability::PerformAnalysis),
);
pub(crate) const CHOOSE_RESULT_FOLDER: CommandSpec = spec(
    "choose_result_folder",
    "interface.set_result_folder",
    MANAGE,
);
pub(crate) const CLEAR_RESULT_FOLDER: CommandSpec =
    spec("clear_result_folder", "interface.set_result_folder", MANAGE);

/// Input of `load_result_file`; the file comes from the shell's dialog.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct LoadResultFileInput {
    /// The result-file instrument whose software exported the file.
    pub instrument_id: InstrumentId,
}

/// Input of `choose_result_folder` and `clear_result_folder`; a chosen folder comes
/// from the shell's dialog.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ResultFolderInput {
    /// The result-file instrument.
    pub instrument_id: InstrumentId,
    /// Revision observed by the caller.
    #[ts(type = "number")]
    pub expected_revision: i64,
}

/// What became of a loaded file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LoadOutcome {
    /// Received; its import follows.
    Received,
    /// The same bytes were received before from this instrument; nothing is imported.
    /// After a template fix, use Retry import on the earlier receipt's attempt (F12).
    Duplicate,
    /// Over the size limit; not read, an aborted receipt (F14).
    TooLarge,
}

/// The receipt of a loaded file.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoadedResultFile {
    /// The new receipt.
    pub receipt_id: ReceiptId,
    /// The instrument it is attributed to.
    pub instrument_id: InstrumentId,
    /// When it was received.
    pub received_at: UtcTimestamp,
    /// Received, duplicate or too large.
    pub outcome: LoadOutcome,
    /// The earlier receipt of the same bytes, for a duplicate.
    pub duplicate_of: Option<ReceiptId>,
    /// The file's name.
    pub file_name: Phi<String>,
    /// Size in bytes.
    #[ts(type = "number")]
    pub byte_len: u64,
    /// SHA-256 of the bytes; absent when the file was too large to read.
    pub sha256: Option<String>,
}

/// A chosen watched folder: the instrument, and how many files already in it are left
/// for loading by hand (§6, "Present at selection").
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChosenResultFolder {
    /// The saved configuration.
    pub instrument: Instrument,
    /// Result files already in the folder; the folder pickup skips them.
    pub present_files: u32,
}

/// A file read from the path the dialog returned; `bytes` is `None` when it is too large.
struct ChosenFile {
    facts: FileFacts,
    bytes: Option<Vec<u8>>,
}

impl Engine {
    /// `load_result_file` (`perform_analysis`): stores the chosen file's exact bytes, then
    /// records the receipt as the user, which queues its import (F1, F16). A file over the
    /// limit is not read and becomes an aborted receipt (F14). `None` when cancelled.
    pub async fn load_result_file(
        &self,
        input: LoadResultFileInput,
        file: Option<PathBuf>,
    ) -> Result<Option<LoadedResultFile>, CommandError> {
        self.run(&LOAD_RESULT_FILE, |call| async move {
            // The capability is checked before a cancelled dialog is answered (NFR-31).
            self.authorize(&call).await?;
            let Some(path) = file else {
                return Ok(None);
            };
            let instrument_id = input.instrument_id;
            // Nothing is read or stored for an instrument that cannot receive files now.
            self.read(&call, move |conn| {
                lis_interfaces::receivable_instrument(conn, instrument_id)
            })
            .await?;
            let chosen = tokio::task::spawn_blocking(move || read_chosen(&path, instrument_id))
                .await
                .map_err(|_| Failure::Internal("result file read task stopped".to_owned()))??;
            let (file_name, byte_len) = (chosen.facts.file_name.clone(), chosen.facts.byte_len);
            let receipt = self.receive(&call, chosen).await?;
            Ok(Some(loaded(receipt, file_name, byte_len)))
        })
        .await
    }

    /// A file too large to read: the aborted receipt (F14). Otherwise the bytes durable
    /// in the artifact store first, then the receipt (REQ-INT-009).
    async fn receive(&self, call: &Call, chosen: ChosenFile) -> Result<Receipt, Failure> {
        let facts = chosen.facts;
        let Some(bytes) = chosen.bytes else {
            return self
                .write(call, move |ctx| {
                    lis_interfaces::record_oversize_file(ctx, facts)
                })
                .await;
        };
        let db = Arc::clone(&self.db);
        let artifact = tokio::task::spawn_blocking(move || db.artifacts().put(&bytes))
            .await
            .map_err(|_| Failure::Internal("artifact task stopped".to_owned()))??;
        let digest = artifact.digest.to_string();
        self.write(call, move |ctx| {
            lis_interfaces::receive_result_file(
                ctx,
                ReceiveResultFileInput {
                    file: facts,
                    sha256: digest.clone(),
                    artifact_digest: digest,
                },
            )
        })
        .await
    }

    /// `choose_result_folder` (`manage_configuration`): watches the chosen folder for the
    /// instrument once it is canonical, exists and can be read, and no other instrument
    /// watches it (F13). The result files already in it are recorded and never picked up;
    /// they can be loaded by hand (§6). The poller picks the change up at its next scan.
    /// `None` when cancelled.
    pub async fn choose_result_folder(
        &self,
        input: ResultFolderInput,
        folder: Option<PathBuf>,
    ) -> Result<Option<ChosenResultFolder>, CommandError> {
        self.run(&CHOOSE_RESULT_FOLDER, |call| async move {
            self.authorize(&call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let folder = tokio::task::spawn_blocking(move || readable_folder(&folder))
                .await
                .map_err(|_| Failure::Internal("folder check task stopped".to_owned()))??;
            let present_files = u32::try_from(folder.present.len()).unwrap_or(u32::MAX);
            let instrument = self.set_result_folder(&call, input, Some(folder)).await?;
            Ok(Some(ChosenResultFolder {
                instrument,
                present_files,
            }))
        })
        .await
    }

    /// `clear_result_folder` (`manage_configuration`): stops folder pickup for the
    /// instrument; files are then loaded by hand.
    pub async fn clear_result_folder(
        &self,
        input: ResultFolderInput,
    ) -> Result<Instrument, CommandError> {
        self.run(&CLEAR_RESULT_FOLDER, |call| async move {
            self.set_result_folder(&call, input, None).await
        })
        .await
    }

    async fn set_result_folder(
        &self,
        call: &Call,
        input: ResultFolderInput,
        folder: Option<WatchedFolder>,
    ) -> Result<Instrument, Failure> {
        self.write(call, move |ctx| {
            lis_interfaces::set_result_folder(
                ctx,
                SetResultFolder {
                    instrument_id: input.instrument_id,
                    expected_revision: input.expected_revision,
                    folder,
                },
            )
        })
        .await
    }
}

/// The chosen file's facts and, within the size limit, its bytes; the size is checked
/// before reading and again on the bytes read (F14).
fn read_chosen(path: &Path, instrument_id: InstrumentId) -> Result<ChosenFile, Failure> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| refused("Choose a file whose name has only standard characters."))?
        .to_owned();
    let metadata = fs::metadata(path).map_err(|_| unreadable_file())?;
    let modified_at = modified_at(&metadata).ok_or_else(unreadable_file)?;
    if !metadata.is_file() {
        return Err(unreadable_file());
    }
    let mut bytes = Vec::new();
    if metadata.len() <= MAX_RESULT_FILE_BYTES {
        open_for_reading(path)
            .map_err(|_| refused("The file could not be opened. If the export is still running, try again when it has finished."))?
            .take(MAX_RESULT_FILE_BYTES.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| unreadable_file())?;
    }
    let read_len = u64::try_from(bytes.len()).map_err(|_| unreadable_file())?;
    let too_large = metadata.len() > MAX_RESULT_FILE_BYTES || read_len > MAX_RESULT_FILE_BYTES;
    Ok(ChosenFile {
        facts: FileFacts {
            instrument_id,
            source: FileSource::Upload,
            file_name: Phi::new(file_name),
            byte_len: if too_large {
                metadata.len().max(read_len)
            } else {
                read_len
            },
            modified_at,
        },
        bytes: (!too_large).then_some(bytes),
    })
}

/// The canonical path of a folder that exists and can be listed, with the result files
/// already in it, as the poller would list them.
fn readable_folder(chosen: &Path) -> Result<WatchedFolder, Failure> {
    let unreadable = || {
        refused(
            "The folder could not be read. Check that it exists and that this computer may read it.",
        )
    };
    let canonical = fs::canonicalize(chosen).map_err(|_| unreadable())?;
    if !canonical.is_dir() {
        return Err(unreadable());
    }
    let present = candidates(&canonical)
        .map_err(|_| unreadable())?
        .into_iter()
        .map(|candidate| PresentFile {
            file_name: Phi::new(candidate.name),
            byte_len: candidate.byte_len,
            modified_at: candidate.modified_at,
        })
        .collect();
    let path = canonical
        .into_os_string()
        .into_string()
        .map_err(|_| refused("Choose a folder whose path has only standard characters."))?;
    Ok(WatchedFolder { path, present })
}

fn loaded(receipt: Receipt, file_name: Phi<String>, byte_len: u64) -> LoadedResultFile {
    // A file receipt is aborted only when the file is too large.
    let outcome = if receipt.state == ReceiptState::Aborted {
        LoadOutcome::TooLarge
    } else if receipt.duplicate_of.is_some() {
        LoadOutcome::Duplicate
    } else {
        LoadOutcome::Received
    };
    LoadedResultFile {
        receipt_id: receipt.id,
        instrument_id: receipt.instrument_id,
        received_at: receipt.started_at,
        outcome,
        duplicate_of: receipt.duplicate_of,
        file_name,
        byte_len,
        sha256: receipt.message_sha256,
    }
}

fn unreadable_file() -> Failure {
    refused("The file could not be read.")
}

fn refused(message: &str) -> Failure {
    Failure::refused(ErrorCode::Validation, message)
}
