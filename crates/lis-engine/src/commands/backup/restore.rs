//! Restore-wizard commands for a new installation (REQ-OPS-002, NFR-04, NFR-30).
//! The installation key and selected paths live only in process memory. Staging performs
//! the real decrypt, database open, chain, anchor and artifact checks on a blocking thread.

mod progress;

use std::path::PathBuf;
use std::sync::Arc;

use lis_core::{CommandError, FieldError, InstallationId, UtcTimestamp};
use lis_ops::anchors::AnchorSummary;
use lis_ops::backup::BackupError;
use lis_ops::restore::{RestoreSession, RestoreSessionId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    BEGIN_RESTORE, BackupSetView, CANCEL_RESTORE, CHECK_RESTORE_ANCHORS, CHOOSE_RESTORE_SOURCE,
    CONFIRM_RESTORE, FindingView, GET_RESTORE_AVAILABILITY, STAGE_RESTORE,
};
use crate::engine::{Engine, MODULES};
use crate::keys::{InstallationKey, KeyState};
use crate::ops_worker::locked;
use crate::pipeline::Failure;
use progress::{
    RestoreProgress, ensure_kind, ensure_new_installation, put_progress, put_unless_expired,
    session_guard, stage_progress, staged_view, take_progress,
};
pub(crate) use progress::{RestoreStep, preflight_restore_step};

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BeginRestoreInput {
    pub installation_key: String,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RestoreSessionInput {
    pub restore_session_id: RestoreSessionId,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct StageRestoreInput {
    pub restore_session_id: RestoreSessionId,
    pub set_id: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConfirmRestoreInput {
    pub restore_session_id: RestoreSessionId,
    pub confirmation: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreAvailability {
    pub available: bool,
    pub pending: Option<PendingRestoreView>,
    pub last_failure_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PendingRestoreView {
    pub set_id: String,
    pub set_created_at: UtcTimestamp,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreSessionView {
    pub restore_session_id: RestoreSessionId,
    pub key_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreSourceView {
    pub installation_id: InstallationId,
    pub sets: Vec<BackupSetView>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StagedRestoreView {
    pub set_id: String,
    pub set_created_at: UtcTimestamp,
    pub app_version: String,
    #[ts(type = "number")]
    pub database_bytes: u64,
    #[ts(type = "number")]
    pub artifact_count: usize,
    pub integrity_ok: bool,
    pub chains: Vec<RestoreChainView>,
    pub anchors: Option<AnchorSummary>,
    pub lost_interval_until: Option<UtcTimestamp>,
    pub findings: Vec<FindingView>,
    pub can_confirm: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreChainView {
    pub stream: String,
    #[ts(type = "number")]
    pub events: u64,
    pub intact: bool,
    pub head_matches_manifest: bool,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfirmRestoreView {
    pub closing: bool,
}

impl Engine {
    pub async fn get_restore_availability(&self) -> Result<RestoreAvailability, CommandError> {
        self.run(&GET_RESTORE_AVAILABILITY, |call| async move {
            let data_dir = self.ops.data_dir.clone();
            let new_installation = self
                .read(&call, |conn| {
                    lis_auth::setup_status(conn).map(|status| status.needs_administrator)
                })
                .await?;
            let pending =
                tokio::task::spawn_blocking(move || lis_ops::restore::pending_restore(&data_dir))
                    .await
                    .map_err(|_| {
                        Failure::Internal("restore availability task stopped".to_owned())
                    })??;
            let last_failure_code = pending
                .as_ref()
                .and_then(|pending| pending.marker.failure)
                .map(|failure| failure.as_str().to_owned());
            Ok(RestoreAvailability {
                available: new_installation && pending.is_none(),
                pending: pending.map(|pending| PendingRestoreView {
                    set_id: pending.marker.set_id,
                    set_created_at: pending.marker.set_created_at,
                }),
                last_failure_code,
            })
        })
        .await
    }

    pub async fn begin_restore(
        &self,
        input: BeginRestoreInput,
    ) -> Result<RestoreSessionView, CommandError> {
        self.run(&BEGIN_RESTORE, |call| async move {
            ensure_new_installation(self, &call).await?;
            let data_dir = self.ops.data_dir.clone();
            let session = RestoreSession::begin(&input.installation_key, self.clock.now())?;
            let view = RestoreSessionView {
                restore_session_id: session.id(),
                key_fingerprint: session.key_id().to_string(),
            };
            let mut sessions = session_guard();
            sessions.retain(|_, progress| progress.data_dir != data_dir);
            sessions.insert(
                session.id(),
                RestoreProgress {
                    data_dir,
                    session,
                    source: None,
                    staged: None,
                },
            );
            Ok(view)
        })
        .await
    }

    pub async fn choose_restore_source(
        &self,
        input: RestoreSessionInput,
        folder: Option<PathBuf>,
    ) -> Result<Option<RestoreSourceView>, CommandError> {
        self.run(&CHOOSE_RESTORE_SOURCE, |call| async move {
            ensure_new_installation(self, &call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let mut progress = take_progress(input.restore_session_id)?;
            let now = self.clock.now();
            let result = tokio::task::spawn_blocking(move || {
                let source = progress
                    .session
                    .backup_key(now)
                    .and_then(|key| lis_ops::restore::read_source(&folder, key, MODULES));
                (progress, source)
            })
            .await
            .map_err(|_| Failure::Internal("restore source task stopped".to_owned()))?;
            progress = result.0;
            let source = match result.1 {
                Ok(source) => source,
                Err(error) => {
                    put_unless_expired(progress, &error);
                    return Err(error.into());
                }
            };
            let view = RestoreSourceView {
                installation_id: source.installation_id,
                sets: source
                    .sets
                    .iter()
                    .cloned()
                    .map(super::views::set_view)
                    .collect(),
            };
            progress.source = Some(source);
            progress.staged = None;
            put_progress(progress);
            Ok(Some(view))
        })
        .await
    }

    pub async fn stage_restore(
        &self,
        input: StageRestoreInput,
    ) -> Result<StagedRestoreView, CommandError> {
        self.run(&STAGE_RESTORE, |call| async move {
            ensure_new_installation(self, &call).await?;
            let progress = take_progress(input.restore_session_id)?;
            let (progress, staged, summary) = stage_progress(
                progress,
                input.set_id,
                Vec::new(),
                self.clock.now(),
                Arc::clone(&self.clock),
                Arc::clone(&self.ops),
            )
            .await?;
            let mut progress = ensure_kind(progress, &staged, self.installation_kind)?;
            let view = staged_view(&staged, &summary);
            progress.staged = Some(staged);
            put_progress(progress);
            Ok(view)
        })
        .await
    }

    pub async fn check_restore_anchors(
        &self,
        input: RestoreSessionInput,
        folder: Option<PathBuf>,
    ) -> Result<Option<AnchorSummary>, CommandError> {
        self.run(&CHECK_RESTORE_ANCHORS, |call| async move {
            ensure_new_installation(self, &call).await?;
            let Some(folder) = folder else {
                return Ok(None);
            };
            let progress = take_progress(input.restore_session_id)?;
            let Some(set_id) = progress.staged.as_ref().map(|staged| staged.set_id.clone()) else {
                put_progress(progress);
                return Err(BackupError::RestoreNotVerified.into());
            };
            let (progress, staged, _) = stage_progress(
                progress,
                set_id,
                vec![folder],
                self.clock.now(),
                Arc::clone(&self.clock),
                Arc::clone(&self.ops),
            )
            .await?;
            let mut progress = ensure_kind(progress, &staged, self.installation_kind)?;
            let anchors = staged.anchors;
            progress.staged = Some(staged);
            put_progress(progress);
            Ok(anchors)
        })
        .await
    }

    pub async fn confirm_restore(
        &self,
        input: ConfirmRestoreInput,
    ) -> Result<ConfirmRestoreView, CommandError> {
        self.run(&CONFIRM_RESTORE, |call| async move {
            ensure_new_installation(self, &call).await?;
            if input.confirmation != "RESTORE" {
                return Err(BackupError::Invalid(FieldError {
                    field: "confirmation".to_owned(),
                    code: "confirmation_mismatch".to_owned(),
                    message: "Type RESTORE exactly to continue.".to_owned(),
                })
                .into());
            }
            let mut progress = take_progress(input.restore_session_id)?;
            let key = match progress.session.installation_key(self.clock.now()) {
                Ok(key) => key.clone(),
                Err(error) => {
                    put_unless_expired(progress, &error);
                    return Err(error.into());
                }
            };
            let Some(staged) = progress.staged.take() else {
                put_progress(progress);
                return Err(BackupError::RestoreNotVerified.into());
            };
            let provider = self.keys.clone();
            let stored_key = key.clone();
            let stored = tokio::task::spawn_blocking(move || provider.store(&stored_key))
                .await
                .map_err(|_| Failure::Internal("installation key task stopped".to_owned()))?;
            if let Err(error) = stored {
                progress.staged = Some(staged);
                put_progress(progress);
                return Err(Failure::Internal(format!(
                    "installation key unavailable: {error}"
                )));
            }
            *locked(&self.ops.key) = InstallationKey {
                state: KeyState::Ok,
                key: Some(key),
            };
            let data_dir = progress.data_dir.clone();
            let now = self.clock.now();
            let to_confirm = staged.clone();
            let confirmed = tokio::task::spawn_blocking(move || {
                lis_ops::restore::confirm_restore(&data_dir, &to_confirm, now)
            })
            .await
            .map_err(|_| Failure::Internal("restore confirmation task stopped".to_owned()))?;
            if let Err(error) = confirmed {
                progress.staged = Some(staged);
                put_progress(progress);
                return Err(error.into());
            }
            Ok(ConfirmRestoreView { closing: true })
        })
        .await
    }

    pub async fn cancel_restore(&self, input: RestoreSessionInput) -> Result<(), CommandError> {
        self.run(&CANCEL_RESTORE, |call| async move {
            ensure_new_installation(self, &call).await?;
            let progress = take_progress(input.restore_session_id)?;
            let data_dir = progress.data_dir.clone();
            let cancelled =
                tokio::task::spawn_blocking(move || lis_ops::restore::cancel_restore(&data_dir))
                    .await
                    .map_err(|_| {
                        Failure::Internal("restore cancellation task stopped".to_owned())
                    })?;
            match cancelled {
                Ok(true) => Ok(()),
                Ok(false) => {
                    put_progress(progress);
                    Err(BackupError::RestoreNotVerified.into())
                }
                Err(error) => {
                    put_progress(progress);
                    Err(error.into())
                }
            }
        })
        .await
    }
}
