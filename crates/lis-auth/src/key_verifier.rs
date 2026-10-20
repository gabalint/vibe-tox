//! Trusted WebAuthn verification. Native transport never decides authentication.
//! Ceremony state remains in Rust; the engine bounds its lifetime and binds it
//! to the unchanged window/session and exact pending operation (REQ-IDN-004).

use serde_json::Value;
use webauthn_rs::prelude::{
    AuthenticationResult, AuthenticatorAttachment, PasskeyAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, SecurityKeyAuthentication,
    SecurityKeyRegistration, Url, Uuid,
};
use webauthn_rs::{Webauthn, WebauthnBuilder};

use crate::error::AuthError;
use crate::key_authentication::{KeyAuthenticationAccount, KeyAuthenticationPurpose};
use crate::key_credential::KeyCredential;
use crate::key_registration::{KeyRegistrationAccount, RegistrationAuthority};

/// Fixed relying-party configuration supplied by trusted engine code.
pub struct SecurityKeyVerifier(Webauthn);

/// Server-side registration state; deliberately neither serializable nor cloneable.
pub struct PendingKeyRegistration {
    account: KeyRegistrationAccount,
    state: RegistrationState,
}

/// Server-side authentication state; deliberately neither serializable nor cloneable.
pub struct PendingKeyAuthentication {
    account: KeyAuthenticationAccount,
    state: AuthenticationState,
    require_verification: bool,
}

/// A library-verified registration, consumed once by an authorized domain command.
pub struct VerifiedKeyRegistration {
    pub(crate) account: KeyRegistrationAccount,
    pub(crate) credential: KeyCredential,
}

/// A library-verified assertion; its snapshots are rechecked in the final transaction.
pub struct VerifiedKeyAuthentication {
    pub(crate) account: KeyAuthenticationAccount,
    pub(crate) result: AuthenticationResult,
}

enum RegistrationState {
    Required(PasskeyRegistration),
    Preferred(SecurityKeyRegistration),
}

enum AuthenticationState {
    Passkey(PasskeyAuthentication),
    SecurityKey(SecurityKeyAuthentication),
}

impl SecurityKeyVerifier {
    /// Builds one verifier for the installation's stable RP identity and origin.
    pub fn new(rp_id: &str, origin: &str) -> Result<Self, AuthError> {
        let origin = Url::parse(origin).map_err(|_| AuthError::SecurityKeyConfiguration)?;
        let verifier = WebauthnBuilder::new(rp_id, &origin)
            .map_err(|_| AuthError::SecurityKeyConfiguration)?
            .rp_name("Vibe Toxicology LIS")
            .build()
            .map_err(|_| AuthError::SecurityKeyConfiguration)?;
        Ok(Self(verifier))
    }

    /// Requests an external key under the frozen laboratory enrollment policy.
    pub fn begin_registration(
        &self,
        account: KeyRegistrationAccount,
    ) -> Result<(Value, PendingKeyRegistration), AuthError> {
        let user_id = Uuid::parse_str(&account.user_id.to_string())
            .map_err(|_| AuthError::SecurityKeyConfiguration)?;
        let exclude = account
            .existing
            .iter()
            .map(|key| key.credential.cred_id().clone())
            .collect();
        let optional = account.policy.allow_touch_only
            && matches!(account.authority, RegistrationAuthority::Own { .. });
        let (options, state) = if optional {
            let (options, state) = self
                .0
                .start_securitykey_registration(
                    user_id,
                    &account.username,
                    &account.display_name,
                    Some(exclude),
                    None,
                    Some(AuthenticatorAttachment::CrossPlatform),
                )
                .map_err(|_| AuthError::SecurityKeyConfiguration)?;
            (options, RegistrationState::Preferred(state))
        } else {
            let (options, state) = self
                .0
                .start_passkey_registration(
                    user_id,
                    &account.username,
                    &account.display_name,
                    Some(exclude),
                )
                .map_err(|_| AuthError::SecurityKeyConfiguration)?;
            (options, RegistrationState::Required(state))
        };
        let mut options =
            serde_json::to_value(options).map_err(|_| AuthError::SecurityKeyConfiguration)?;
        options["publicKey"]["authenticatorSelection"]["authenticatorAttachment"] =
            Value::String("cross-platform".to_owned());
        if optional {
            // 0.5.5 opportunistically requests credProtect=UVRequired, even for
            // Preferred keys. Omit that unenforced transport hint so a no-PIN
            // key is usable. The library keeps the signed actual extension and
            // original Preferred policy; no private state/metadata is rewritten.
            if let Some(extensions) = options["publicKey"]["extensions"].as_object_mut() {
                extensions.remove("credentialProtectionPolicy");
                extensions.remove("enforceCredentialProtectionPolicy");
            }
        }
        Ok((options, PendingKeyRegistration { account, state }))
    }

