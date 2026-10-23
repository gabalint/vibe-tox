//! Exit-export request and history commands (REQ-OPS-010, NFR-12).
//! The selected folder is kept only by the in-memory job runner; no path is audited or
//! stored. Fresh authentication commits before the request transaction.

use std::path::PathBuf;

use lis_auth::Reauthentication;
use lis_auth::{ReauthPurpose, reauthenticate};
use lis_core::{CommandError, UtcTimestamp};
use lis_ops::exit_export::{ExitExportId, ExitExportRecord};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{LIST_EXIT_EXPORTS, LimitInput, REQUEST_EXIT_EXPORT, checked_limit};
use crate::engine::Engine;
use crate::pipeline::accepted;

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RequestExitExportInput {
    pub reason: String,
    /// Fresh confirmation by the requester.
    pub password: Reauthentication,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExitExportView {
    pub export_id: ExitExportId,
    pub state: String,
    pub requested_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub requested_by_display: Option<String>,
    pub requested_at: UtcTimestamp,
    pub finished_at: Option<UtcTimestamp>,
    pub folder_label: String,
    pub package_name: Option<String>,
    #[ts(type = "number | null")]
    pub database_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub artifact_count: Option<u64>,
    pub manifest_sha256: Option<String>,
    pub failure_code: Option<String>,
}

impl Engine {
    pub async fn request_exit_export(
        &self,
        input: RequestExitExportInput,
        folder: Option<PathBuf>,
    ) -> Result<Option<ExitExportView>, CommandError> {
        self.run(&REQUEST_EXIT_EXPORT, |call| async move {
            // The capability is checked before a cancelled dialog is answered (NFR-31).
            self.authorize(&call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let password = input.password;
            let attempt = self
                .write(&call, move |ctx| {
                    reauthenticate(ctx, &password, ReauthPurpose::ExitExport)
                })
                .await?;
            accepted(attempt)?;
            let data_dir = self.ops.data_dir.clone();
            let export_id = self
                .write(&call, {
                    let folder = folder.clone();
                    move |ctx| request(ctx, &data_dir, &folder, &input.reason)
                })
                .await?;
            let view = self
                .read(&call, move |conn| requested_view(conn, export_id))
                .await?;
            self.queue_exit_export(export_id, folder);
            Ok(Some(view))
        })
        .await
    }

    pub async fn list_exit_exports(
        &self,
        input: LimitInput,
    ) -> Result<Vec<ExitExportView>, CommandError> {
        self.run(&LIST_EXIT_EXPORTS, |call| async move {
            let limit = checked_limit(input.limit)?;
            self.read(&call, move |conn| {
                let rows = lis_ops::exit_export::list_exit_exports(conn, limit)?;
                let names = crate::commands::people::display_names(
                    conn,
                    rows.iter().map(|row| row.requested_by.as_str()),
                )?;
                Ok::<_, lis_ops::backup::BackupError>(
                    rows.into_iter()
                        .map(|row| exit_export_view(row, &names))
                        .collect(),
                )
            })
            .await
        })
        .await
    }
}

fn request(
    ctx: &mut lis_store::CommandContext<'_>,
    data_dir: &std::path::Path,
    folder: &std::path::Path,
    reason: &str,
) -> Result<ExitExportId, lis_ops::backup::BackupError> {
    lis_ops::exit_export::request_exit_export(ctx, data_dir, folder, reason)
}

fn requested_view(
    conn: &rusqlite::Connection,
    export_id: ExitExportId,
) -> Result<ExitExportView, lis_ops::backup::BackupError> {
    let row = lis_ops::exit_export::list_exit_exports(conn, 500)?
        .into_iter()
        .find(|row| row.export_id == export_id)
        .ok_or(lis_ops::backup::BackupError::NotFound("exit export"))?;
    let names = crate::commands::people::display_names(conn, [row.requested_by.as_str()])?;
    Ok(exit_export_view(row, &names))
}

fn exit_export_view(
    record: ExitExportRecord,
    names: &std::collections::HashMap<String, String>,
) -> ExitExportView {
    ExitExportView {
        export_id: record.export_id,
        state: record.state.as_str().to_owned(),
        requested_by_display: crate::commands::people::name_of(names, &record.requested_by),
        requested_by: record.requested_by,
        requested_at: record.requested_at,
        finished_at: record.finished_at,
        folder_label: record.folder_label,
        package_name: record.package_name,
        database_bytes: record.database_bytes,
        artifact_count: record.artifact_count,
        manifest_sha256: record.manifest_sha256,
        failure_code: record.failure.map(|failure| failure.as_str().to_owned()),
    }
}
