//! Post-restore reconciliation and number-floor commands (REQ-OPS-002).

use lis_auth::Reauthentication;
use lis_auth::{Attempt, AuthError, ReauthPurpose};
use lis_core::{CommandError, UtcTimestamp};
use lis_ops::anchors::AnchorSummary;
use lis_ops::backup::BackupError;
use lis_ops::recovery::ChecklistItem;
use lis_ops::restore::RestoreId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CLOSE_RECOVERY_RECONCILIATION, GET_RECOVERY_RECONCILIATION, RAISE_NUMBER_FLOORS};
use crate::engine::Engine;
use crate::pipeline::{Failure, ModuleError, accepted};

#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CloseRecoveryInput {
    pub restore_id: RestoreId,
    pub attested_items: Vec<ChecklistItem>,
    pub note: String,
    /// The closer's password (fresh authentication).
    pub password: Reauthentication,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryReconciliation {
    pub restore_id: RestoreId,
    pub set_created_at: UtcTimestamp,
    pub applied_at: UtcTimestamp,
    pub lost_interval_from: UtcTimestamp,
    pub items: Vec<ChecklistItemView>,
    pub anchors: AnchorSummary,
    #[ts(type = "number")]
    pub held_label_jobs: u64,
    pub floors: RecoveryFloorView,
    pub closed: Option<RecoveryClosedView>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChecklistItemView {
    pub code: String,
    pub label: String,
    pub guidance: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryFloorView {
    pub accession: String,
    pub container: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryClosedView {
    pub by: String,
    pub at: UtcTimestamp,
}

#[derive(Debug, thiserror::Error)]
enum RecoveryCommandError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Backup(#[from] BackupError),
}

impl From<lis_store::StoreError> for RecoveryCommandError {
    fn from(error: lis_store::StoreError) -> Self {
        Self::Backup(BackupError::Store(error))
    }
}

impl ModuleError for RecoveryCommandError {
    fn denied(&self) -> Option<lis_core::Denied> {
        match self {
            Self::Auth(error) => ModuleError::denied(error),
            Self::Backup(BackupError::Store(error)) => ModuleError::denied(error),
            Self::Backup(_) => None,
        }
    }
}

impl From<RecoveryCommandError> for Failure {
    fn from(error: RecoveryCommandError) -> Self {
        match error {
            RecoveryCommandError::Auth(error) => error.into(),
            RecoveryCommandError::Backup(error) => error.into(),
        }
    }
}

impl Engine {
    pub async fn get_recovery_reconciliation(
        &self,
    ) -> Result<Option<RecoveryReconciliation>, CommandError> {
        self.run(&GET_RECOVERY_RECONCILIATION, |call| async move {
            self.read(&call, |conn| {
                let view = lis_ops::recovery::get_recovery_reconciliation(conn)?;
                let held: i64 = conn.query_row(
                    "SELECT count(*) FROM store_outbox WHERE topic = 'spc.label.print' \
                     AND state IN ('pending', 'leased')",
                    [],
                    |row| row.get(0),
                )?;
                Ok::<_, BackupError>(view.map(|view| reconciliation_view(view, held)))
            })
            .await
        })
        .await
    }

    pub async fn close_recovery_reconciliation(
        &self,
        input: CloseRecoveryInput,
    ) -> Result<(), CommandError> {
        self.run(&CLOSE_RECOVERY_RECONCILIATION, |call| async move {
            // The capability is checked before the password, so a person without it is
            // refused as forbidden, never asked for a password.
            self.authorize(&call).await?;
            let CloseRecoveryInput {
                restore_id,
                attested_items,
                note,
                password,
            } = input;
            let attempt = self
                .write(&call, move |ctx| {
                    close(ctx, &password, restore_id, &attested_items, &note)
                })
                .await?;
            accepted(attempt)
        })
        .await
    }

    pub async fn raise_number_floors(
        &self,
        input: lis_specimens::RaiseNumberFloorsInput,
    ) -> Result<lis_specimens::NumberFloors, CommandError> {
        self.run(&RAISE_NUMBER_FLOORS, |call| async move {
            self.write(&call, move |ctx| {
                lis_specimens::raise_number_floors(ctx, input)
            })
            .await
        })
        .await
    }
}

fn reconciliation_view(view: lis_ops::recovery::RecoveryView, held: i64) -> RecoveryReconciliation {
    RecoveryReconciliation {
        restore_id: view.restore_id,
        set_created_at: view.set_created_at,
        applied_at: view.applied_at,
        lost_interval_from: view.set_created_at,
        items: view
            .items
            .into_iter()
            .map(|item| ChecklistItemView {
                code: item.as_str().to_owned(),
                label: item.label().to_owned(),
                guidance: item.guidance().to_owned(),
            })
            .collect(),
        anchors: view.anchors,
        held_label_jobs: u64::try_from(held).unwrap_or(0),
        floors: RecoveryFloorView {
            accession: view.floors.accession,
            container: view.floors.container,
        },
        closed: view.closed.map(|(by, at)| RecoveryClosedView { by, at }),
    }
}

fn close(
    ctx: &mut lis_store::CommandContext<'_>,
    password: &Reauthentication,
    restore_id: RestoreId,
    attested_items: &[ChecklistItem],
    note: &str,
) -> Result<Attempt<()>, RecoveryCommandError> {
    match lis_auth::reauthenticate(ctx, password, ReauthPurpose::RecoveryClose)? {
        Attempt::Rejected(rejection) => Ok(Attempt::Rejected(rejection)),
        Attempt::Accepted(()) => {
            lis_ops::recovery::close_recovery_reconciliation(
                ctx,
                restore_id,
                attested_items,
                note,
            )?;
            Ok(Attempt::Accepted(()))
        }
    }
}