    /// Checks RP/origin/challenge, attestation structure, possession and the
    /// requested verification policy; rejects backup-capable credentials.
    pub fn finish_registration(
        &self,
        pending: PendingKeyRegistration,
        response: Value,
    ) -> Result<VerifiedKeyRegistration, AuthError> {
        let response: RegisterPublicKeyCredential =
            serde_json::from_value(response).map_err(|_| AuthError::SecurityKeyVerification)?;
        let (credential, require_verification) = match pending.state {
            RegistrationState::Required(state) => (
                KeyCredential::Passkey(
                    self.0
                        .finish_passkey_registration(&response, &state)
                        .map_err(|_| AuthError::SecurityKeyVerification)?,
                ),
                true,
            ),
            RegistrationState::Preferred(state) => (
                KeyCredential::SecurityKey {
                    security_key: self
                        .0
                        .finish_securitykey_registration(&response, &state)
                        .map_err(|_| AuthError::SecurityKeyVerification)?,
                },
                false,
            ),
        };
        let metadata = credential.metadata()?;
        if (require_verification && !metadata.user_verified)
            || metadata.backup_eligible
            || metadata.backup_state
        {
            return Err(AuthError::SecurityKeyVerification);
        }
        Ok(VerifiedKeyRegistration {
            account: pending.account,
            credential,
        })
    }

    /// Exactly one explicitly selected key is offered to the native transport.
    pub fn begin_authentication(
        &self,
        account: KeyAuthenticationAccount,
    ) -> Result<(Value, PendingKeyAuthentication), AuthError> {
        let require_verification = account.key.credential.metadata()?.user_verified
            || matches!(account.key.credential, KeyCredential::Passkey(_))
            || !account.policy.allow_touch_only
            || account.purpose == KeyAuthenticationPurpose::SensitiveAction;
        let (options, state) = match &account.key.credential {
            KeyCredential::Passkey(key) => {
                let (options, state) = self
                    .0
                    .start_passkey_authentication(std::slice::from_ref(key))
                    .map_err(|_| AuthError::SecurityKeyConfiguration)?;
                (options, AuthenticationState::Passkey(state))
            }
            KeyCredential::SecurityKey { security_key } => {
                let (options, state) = self
                    .0
                    .start_securitykey_authentication(std::slice::from_ref(security_key))
                    .map_err(|_| AuthError::SecurityKeyConfiguration)?;
                (options, AuthenticationState::SecurityKey(state))
            }
        };
        let mut options =
            serde_json::to_value(options).map_err(|_| AuthError::SecurityKeyConfiguration)?;
        if require_verification {
            // Strengthen the client request and independently enforce the signed
            // result below. Historical verifier registration policy is preserved.
            options["publicKey"]["userVerification"] = Value::String("required".to_owned());
        }
        Ok((
            options,
            PendingKeyAuthentication {
                account,
                state,
                require_verification,
            },
        ))
    }

    /// Verifies the assertion and rejects cloned counters and backup transitions.
    /// The engine must record failures with `record_key_failure` after refusal.
    pub fn finish_authentication(
        &self,
        pending: PendingKeyAuthentication,
        response: Value,
    ) -> Result<VerifiedKeyAuthentication, AuthError> {
        let response: PublicKeyCredential =
            serde_json::from_value(response).map_err(|_| AuthError::SecurityKeyVerification)?;
        let expected_user = Uuid::parse_str(&pending.account.user_id.to_string())
            .map_err(|_| AuthError::SecurityKeyConfiguration)?;
        if response
            .get_user_unique_id()
            .is_some_and(|handle| handle != expected_user.as_bytes())
        {
            return Err(AuthError::SecurityKeyVerification);
        }
        let result = match pending.state {
            AuthenticationState::Passkey(state) => {
                self.0.finish_passkey_authentication(&response, &state)
            }
            AuthenticationState::SecurityKey(state) => {
                self.0.finish_securitykey_authentication(&response, &state)
            }
        }
        .map_err(|_| AuthError::SecurityKeyVerification)?;
        let previous = pending.account.key.credential.metadata()?;
        if (pending.require_verification && !result.user_verified())
            || result.backup_eligible()
            || result.backup_state()
            || (previous.counter > 0 && result.counter() <= previous.counter)
        {
            return Err(AuthError::SecurityKeyVerification);
        }
        Ok(VerifiedKeyAuthentication {
            account: pending.account,
            result,
        })
    }
}
