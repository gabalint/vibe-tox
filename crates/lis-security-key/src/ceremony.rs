use std::sync::Arc;

use webauthn_authenticator_rs::error::{CtapError, WebauthnCError};
#[cfg(target_os = "windows")]
use webauthn_rs_proto::{CreationChallengeResponse, RequestChallengeResponse};
use webauthn_rs_proto::{PublicKeyCredential, RegisterPublicKeyCredential};

use crate::{KeyInteraction, KeyStatus, SecurityKeyError, registration, request};

/// Register an external key with the engine's explicit verification policy (NFR-19).
/// `origin` and `options` must come directly from the trusted engine ceremony.
/// Native responses are unverified until the engine checks its private state.
pub async fn register(
    origin: &str,
    options: serde_json::Value,
    interaction: KeyInteraction,
) -> Result<RegisterPublicKeyCredential, SecurityKeyError> {
    let options = request::registration(options)?;
    let policy = options
        .public_key
        .authenticator_selection
        .as_ref()
        .ok_or(SecurityKeyError::InvalidRequest)?
        .user_verification;
    let origin = request::origin(origin, &options.public_key.rp.id)?;
    run(interaction, move |interaction| {
        let response = register_native(origin, options, interaction)?;
        registration::normalize(response, policy)
    })
    .await
}

/// Request a fresh assertion from an account's selected hardware key (NFR-19).
/// The engine must consume the challenge once and independently verify the result.
pub async fn authenticate(
    origin: &str,
    options: serde_json::Value,
    interaction: KeyInteraction,
) -> Result<PublicKeyCredential, SecurityKeyError> {
    let options = request::authentication(options)?;
    let origin = request::origin(origin, &options.public_key.rp_id)?;
    run(interaction, move |interaction| {
        authenticate_native(origin, options, interaction)
    })
    .await
}

async fn run<T: Send + 'static>(
    interaction: KeyInteraction,
    operation: impl FnOnce(&KeyInteraction) -> Result<T, SecurityKeyError> + Send + 'static,
) -> Result<T, SecurityKeyError> {
    interaction.shared.check()?;
    interaction.status(KeyStatus::WaitingForTouch);
    let shared = Arc::clone(&interaction.shared);
    let task = tokio::task::spawn_blocking(move || {
        // Upstream traces protocol messages. Suppress them for the whole native
        // ceremony so diagnostic configuration cannot disclose credential data.
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            interaction.shared.check()?;
            let result = operation(&interaction);
            interaction.shared.check()?;
            result
        })
    });
    tokio::select! {
        biased;
        error = shared.stopped() => Err(error),
        result = task => {
            shared.check()?;
            result.map_err(|_| SecurityKeyError::NativeFailure)?
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::usb::{authenticate as authenticate_native, register as register_native};

#[cfg(target_os = "windows")]
fn register_native(
    origin: url::Url,
    options: CreationChallengeResponse,
    _interaction: &KeyInteraction,
) -> Result<RegisterPublicKeyCredential, SecurityKeyError> {
    use webauthn_authenticator_rs::{WebauthnAuthenticator, win10::Win10};
    let response = WebauthnAuthenticator::new(Win10::default())
        .do_registration(origin, options)
        .map_err(native_error)?;
    // Win32 reports the transport used. Require USB rather than accepting an
    // unrecognized hybrid/platform path from the OS credential picker.
    if response.response.transports.as_deref()
        != Some(&[webauthn_rs_proto::AuthenticatorTransport::Usb])
    {
        return Err(SecurityKeyError::UnsupportedKey);
    }
    Ok(response)
}

#[cfg(target_os = "windows")]
fn authenticate_native(
    origin: url::Url,
    options: RequestChallengeResponse,
    _interaction: &KeyInteraction,
) -> Result<PublicKeyCredential, SecurityKeyError> {
    use webauthn_authenticator_rs::{WebauthnAuthenticator, win10::Win10};
    WebauthnAuthenticator::new(Win10::default())
        .do_authentication(origin, options)
        .map_err(native_error)
}

pub(crate) fn native_error(error: WebauthnCError) -> SecurityKeyError {
    match error {
        WebauthnCError::Cancelled | WebauthnCError::Ctap(CtapError::Ctap2KeepAliveCancel) => {
            SecurityKeyError::Cancelled
        }
        WebauthnCError::NoHidDevices
        | WebauthnCError::NoSelectedToken
        | WebauthnCError::PermissionDenied => SecurityKeyError::NoKey,
        WebauthnCError::Ctap(CtapError::Ctap2PinInvalid | CtapError::Ctap2PinAuthInvalid)
        | WebauthnCError::PinTooShort
        | WebauthnCError::PinTooLong
        | WebauthnCError::PinContainsNull => SecurityKeyError::InvalidPin,
        WebauthnCError::Ctap(CtapError::Ctap2PinNotSet) => SecurityKeyError::PinNotSet,
        WebauthnCError::Ctap(
            CtapError::Ctap2PinBlocked
            | CtapError::Ctap2PinAuthBlocked
            | CtapError::Ctap2UserVerificationBlocked,
        ) => SecurityKeyError::PinBlocked,
        WebauthnCError::Ctap(CtapError::Ctap2NoCredentials | CtapError::Ctap2InvalidCredential) => {
            SecurityKeyError::CredentialNotFound
        }
        WebauthnCError::Ctap(
            CtapError::Ctap1Timeout
            | CtapError::Ctap2UserActionTimeout
            | CtapError::Ctap2ActionTimeout,
        ) => SecurityKeyError::TimedOut,
        WebauthnCError::NotSupported
        | WebauthnCError::UserVerificationRequired
        | WebauthnCError::Ctap(
            CtapError::Ctap2UnsupportedOption | CtapError::Ctap2UnsupportedAlgorithm,
        ) => SecurityKeyError::UnsupportedKey,
        _ => SecurityKeyError::NativeFailure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn successful_native_response_after_cancellation_is_discarded() {
        let (interaction, control, _events) = crate::interaction();
        let result = run(interaction, move |_| {
            control.cancel();
            Ok::<_, SecurityKeyError>("synthetic assertion")
        })
        .await;
        assert_eq!(result, Err(SecurityKeyError::Cancelled));
    }
}
