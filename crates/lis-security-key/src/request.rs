use url::Url;
use webauthn_rs_proto::{
    AuthenticatorAttachment, AuthenticatorSelectionCriteria, AuthenticatorTransport,
    CreationChallengeResponse, RequestChallengeResponse, ResidentKeyRequirement,
    UserVerificationPolicy,
};

use crate::SecurityKeyError;

pub(crate) fn origin(origin: &str, rp_id: &str) -> Result<Url, SecurityKeyError> {
    let origin = Url::parse(origin).map_err(|_| SecurityKeyError::InvalidRequest)?;
    if origin.scheme() != "https"
        || origin.host_str() != Some(rp_id)
        || origin.username() != ""
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
    {
        return Err(SecurityKeyError::InvalidRequest);
    }
    Ok(origin)
}

pub(crate) fn registration(
    options: serde_json::Value,
) -> Result<CreationChallengeResponse, SecurityKeyError> {
    let policy = verification_policy(
        &options,
        "/publicKey/authenticatorSelection/userVerification",
    )?;
    let mut request: CreationChallengeResponse =
        serde_json::from_value(options).map_err(|_| SecurityKeyError::InvalidRequest)?;
    let options = &mut request.public_key;
    if options.challenge.len() < 16
        || options.challenge.len() > 1024
        || options.user.id.is_empty()
        || options.user.id.len() > 64
    {
        return Err(SecurityKeyError::InvalidRequest);
    }
    // NFR-19: native ceremonies always require a personal external authenticator.
    options.authenticator_selection = Some(AuthenticatorSelectionCriteria {
        authenticator_attachment: Some(AuthenticatorAttachment::CrossPlatform),
        resident_key: Some(ResidentKeyRequirement::Discouraged),
        require_resident_key: false,
        user_verification: policy,
    });
    options.timeout = Some(60_000);
    // Upstream 0.5.5's WinCredentialList cannot represent a pointer list with
    // multiple elements safely. Engine registration still rejects duplicate IDs.
    #[cfg(target_os = "windows")]
    {
        options.exclude_credentials = None;
    }
    Ok(request)
}

pub(crate) fn authentication(
    options: serde_json::Value,
) -> Result<RequestChallengeResponse, SecurityKeyError> {
    verification_policy(&options, "/publicKey/userVerification")?;
    let mut request: RequestChallengeResponse =
        serde_json::from_value(options).map_err(|_| SecurityKeyError::InvalidRequest)?;
    let options = &mut request.public_key;
    if options.challenge.len() < 16
        || options.challenge.len() > 1024
        || options.allow_credentials.is_empty()
        || options.allow_credentials.len() > 32
        || options
            .allow_credentials
            .iter()
            .any(|c| c.id.is_empty() || c.id.len() > 1024)
    {
        return Err(SecurityKeyError::InvalidRequest);
    }
    #[cfg(target_os = "windows")]
    if options.allow_credentials.len() != 1 {
        return Err(SecurityKeyError::SelectCredential);
    }
    options.timeout = Some(60_000);
    for credential in &mut options.allow_credentials {
        credential.transports = Some(vec![AuthenticatorTransport::Usb]);
    }
    Ok(request)
}

