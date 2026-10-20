//! The installation key's state and its restore from the recovery kit (§3.9). Both are
//! public: the sign-in screen offers the restore while TOTP sign-in is refused.
//!
//! A restore writes the OS key store, which no transaction covers, so it is audited around
//! the write (M7): an intent event commits first, then the key is stored and used, then a
//! completion event (`auth.restore_installation_key`, succeeded or failed) commits. If the
//! completion cannot be recorded, startup records it from the key it then finds
//! ([`reconcile_key_restores`]), so every key-store write has a completed audit.

use lis_audit::{AuditDetail, AuditEvent, EventFilter, Outcome, Stream};
use lis_core::{CommandError, CorrelationId, ErrorCode};
use lis_store::{Database, StoreError};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use ts_rs::TS;

use super::{GET_INSTALLATION_KEY_STATE, RESTORE_INSTALLATION_KEY, invalid};
use crate::engine::{Engine, EngineError};
use crate::keys::{InstallationKey, InstallationKeyStateView, KeyState};
use crate::ops_worker::{locked, startup};
use crate::pipeline::{Call, Failure};

/// Audit action of a completed installation-key restore (security stream).
pub(super) const RESTORE_INSTALLATION_KEY_ACTION: &str = "auth.restore_installation_key";
/// Audit action committed before the key store is written.
const RESTORE_INTENT_ACTION: &str = "auth.restore_installation_key_intent";
/// Record type of both events; the record ID is the restore command's correlation ID.
const KEY_RESTORE_RECORD: &str = "installation_key_restore";

/// Input of `restore_installation_key`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RestoreInstallationKeyInput {
    /// The `Installation key:` line of the recovery kit.
    #[ts(type = "string")]
    pub installation_key: SecretString,
}

impl Engine {
    /// `get_installation_key_state` (public).
    pub async fn get_installation_key_state(
        &self,
    ) -> Result<InstallationKeyStateView, CommandError> {
        self.run(&GET_INSTALLATION_KEY_STATE, |_| async move {
            Ok(InstallationKeyStateView {
                key_state: locked(&self.ops.key).state,
            })
        })
        .await
    }

    /// `restore_installation_key` (public): accepts the key line from the recovery kit only
    /// while the key is not `ok`, and only when it belongs to this database: it opens a
    /// sealed secret, or with none sealed it has the newest kit's key ID. Audited around
    /// the key-store write (module documentation).
    pub async fn restore_installation_key(
        &self,
        input: RestoreInstallationKeyInput,
    ) -> Result<InstallationKeyStateView, CommandError> {
        self.run(&RESTORE_INSTALLATION_KEY, |call| async move {
            let previous = locked(&self.ops.key).state;
            if previous == KeyState::Ok {
                return Err(Failure::refused(
                    ErrorCode::Conflict,
                    "The installation key is already in place.",
                ));
            }
            let key =
                lis_ops::kit::parse_key(input.installation_key.expose_secret()).map_err(|_| {
                    invalid(
                        "installationKey",
                        "invalid_format",
                        "Enter the whole key line from the recovery kit, including its checksum.",
                    )
                })?;
            self.check_belongs(&key).await?;
            let attempt = call.correlation_id;
            self.write(&call, move |ctx| -> Result<(), StoreError> {
                ctx.audit(key_restore_event(
                    RESTORE_INTENT_ACTION,
                    attempt,
                    Outcome::Succeeded,
                    previous,
                ))
                .map(drop)
            })
            .await?;
            let (keys, stored) = (self.keys.clone(), key.clone());
            let written = tokio::task::spawn_blocking(move || keys.store(&stored))
                .await
                .map_err(|_| Failure::Internal("key store task stopped".to_owned()))?;
            if let Err(error) = written {
                tracing::error!(%error, "restored installation key not stored");
                self.complete_key_restore(&call, Outcome::Failed, previous)
                    .await?;
                return Err(Failure::refused(
                    ErrorCode::Unavailable,
                    "The key is correct but could not be saved in this computer's key store.",
                ));
            }
            *locked(&self.ops.key) = InstallationKey {
                state: KeyState::Ok,
                key: Some(key),
            };
            tracing::info!(
                previous_state = previous.as_str(),
                "installation key restored"
            );
            self.complete_key_restore(&call, Outcome::Succeeded, previous)
                .await?;
            Ok(InstallationKeyStateView {
                key_state: KeyState::Ok,
            })
        })
        .await
    }

