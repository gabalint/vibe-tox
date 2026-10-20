//! Encryption of authentication secrets at rest (security-audit-logging.md, "Secrets and
//! credentials"), OS randomness, and the checks that tell whether a key
//! belongs to this database.
//!
//! A secret is sealed with ChaCha20-Poly1305 under the [`DataKey`], with a
//! fresh random 96-bit nonce stored in front of the ciphertext. The associated
//! data names the column and the user, so a sealed secret copied to another
//! account's row fails to open.

use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use lis_core::UserId;
use rand::TryRng;
use rand::rngs::SysRng;
use rusqlite::Connection;
use zeroize::Zeroizing;

use crate::error::AuthError;
use crate::repo;

const NONCE_BYTES: usize = 12;

/// The 256-bit key that seals TOTP secrets and PIN verifiers. The engine keeps it in the OS key
/// store; `Debug` never prints it.
#[derive(Clone)]
pub struct DataKey(Zeroizing<[u8; 32]>);

impl DataKey {
    /// A key from its bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// A new random key from the OS random number generator.
    pub fn generate() -> Result<Self, AuthError> {
        random_bytes().map(Self::from_bytes)
    }

    /// The key bytes, for storing the key in the OS key store.
    #[must_use]
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl TryFrom<&[u8]> for DataKey {
    type Error = AuthError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let bytes = <[u8; 32]>::try_from(bytes).map_err(|_| AuthError::DataKeyLength)?;
        Ok(Self::from_bytes(bytes))
    }
}

impl fmt::Debug for DataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DataKey([redacted])")
    }
}

/// `N` bytes from the OS random number generator.
pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], AuthError> {
    let mut bytes = [0; N];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| AuthError::Random)?;
    Ok(bytes)
}

/// Stable column-specific authenticated context; TOTP keeps its original bytes.
#[derive(Clone, Copy)]
pub(crate) enum SecretPurpose {
    Totp,
    UnlockPin,
}

impl SecretPurpose {
    fn associated_data(self, user_id: UserId) -> String {
        let column = match self {
            Self::Totp => "auth_users.totp_secret",
            Self::UnlockPin => "auth_unlock_pins.verifier",
        };
        format!("{column}:{user_id}")
    }
}

