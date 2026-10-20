//! Recovery-kit status, export and off-computer storage confirmation.

use std::path::PathBuf;
use std::sync::Arc;

use lis_auth::ReauthPurpose;
use lis_auth::Reauthentication;
use lis_core::{CommandError, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CONFIRM_RECOVERY_KIT_STORAGE, EXPORT_RECOVERY_KIT, GET_RECOVERY_KIT_STATUS};
use crate::commands::reporting::user_of;
use crate::engine::Engine;
use crate::ops_worker::locked;
use crate::pipeline::accepted;

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportRecoveryKitInput {
    pub disk_encryption_reference: String,
    /// Fresh confirmation by the exporter.
    pub password: Reauthentication,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConfirmRecoveryKitStorageInput {
    #[ts(type = "number")]
    pub kit_no: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedKit {
    #[ts(type = "number")]
    pub kit_no: i64,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryKitStatus {
    pub key_fingerprint: String,
    pub key_state: String,
    pub last_export: Option<KitExportView>,
    pub last_confirmation: Option<KitConfirmationView>,
    pub outdated: bool,
    pub outdated_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KitExportView {
    #[ts(type = "number")]
    pub kit_no: i64,
    pub exported_at: UtcTimestamp,
    pub exported_by: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KitConfirmationView {
    pub at: UtcTimestamp,
    pub by: String,
}

impl Engine {
    pub async fn get_recovery_kit_status(&self) -> Result<RecoveryKitStatus, CommandError> {
        self.run(&GET_RECOVERY_KIT_STATUS, |call| async move {
            let (key_state, key_id) = {
                let installed = locked(&self.ops.key);
                (
                    installed.state,
                    installed
                        .key
                        .as_ref()
                        .map(lis_ops::format::BackupKey::derive)
                        .map(|key| key.key_id()),
                )
            };
            self.read(&call, move |conn| {
                let status = match key_id {
                    Some(key_id) => lis_ops::kit::status(conn, key_id)?,
                    None => lis_ops::kit::status_without_key(conn)?,
                };
                let fingerprint = match key_id {
                    Some(key_id) => key_id.to_string(),
                    None => lis_ops::kit::latest_key_fingerprint(conn)?.unwrap_or_default(),
                };
                Ok::<_, lis_ops::backup::BackupError>(kit_status(
                    fingerprint,
                    key_state.as_str(),
                    status,
                ))
            })
            .await
        })
        .await
    }

    pub async fn export_recovery_kit(
        &self,
        input: ExportRecoveryKitInput,
        destination: Option<PathBuf>,
    ) -> Result<Option<ExportedKit>, CommandError> {
        self.run(&EXPORT_RECOVERY_KIT, |call| async move {
            // The capability is checked before a cancelled dialog is answered (NFR-31).
            self.authorize(&call).await?;
            let Some(destination) = destination else {
                return Ok(None);
            };
            let password = input.password;
            let reauthenticated = self
                .write(&call, move |ctx| {
                    lis_auth::reauthenticate(ctx, &password, ReauthPurpose::RecoveryKitExport)
                })
                .await?;
            accepted(reauthenticated)?;

            let key = self.data_key().await?;
            let data_dir = self.ops.data_dir.clone();
            let operating_system = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
            let reference = input.disk_encryption_reference;
            let user_id = user_of(&call.principal)?;
            let user = self
                .read(&call, move |conn| lis_auth::session_user(conn, user_id))
                .await?;
            let prepared = self
                .write(&call, move |ctx| {
                    lis_ops::kit::prepare_export(
                        ctx,
                        &key,
                        &lis_ops::kit::KitInput {
                            created_by_name: &user.username,
                            operating_system: &operating_system,
                            data_dir: &data_dir,
                            disk_encryption_reference: &reference,
                        },
                        &destination,
                    )
                })
                .await?;
            let prepared = Arc::new(prepared);
            let to_write = Arc::clone(&prepared);
            tokio::task::spawn_blocking(move || lis_ops::kit::write_prepared(&to_write))
                .await
                .map_err(|_| {
                    crate::pipeline::Failure::Internal("recovery kit task stopped".to_owned())
                })??;
            let to_record = Arc::clone(&prepared);
            let result = self
                .write(&call, move |ctx| {
                    lis_ops::kit::record_prepared(ctx, &to_record)
                })
                .await;
            match result {
                Ok(exported) => Ok(Some(ExportedKit {
                    kit_no: exported.kit_no,
                    file_name: exported.file_name,
                })),
                Err(error) => {
                    discard(Arc::clone(&prepared)).await;
                    Err(error)
                }
            }
        })
        .await
    }

    pub async fn confirm_recovery_kit_storage(
        &self,
        input: ConfirmRecoveryKitStorageInput,
    ) -> Result<RecoveryKitStatus, CommandError> {
        self.run(&CONFIRM_RECOVERY_KIT_STORAGE, |call| async move {
            let key = self.data_key().await?;
            let key_id = lis_ops::format::BackupKey::derive(&key).key_id();
            self.write(&call, move |ctx| {
                lis_ops::kit::confirm_storage(ctx, input.kit_no, &input.note)?;
                lis_ops::kit::status(ctx.tx(), key_id)
                    .map(|status| kit_status(key_id.to_string(), "ok", status))
            })
            .await
        })
        .await
    }
}

async fn discard(prepared: Arc<lis_ops::kit::PreparedKit>) {
    let _ = tokio::task::spawn_blocking(move || lis_ops::kit::discard_prepared(&prepared)).await;
}

fn kit_status(
    key_fingerprint: String,
    key_state: &str,
    status: lis_ops::kit::KitStatus,
) -> RecoveryKitStatus {
    let outdated_reasons: Vec<String> = status
        .outdated_reasons
        .into_iter()
        .map(|reason| reason.as_str().to_owned())
        .collect();
    RecoveryKitStatus {
        key_fingerprint,
        key_state: key_state.to_owned(),
        last_export: status.latest.map(|kit| KitExportView {
            kit_no: kit.kit_no,
            exported_at: kit.exported_at,
            exported_by: kit.exported_by,
            file_name: kit.file_name,
        }),
        last_confirmation: status
            .last_confirmation
            .map(|confirmation| KitConfirmationView {
                at: confirmation.confirmed_at,
                by: confirmation.confirmed_by,
            }),
        outdated: !outdated_reasons.is_empty(),
        outdated_reasons,
    }
}
