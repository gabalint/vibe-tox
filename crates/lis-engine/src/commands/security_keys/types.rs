//! Secret-aware desktop inputs. Ceremony challenges and proofs never cross IPC.

use std::future::Future;

use lis_auth::{Reauthentication, SecurityKeyId};
use lis_core::CommandError;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Trusted native transport. The engine independently verifies every response.
pub trait SecurityKeyClient: Sync {
    /// Seal the final commit boundary, refusing cancellation received before it.
    /// A native client rejects later cancellation because the engine may commit.
    fn finish(&self) -> Result<(), CommandError> {
        Ok(())
    }
    /// Registers a key with mandatory authenticator user verification.
    fn register(
        &self,
        origin: String,
        options: Value,
    ) -> impl Future<Output = Result<Value, CommandError>> + Send;
    /// Asserts possession and mandatory authenticator user verification.
    fn authenticate(
        &self,
        origin: String,
        options: Value,
    ) -> impl Future<Output = Result<Value, CommandError>> + Send;
}

/// First administrator creation using a personal security key.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeySignupInput {
    pub username: String,
    pub display_name: String,
    pub one_person: bool,
    pub load_demo_data: Option<bool>,
    pub key_label: String,
}

/// Public key selection metadata query.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct KeySignInChoicesInput {
    pub username: String,
}

/// Sign in with the selected, already registered key.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeySignInInput {
    pub username: String,
    pub credential_id: SecurityKeyId,
}

/// Unlock the same person's session with a registered key.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeyUnlockInput {
    pub credential_id: SecurityKeyId,
}

/// Add a key, or switch a password/TOTP account after fresh authentication.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeyEnrollInput {
    pub key_label: String,
    pub password: Reauthentication,
    pub credential_id: Option<SecurityKeyId>,
}

/// Recover by enrolling a replacement key. Never grants a session.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeyRecoveryInput {
    pub username: String,
    #[ts(type = "string")]
    pub recovery_code: SecretString,
    pub key_label: String,
}

/// Acknowledgement of the first setup's one-time recovery code.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct KeyRecoveryAcknowledgement {
    #[ts(type = "string")]
    pub recovery_code: SecretString,
}

/// A one-time display value; never put it in logs or persisted UI state.
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeyRecoveryResult {
    pub recovery_code: String,
}

/// Revoke an owned key after fresh confirmation; retain its audit history.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RevokeSecurityKeyInput {
    pub credential_id: SecurityKeyId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub password: Reauthentication,
    pub reason: String,
}
