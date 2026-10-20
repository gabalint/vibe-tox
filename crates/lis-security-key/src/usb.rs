use std::sync::Arc;

use tokio::runtime::Handle;
use webauthn_authenticator_rs::{
    WebauthnAuthenticator,
    ctap2::{CtapAuthenticator, commands::GetInfoResponse},
    transport::Transport,
    types::{CableRequestType, CableState, EnrollSampleStatus},
    ui::UiCallback,
    usb::USBTransport,
};
use webauthn_rs_proto::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse, UserVerificationPolicy,
};

use crate::{KeyInteraction, KeyStatus, SecurityKeyError, usb_token::BoundedUsbToken};

impl UiCallback for KeyInteraction {
    fn request_pin(&self) -> Option<String> {
        self.request_pin()
    }
    fn request_touch(&self) {
        self.status(KeyStatus::WaitingForTouch);
    }
    fn processing(&self) {
        self.status(KeyStatus::Processing);
    }
    fn fingerprint_enrollment_feedback(
        &self,
        _remaining: u32,
        _feedback: Option<EnrollSampleStatus>,
    ) {
    }
    fn cable_qr_code(&self, _request: CableRequestType, _url: String) {}
    fn dismiss_qr_code(&self) {}
    fn cable_status_update(&self, _state: CableState) {}
}

async fn device(interaction: &KeyInteraction) -> Result<BoundedUsbToken, SecurityKeyError> {
    let transport = USBTransport::new()
        .await
        .map_err(super::ceremony::native_error)?;
    let mut tokens = transport
        .tokens()
        .await
        .map_err(super::ceremony::native_error)?;
    if tokens.len() > 1 {
        return Err(SecurityKeyError::MultipleKeys);
    }
    let token = tokens.pop().ok_or(SecurityKeyError::NoKey)?;
    Ok(BoundedUsbToken::new(token, Arc::clone(&interaction.shared)))
}

fn authenticator(
    interaction: &KeyInteraction,
) -> Result<CtapAuthenticator<'_, BoundedUsbToken, KeyInteraction>, SecurityKeyError> {
    let runtime = Handle::current();
    let token = runtime.block_on(async {
        tokio::select! {
            biased;
            error = interaction.shared.stopped() => Err(error),
            token = device(interaction) => token,
        }
    })?;
    runtime
        .block_on(CtapAuthenticator::new(token, interaction))
        .ok_or(SecurityKeyError::UnsupportedKey)
}

fn verification_policy(
    requested: UserVerificationPolicy,
    info: &GetInfoResponse,
) -> Result<UserVerificationPolicy, SecurityKeyError> {
    let client_pin = info.get_option("clientPin");
    let uv = info.get_option("uv").unwrap_or(false);
    let always_uv = info.get_option("alwaysUv").unwrap_or(false);
    if requested == UserVerificationPolicy::Required && client_pin == Some(false) && !uv {
        return Err(SecurityKeyError::PinNotSet);
    }
    if requested == UserVerificationPolicy::Preferred
        && client_pin != Some(true)
        && !uv
        && !always_uv
    {
        // Upstream 0.5.5 treats Preferred like Required on an unconfigured key.
        // Adapt only from initial capabilities, before any PIN/UV attempt. A
        // configured key still uses verification; failures never trigger retry.
        return Ok(UserVerificationPolicy::Discouraged_DO_NOT_USE);
    }
    Ok(requested)
}

pub(crate) fn register(
    origin: url::Url,
    mut options: CreationChallengeResponse,
    interaction: &KeyInteraction,
) -> Result<RegisterPublicKeyCredential, SecurityKeyError> {
    let authenticator = authenticator(interaction)?;
    let selection = options
        .public_key
        .authenticator_selection
        .as_mut()
        .ok_or(SecurityKeyError::InvalidRequest)?;
    selection.user_verification =
        verification_policy(selection.user_verification, authenticator.get_info())?;
    WebauthnAuthenticator::new(authenticator)
        .do_registration(origin, options)
        .map_err(super::ceremony::native_error)
}

pub(crate) fn authenticate(
    origin: url::Url,
    mut options: RequestChallengeResponse,
    interaction: &KeyInteraction,
) -> Result<PublicKeyCredential, SecurityKeyError> {
    let authenticator = authenticator(interaction)?;
    options.public_key.user_verification = verification_policy(
        options.public_key.user_verification,
        authenticator.get_info(),
    )?;
    WebauthnAuthenticator::new(authenticator)
        .do_authentication(origin, options)
        .map_err(super::ceremony::native_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(options: &[(&str, bool)]) -> GetInfoResponse {
        GetInfoResponse {
            options: Some(
                options
                    .iter()
                    .map(|(key, value)| ((*key).into(), *value))
                    .collect(),
            ),
            ..GetInfoResponse::default()
        }
    }

    #[test]
    fn unset_pin_is_refused_only_when_verification_is_required() {
        let info = info(&[("clientPin", false)]);
        assert_eq!(
            verification_policy(UserVerificationPolicy::Required, &info),
            Err(SecurityKeyError::PinNotSet)
        );
        assert_eq!(
            verification_policy(UserVerificationPolicy::Preferred, &info),
            Ok(UserVerificationPolicy::Discouraged_DO_NOT_USE)
        );
        assert_eq!(
            verification_policy(
                UserVerificationPolicy::Preferred,
                &GetInfoResponse::default()
            ),
            Ok(UserVerificationPolicy::Discouraged_DO_NOT_USE)
        );
    }

    #[test]
    fn configured_or_always_required_verification_is_never_relaxed() {
        for option in ["clientPin", "uv", "alwaysUv"] {
            let info = info(&[(option, true)]);
            assert_eq!(
                verification_policy(UserVerificationPolicy::Preferred, &info),
                Ok(UserVerificationPolicy::Preferred)
            );
            assert_eq!(
                verification_policy(UserVerificationPolicy::Required, &info),
                Ok(UserVerificationPolicy::Required)
            );
        }
    }
}
