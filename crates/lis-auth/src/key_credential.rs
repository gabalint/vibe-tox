//! Persist the verifier's credential kind without changing its historical policy.

use serde::{Deserialize, Serialize};
use webauthn_rs::prelude::{AuthenticationResult, CredentialID, Passkey, SecurityKey};

use crate::error::AuthError;

/// Existing Passkey JSON remains byte-shape compatible; new optional-UV keys
/// have an explicit envelope. Neither variant rewrites verifier metadata.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum KeyCredential {
    SecurityKey {
        #[serde(rename = "securityKey")]
        security_key: SecurityKey,
    },
    Passkey(Passkey),
}

impl KeyCredential {
    pub(crate) fn cred_id(&self) -> &CredentialID {
        match self {
            Self::Passkey(key) => key.cred_id(),
            Self::SecurityKey { security_key } => security_key.cred_id(),
        }
    }

    pub(crate) fn update_credential(&mut self, result: &AuthenticationResult) -> Option<bool> {
        match self {
            Self::Passkey(key) => key.update_credential(result),
            Self::SecurityKey { security_key } => security_key.update_credential(result),
        }
    }

    /// Read only the pinned library's validated serialization. An unexpected
    /// representation fails closed; raw client data never enters this path.
    pub(crate) fn metadata(&self) -> Result<CredentialMetadata, AuthError> {
        let serialized = match self {
            Self::Passkey(key) => serde_json::to_value(key),
            Self::SecurityKey { security_key } => serde_json::to_value(security_key),
        }
        .map_err(|_| AuthError::SecurityKeyConfiguration)?;
        serde_json::from_value(
            serialized
                .get("cred")
                .cloned()
                .ok_or(AuthError::SecurityKeyConfiguration)?,
        )
        .map_err(|_| AuthError::SecurityKeyConfiguration)
    }
}

#[derive(Deserialize)]
pub(crate) struct CredentialMetadata {
    pub(crate) counter: u32,
    pub(crate) user_verified: bool,
    pub(crate) backup_eligible: bool,
    pub(crate) backup_state: bool,
}