/// Nonce followed by the ciphertext and tag of `secret`, bound to `user_id`.
pub(crate) fn seal(
    key: &DataKey,
    user_id: UserId,
    purpose: SecretPurpose,
    secret: &[u8],
) -> Result<Vec<u8>, AuthError> {
    let nonce_bytes: [u8; NONCE_BYTES] = random_bytes()?;
    let aad = purpose.associated_data(user_id);
    let ciphertext = ChaCha20Poly1305::new(&Key::from(*key.expose()))
        .encrypt(
            &Nonce::from(nonce_bytes),
            Payload {
                msg: secret,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| AuthError::SecretCipher)?;
    let mut sealed = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    sealed.extend_from_slice(&nonce_bytes);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// The secret sealed by [`seal`] for `user_id`; fails if anything was altered.
pub(crate) fn open(
    key: &DataKey,
    user_id: UserId,
    purpose: SecretPurpose,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, AuthError> {
    let (nonce, ciphertext) = sealed
        .split_at_checked(NONCE_BYTES)
        .ok_or(AuthError::SecretCipher)?;
    let nonce = <[u8; NONCE_BYTES]>::try_from(nonce).map_err(|_| AuthError::SecretCipher)?;
    let aad = purpose.associated_data(user_id);
    ChaCha20Poly1305::new(&Key::from(*key.expose()))
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| AuthError::SecretCipher)
}

/// True when any account holds a sealed TOTP secret or PIN verifier. The engine creates a new
/// installation key only for a database without one (M4 §3.9): a new key
/// could never open existing secrets and would lock out every TOTP user.
pub fn has_sealed_secrets(conn: &Connection) -> Result<bool, AuthError> {
    Ok(!repo::sealed_secrets(conn)?.is_empty()
        || !crate::unlock_pin_repo::sealed_verifiers(conn)?.is_empty())
}

/// True when `key` opens at least one sealed authentication secret: it belongs to
/// this database. False when it opens none, including when no secret is
/// sealed; call [`has_sealed_secrets`] to tell the two apart. Nothing is
/// written and no secret leaves this function.
pub fn key_opens_sealed_secrets(conn: &Connection, key: &DataKey) -> Result<bool, AuthError> {
    Ok(repo::sealed_secrets(conn)?
        .iter()
        .any(|(user_id, sealed)| open(key, *user_id, SecretPurpose::Totp, sealed).is_ok())
        || crate::unlock_pin_repo::sealed_verifiers(conn)?
            .iter()
            .any(|(user_id, sealed)| open(key, *user_id, SecretPurpose::UnlockPin, sealed).is_ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> DataKey {
        DataKey::from_bytes([byte; 32])
    }

    #[test]
    fn sealed_secret_opens_with_the_same_key_and_user() {
        let user = UserId::generate();
        let sealed = seal(
            &key(1),
            user,
            SecretPurpose::Totp,
            b"synthetic-secret-bytes",
        )
        .unwrap();
        assert_eq!(sealed.len(), NONCE_BYTES + 22 + 16);
        assert_eq!(
            open(&key(1), user, SecretPurpose::Totp, &sealed)
                .unwrap()
                .as_slice(),
            b"synthetic-secret-bytes"
        );
        assert_ne!(
            seal(&key(1), user, SecretPurpose::Totp, b"same").unwrap(),
            seal(&key(1), user, SecretPurpose::Totp, b"same").unwrap()
        );
    }

    #[test]
    fn open_refuses_another_key_another_user_or_altered_bytes() {
        let user = UserId::generate();
        let sealed = seal(&key(1), user, SecretPurpose::Totp, b"secret").unwrap();
        assert!(open(&key(2), user, SecretPurpose::Totp, &sealed).is_err());
        assert!(open(&key(1), UserId::generate(), SecretPurpose::Totp, &sealed).is_err());
        let mut altered = sealed.clone();
        altered[NONCE_BYTES] ^= 1;
        assert!(open(&key(1), user, SecretPurpose::Totp, &altered).is_err());
        assert!(open(&key(1), user, SecretPurpose::Totp, &sealed[..5]).is_err());
    }

    fn database_with_users(users: &[UserId]) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::MIGRATIONS[0].sql).unwrap();
        conn.execute_batch(crate::MIGRATIONS[7].sql).unwrap();
        for (n, user) in users.iter().enumerate() {
            conn.execute(
                "INSERT INTO auth_users (id, username, display_name, status, password_hash, \
                 must_change_password, created_at, updated_at, revision) VALUES (?1, ?2, 'Test', \
                 'active', '$argon2id$synthetic', 0, 't', 't', 1)",
                rusqlite::params![user, format!("user{n}")],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn only_the_sealing_key_opens_a_databases_sealed_secrets() {
        let (enrolled, other) = (UserId::generate(), UserId::generate());
        let conn = database_with_users(&[enrolled, other]);
        assert!(!has_sealed_secrets(&conn).unwrap());
        assert!(!key_opens_sealed_secrets(&conn, &key(1)).unwrap());
        let sealed = seal(&key(1), enrolled, SecretPurpose::Totp, b"synthetic-secret").unwrap();
        repo::set_totp(&conn, enrolled, Some((&sealed, 0))).unwrap();
        assert!(has_sealed_secrets(&conn).unwrap());
        assert!(key_opens_sealed_secrets(&conn, &key(1)).unwrap());
        assert!(!key_opens_sealed_secrets(&conn, &key(2)).unwrap());
    }

    #[test]
    fn pin_verifier_has_separate_authenticated_context_and_salted_argon2() {
        use secrecy::SecretString;
        let user = UserId::generate();
        let pin = SecretString::from("482905");
        let hash = Zeroizing::new(crate::password::hash_secret(&pin).unwrap());
        assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert_ne!(*hash, crate::password::hash_secret(&pin).unwrap());
        let sealed = seal(&key(1), user, SecretPurpose::UnlockPin, hash.as_bytes()).unwrap();
        assert_eq!(
            open(&key(1), user, SecretPurpose::UnlockPin, &sealed)
                .unwrap()
                .as_slice(),
            hash.as_bytes()
        );
        assert!(open(&key(1), user, SecretPurpose::Totp, &sealed).is_err());
        assert!(
            open(
                &key(1),
                UserId::generate(),
                SecretPurpose::UnlockPin,
                &sealed
            )
            .is_err()
        );
        assert!(open(&key(2), user, SecretPurpose::UnlockPin, &sealed).is_err());
        assert_eq!(
            SecretPurpose::Totp.associated_data(user),
            format!("auth_users.totp_secret:{user}")
        );
    }

    #[test]
    fn data_key_is_redacted_and_length_checked() {
        assert_eq!(format!("{:?}", key(7)), "DataKey([redacted])");
        assert!(DataKey::try_from([0_u8; 31].as_slice()).is_err());
        assert_eq!(
            DataKey::try_from([9_u8; 32].as_slice()).unwrap().expose(),
            &[9; 32]
        );
        assert_ne!(
            DataKey::generate().unwrap().expose(),
            DataKey::generate().unwrap().expose()
        );
    }
}
