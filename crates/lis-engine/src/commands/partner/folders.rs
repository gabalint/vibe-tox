//! Choosing a connection's folder, testing it, and writing a synthetic test message there.
//! File work never runs in a transaction.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lis_core::{CommandError, ErrorCode, UtcTimestamp};
use lis_interfaces::partner::{AttemptOutcome, PartnerConnectionId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::connections::PartnerConnectionInput;
use super::folder_choices::FolderChoiceId;
use super::{CHOOSE_PARTNER_FOLDER, SEND_PARTNER_TEST_MESSAGE, TEST_PARTNER_FOLDER};
use crate::commands::sign_in::session_of;
use crate::engine::Engine;
use crate::partner_delivery::{lab_zone, probe, stamp, write_file, written};
use crate::pipeline::{Call, Failure};

/// A folder chosen in the native dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChosenPartnerFolder {
    /// Pass this to save or test the connection with the folder.
    #[ts(type = "string")]
    pub folder_choice: FolderChoiceId,
    /// The folder, for display.
    pub folder: String,
}

/// Input of `test_partner_folder`: a newly chosen folder, or a connection's folder.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TestPartnerFolderInput {
    #[serde(default)]
    #[ts(optional, type = "string")]
    pub folder_choice: Option<FolderChoiceId>,
    #[serde(default)]
    #[ts(optional)]
    pub connection_id: Option<PartnerConnectionId>,
}

/// What a folder test or a test message found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderTestResult {
    /// `None` when the LIS could write there.
    pub problem: Option<String>,
    /// The test message's file, when one was written.
    pub file_name: Option<String>,
}

impl Engine {
    /// `choose_partner_folder` (`manage_configuration`): keeps the folder the dialog
    /// returned, once canonical and existing, for this session. `None` when cancelled.
    pub async fn choose_partner_folder(
        &self,
        folder: Option<PathBuf>,
    ) -> Result<Option<ChosenPartnerFolder>, CommandError> {
        self.run(&CHOOSE_PARTNER_FOLDER, |call| async move {
            self.authorize(&call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let folder = blocking(move || existing_folder(&folder)).await??;
            let (_, session) = session_of(&call.principal)?;
            Ok(Some(ChosenPartnerFolder {
                folder_choice: self.folder_choices.insert(session, folder.clone()),
                folder,
            }))
        })
        .await
    }

    /// `test_partner_folder`: whether the LIS can create, write and remove a file in the
    /// folder. Writes no message and changes nothing.
    pub async fn test_partner_folder(
        &self,
        input: TestPartnerFolderInput,
    ) -> Result<FolderTestResult, CommandError> {
        self.run(&TEST_PARTNER_FOLDER, |call| async move {
            self.authorize(&call).await?;
            let folder = match (input.folder_choice, input.connection_id) {
                (Some(choice), None) => self.chosen_folder(&call, Some(choice))?,
                (None, Some(id)) => Some(self.connection_folder(&call, id).await?),
                (Some(_), Some(_)) | (None, None) => None,
            };
            let folder = folder.ok_or_else(|| {
                Failure::refused(ErrorCode::Validation, "Choose a folder or a connection.")
            })?;
            let tested = blocking(move || probe(Path::new(&folder))).await?;
            Ok(FolderTestResult {
                problem: tested.err().map(|problem| problem.code().to_owned()),
                file_name: None,
            })
        })
        .await
    }

    /// `send_partner_test_message`: writes a synthetic file, with no patient data, to the
    /// connection's folder now, stopped or unchecked, and records it in the log, so the
    /// partner can confirm receipt before the destination check.
    pub async fn send_partner_test_message(
        &self,
        input: PartnerConnectionInput,
    ) -> Result<FolderTestResult, CommandError> {
        self.run(&SEND_PARTNER_TEST_MESSAGE, |call| async move {
            self.authorize(&call).await?;
            let id = input.connection_id;
            let folder = self.connection_folder(&call, id).await?;
            let now = self.clock.now();
            let bytes = test_message(id, now);
            let db = Arc::clone(&self.db);
            let stored = blocking(move || db.artifacts().put(&bytes).map(|stored| (stored, bytes)))
                .await??;
            let (stored, bytes) = stored;
            // Named in the laboratory's time, as the screens show it and billing files are named.
            let db = Arc::clone(&self.db);
            let zone = blocking(move || lab_zone(&db)).await?;
            let (date, time) = stamp(now, zone.as_ref());
            let name = format!("vibetox-test-{date}-{time}.json");
            let outcome =
                written(blocking(move || write_file(Path::new(&folder), &name, &bytes)).await?);
            let result = FolderTestResult {
                problem: match &outcome {
                    AttemptOutcome::Written { .. } => None,
                    AttemptOutcome::Failed { error_code } => Some((*error_code).to_owned()),
                },
                file_name: match &outcome {
                    AttemptOutcome::Written { path } => file_name(path),
                    AttemptOutcome::Failed { .. } => None,
                },
            };
            let digest = stored.digest.to_string();
            self.write(&call, move |ctx| {
                lis_interfaces::partner::record_partner_test_message(ctx, id, &digest, &outcome)
            })
            .await?;
            Ok(result)
        })
        .await
    }

    async fn connection_folder(
        &self,
        call: &Call,
        id: PartnerConnectionId,
    ) -> Result<String, Failure> {
        self.read(call, move |conn| {
            lis_interfaces::partner::partner_connection_folder(conn, id)
        })
        .await
    }
}

/// The synthetic test file: a fixed format, the connection and the time, no patient data.
fn test_message(connection_id: PartnerConnectionId, sent_at: UtcTimestamp) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "format": "vibetox-test-v1",
        "connectionId": connection_id.to_string(),
        "sentAt": sent_at.to_string(),
        "note": "Synthetic test message from the Vibe Tox LIS. It contains no patient data.",
    }))
    .unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn file_name(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// The canonical path of a folder that exists.
fn existing_folder(chosen: &Path) -> Result<String, Failure> {
    let refused = |message| Failure::refused(ErrorCode::Validation, message);
    let canonical = fs::canonicalize(chosen)
        .map_err(|_| refused("The folder could not be opened. Check that it exists."))?;
    if !canonical.is_dir() {
        return Err(refused("Choose a folder, not a file."));
    }
    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| refused("Choose a folder whose path has only standard characters."))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| Failure::Internal("folder task stopped".to_owned()))
}
