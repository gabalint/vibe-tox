//! Random offline recovery material and proof digests; no custom cryptography.

use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use crate::cipher::random_bytes;
use crate::error::AuthError;
use crate::password::{self, NewPassword};

pub(crate) fn random_secret() -> Result<SecretString, AuthError> {
    let bytes = random_bytes::<32>()?;
    let text = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(SecretString::from(text))
}

pub(crate) fn digest(secret: &SecretString) -> Vec<u8> {
    Sha256::digest(secret.expose_secret().as_bytes()).to_vec()
}

pub(crate) fn hash_generated(secret: SecretString) -> Result<String, AuthError> {
    let password =
        NewPassword::parse(secret, "").map_err(|_| AuthError::SecurityKeyConfiguration)?;
    password::hash(&password)
}