    /// Refuses a key that does not belong to this database, or that nothing here can
    /// confirm.
    async fn check_belongs(&self, key: &lis_auth::DataKey) -> Result<(), Failure> {
        let (db, key) = (std::sync::Arc::clone(&self.db), key.clone());
        let belongs = tokio::task::spawn_blocking(move || crate::keys::belongs(&db, &key))
            .await
            .map_err(|_| Failure::Internal("key check task stopped".to_owned()))?
            .map_err(|error| Failure::Internal(format!("key check failed: {error}")))?;
        match belongs {
            Some(true) => Ok(()),
            Some(false) => Err(invalid(
                "installationKey",
                "does_not_open",
                "This key does not belong to this installation's data.",
            )),
            None => Err(invalid(
                "installationKey",
                "cannot_check",
                "This installation holds nothing this key can be checked against. Contact support.",
            )),
        }
    }

    async fn complete_key_restore(
        &self,
        call: &Call,
        outcome: Outcome,
        previous: KeyState,
    ) -> Result<(), Failure> {
        let attempt = call.correlation_id;
        self.write(call, move |ctx| -> Result<(), StoreError> {
            ctx.audit(key_restore_event(
                RESTORE_INSTALLATION_KEY_ACTION,
                attempt,
                outcome,
                previous,
            ))
            .map(drop)
        })
        .await
    }
}

fn key_restore_event(
    action: &'static str,
    attempt: CorrelationId,
    outcome: Outcome,
    previous: KeyState,
) -> AuditEvent {
    AuditEvent::new(Stream::Security, action, outcome)
        .record(KEY_RESTORE_RECORD, attempt)
        .detail("previous_state", AuditDetail::code(previous.as_str()))
}

/// Startup step (M7): every key-restore intent without a completion gets one now,
/// `succeeded` when the key loaded at this start is usable, `failed` otherwise, marked
/// `reconciled`. Returns how many it completed.
pub(crate) async fn reconcile_key_restores(
    db: &Database,
    state: KeyState,
) -> Result<usize, EngineError> {
    let completed = startup::lifecycle_write(db, move |ctx| -> Result<usize, StoreError> {
        let intents = lis_audit::list(ctx.tx(), &key_restore_filter(RESTORE_INTENT_ACTION, None))?;
        let mut completed = 0;
        for intent in intents {
            let Some(attempt) = intent.record_id else {
                continue;
            };
            let filter = key_restore_filter(RESTORE_INSTALLATION_KEY_ACTION, Some(&attempt));
            if !lis_audit::list(ctx.tx(), &filter)?.is_empty() {
                continue;
            }
            let outcome = if state == KeyState::Ok {
                Outcome::Succeeded
            } else {
                Outcome::Failed
            };
            ctx.audit(
                AuditEvent::new(Stream::Security, RESTORE_INSTALLATION_KEY_ACTION, outcome)
                    .record(KEY_RESTORE_RECORD, &attempt)
                    .detail("key_state", AuditDetail::code(state.as_str()))
                    .detail("reconciled", AuditDetail::boolean(true)),
            )?;
            completed += 1;
        }
        Ok(completed)
    })
    .await?;
    if completed > 0 {
        tracing::warn!(completed, "key restores completed at startup");
    }
    Ok(completed)
}

/// The key-restore events of `action`, optionally of one attempt, newest first.
fn key_restore_filter<'a>(action: &'a str, attempt: Option<&'a str>) -> EventFilter<'a> {
    EventFilter {
        stream: Stream::Security,
        before_seq: None,
        limit: u32::MAX,
        action: Some(action),
        record_type: Some(KEY_RESTORE_RECORD),
        record_id: attempt,
        self_approvals_only: false,
    }
}