fn verification_policy(
    options: &serde_json::Value,
    path: &str,
) -> Result<UserVerificationPolicy, SecurityKeyError> {
    // Only the trusted engine may choose optional verification. Missing or
    // unknown policies must never inherit a library's permissive default.
    match options.pointer(path).and_then(serde_json::Value::as_str) {
        Some("required") => Ok(UserVerificationPolicy::Required),
        Some("preferred") => Ok(UserVerificationPolicy::Preferred),
        _ => Err(SecurityKeyError::InvalidRequest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_origin_must_match_relying_party_exactly() {
        assert!(origin("https://lis.example", "lis.example").is_ok());
        assert!(origin("https://other.example", "lis.example").is_err());
        assert!(origin("http://lis.example", "lis.example").is_err());
        assert!(origin("https://user:secret@lis.example", "lis.example").is_err());
    }

    fn authentication_options(policy: &str) -> serde_json::Value {
        serde_json::json!({
            "publicKey": { "challenge": "AAAAAAAAAAAAAAAAAAAAAA", "rpId": "lis.example",
                "allowCredentials": [{ "type": "public-key", "id": "AQ" }],
                "userVerification": policy }
        })
    }

    fn registration_options(policy: &str) -> serde_json::Value {
        serde_json::json!({
            "publicKey": {
                "rp": { "id": "lis.example", "name": "Synthetic LIS" },
                "user": { "id": "AQ", "name": "synthetic", "displayName": "Synthetic" },
                "challenge": "AAAAAAAAAAAAAAAAAAAAAA",
                "pubKeyCredParams": [{ "type": "public-key", "alg": -7 }],
                "authenticatorSelection": {
                    "authenticatorAttachment": "platform", "requireResidentKey": true,
                    "userVerification": policy
                }
            }
        })
    }

    #[test]
    fn registration_preserves_explicit_policy_and_requires_external_key() {
        for (value, expected) in [
            ("required", UserVerificationPolicy::Required),
            ("preferred", UserVerificationPolicy::Preferred),
        ] {
            let options = registration(registration_options(value)).unwrap();
            let selection = options.public_key.authenticator_selection.unwrap();
            assert_eq!(selection.user_verification, expected);
            assert_eq!(
                selection.authenticator_attachment,
                Some(AuthenticatorAttachment::CrossPlatform)
            );
            assert_eq!(
                selection.resident_key,
                Some(ResidentKeyRequirement::Discouraged)
            );
            assert!(!selection.require_resident_key);
        }
    }

    #[test]
    fn registration_refuses_missing_or_unsupported_verification_policy() {
        for value in [serde_json::Value::Null, serde_json::json!("discouraged")] {
            let mut options = registration_options("required");
            options["publicKey"]["authenticatorSelection"]["userVerification"] = value;
            assert!(matches!(
                registration(options),
                Err(SecurityKeyError::InvalidRequest)
            ));
        }
        let mut options = registration_options("required");
        options["publicKey"]
            .as_object_mut()
            .unwrap()
            .remove("authenticatorSelection");
        assert!(matches!(
            registration(options),
            Err(SecurityKeyError::InvalidRequest)
        ));
    }

    #[test]
    fn explicit_policy_is_preserved_and_transport_remains_usb() {
        for (value, expected) in [
            ("required", UserVerificationPolicy::Required),
            ("preferred", UserVerificationPolicy::Preferred),
        ] {
            let options = authentication(authentication_options(value)).unwrap();
            assert_eq!(options.public_key.user_verification, expected);
            assert_eq!(
                options.public_key.allow_credentials[0].transports,
                Some(vec![AuthenticatorTransport::Usb])
            );
        }
    }

    #[test]
    fn account_discovery_and_missing_or_unsupported_policies_are_refused() {
        let mut options = authentication_options("required");
        options["publicKey"]["allowCredentials"] = serde_json::json!([]);
        assert!(matches!(
            authentication(options),
            Err(SecurityKeyError::InvalidRequest)
        ));
        for value in [
            serde_json::Value::Null,
            serde_json::json!("discouraged"),
            serde_json::json!("unknown"),
        ] {
            let mut options = authentication_options("required");
            options["publicKey"]["userVerification"] = value;
            assert!(matches!(
                authentication(options),
                Err(SecurityKeyError::InvalidRequest)
            ));
        }
        let mut options = authentication_options("required");
        options["publicKey"]
            .as_object_mut()
            .unwrap()
            .remove("userVerification");
        assert!(matches!(
            authentication(options),
            Err(SecurityKeyError::InvalidRequest)
        ));
    }
}
