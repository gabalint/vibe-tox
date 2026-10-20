use ciborium::Value;
use webauthn_rs_proto::{RegisterPublicKeyCredential, UserVerificationPolicy};

use crate::SecurityKeyError;

const MAX_ATTESTATION_BYTES: usize = 65_536;

// Native upstream USB ceremonies leave id/rawId empty and put the attestation
// format in type. Reconstruct the envelope; the engine still verifies the CBOR,
// flags, key, RP and challenge independently before storing a credential.
pub(crate) fn normalize(
    mut response: RegisterPublicKeyCredential,
    policy: UserVerificationPolicy,
) -> Result<RegisterPublicKeyCredential, SecurityKeyError> {
    let data = credential_id(response.response.attestation_object.as_slice(), policy)?;
    if !response.raw_id.is_empty() && response.raw_id.as_slice() != data.as_slice() {
        return Err(SecurityKeyError::InvalidResponse);
    }
    response.raw_id = data.into();
    let encoded =
        serde_json::to_value(&response.raw_id).map_err(|_| SecurityKeyError::InvalidResponse)?;
    response.id = encoded
        .as_str()
        .ok_or(SecurityKeyError::InvalidResponse)?
        .to_owned();
    response.type_ = "public-key".to_owned();
    Ok(response)
}

fn credential_id(
    bytes: &[u8],
    policy: UserVerificationPolicy,
) -> Result<Vec<u8>, SecurityKeyError> {
    let data = authenticator_data(bytes)?;
    // 32-byte RP hash + flags + counter + 16-byte AAGUID + 2-byte ID length.
    let required_flags = match policy {
        UserVerificationPolicy::Required => 0x45,  // UP + UV + AT.
        UserVerificationPolicy::Preferred => 0x41, // UP + AT.
        UserVerificationPolicy::Discouraged_DO_NOT_USE => {
            return Err(SecurityKeyError::InvalidRequest);
        }
    };
    if data.len() < 55 || data[32] & required_flags != required_flags {
        return Err(SecurityKeyError::InvalidResponse);
    }
    let length = usize::from(u16::from_be_bytes([data[53], data[54]]));
    if length == 0 || length > 1024 {
        return Err(SecurityKeyError::InvalidResponse);
    }
    data.get(55..55 + length)
        .map(<[u8]>::to_vec)
        .ok_or(SecurityKeyError::InvalidResponse)
}

fn authenticator_data(bytes: &[u8]) -> Result<Vec<u8>, SecurityKeyError> {
    if bytes.len() > MAX_ATTESTATION_BYTES {
        return Err(SecurityKeyError::InvalidResponse);
    }
    let mut input = bytes;
    let value: Value = ciborium::de::from_reader_with_recursion_limit(&mut input, 16)
        .map_err(|_| SecurityKeyError::InvalidResponse)?;
    if !input.is_empty() {
        return Err(SecurityKeyError::InvalidResponse);
    }
    let Value::Map(entries) = value else {
        return Err(SecurityKeyError::InvalidResponse);
    };
    let mut auth_data = None;
    for (key, value) in entries {
        if key == Value::Text("authData".to_owned()) {
            if auth_data.is_some() {
                return Err(SecurityKeyError::InvalidResponse);
            }
            let Value::Bytes(bytes) = value else {
                return Err(SecurityKeyError::InvalidResponse);
            };
            auth_data = Some(bytes);
        }
    }
    auth_data.ok_or(SecurityKeyError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attestation(flags: u8, length: u16, id: &[u8]) -> Vec<u8> {
        let mut data = vec![0; 55];
        data[32] = flags;
        data[53..55].copy_from_slice(&length.to_be_bytes());
        data.extend_from_slice(id);
        let mut encoded = Vec::new();
        ciborium::into_writer(
            &Value::Map(vec![(Value::Text("authData".into()), Value::Bytes(data))]),
            &mut encoded,
        )
        .unwrap();
        encoded
    }

    #[test]
    fn malformed_unverified_and_truncated_registration_are_refused() {
        assert_eq!(
            credential_id(&[0xff], UserVerificationPolicy::Required),
            Err(SecurityKeyError::InvalidResponse)
        );
        assert_eq!(
            credential_id(
                &attestation(0x41, 1, &[7]),
                UserVerificationPolicy::Required
            ),
            Err(SecurityKeyError::InvalidResponse)
        );
        assert_eq!(
            credential_id(
                &attestation(0x45, 2, &[7]),
                UserVerificationPolicy::Required
            ),
            Err(SecurityKeyError::InvalidResponse)
        );
        assert_eq!(
            credential_id(
                &attestation(0x45, 1, &[7]),
                UserVerificationPolicy::Required
            ),
            Ok(vec![7])
        );
    }

    #[test]
    fn optional_verification_accepts_touch_but_always_requires_presence_and_attestation() {
        assert_eq!(
            credential_id(
                &attestation(0x41, 1, &[7]),
                UserVerificationPolicy::Preferred
            ),
            Ok(vec![7])
        );
        for flags in [0x40, 0x44, 0x01, 0x05] {
            assert_eq!(
                credential_id(
                    &attestation(flags, 1, &[7]),
                    UserVerificationPolicy::Preferred
                ),
                Err(SecurityKeyError::InvalidResponse)
            );
        }
    }
}
