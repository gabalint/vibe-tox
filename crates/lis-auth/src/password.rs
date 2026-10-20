//! Password policy and Argon2id hashing (NFR-19; security-audit-logging.md,
//! "Secrets and credentials").
//!
//! Parameters: Argon2id v1.3, 19 MiB memory, 2 iterations, 1 lane — the OWASP
//! Password Storage Cheat Sheet minimum, checked 2026-10-12
//! (<https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>).
//! The PHC string stores the parameters with the hash, so verification keeps
//! working after they are raised.

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{Error as HashError, PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretString};

use crate::account::normalize_username;
use crate::error::{AuthError, Problem};

const MIN_CHARS: usize = 12;
const MAX_CHARS: usize = 128;
const MEMORY_KIB: u32 = 19 * 1024;
const ITERATIONS: u32 = 2;
const LANES: u32 = 1;

/// Verified against when the username matches no account, so that unknown
/// usernames take as long as wrong passwords. Well-formed; matches no password.
const UNKNOWN_USER_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// A new password that meets the policy: 12–128 characters and not containing
/// the username. No composition rules (NIST SP 800-63B-4). A common-password
/// blocklist is an M5 item.
pub(crate) struct NewPassword(SecretString);

impl NewPassword {
    pub(crate) fn parse(password: SecretString, username: &str) -> Result<Self, Problem> {
        let text = password.expose_secret();
        let length = text.chars().count();
        if length < MIN_CHARS {
            return Err(Problem {
                code: "too_short",
                message: "Use at least 12 characters.",
            });
        }
        if length > MAX_CHARS {
            return Err(Problem {
                code: "too_long",
                message: "Use 128 characters or fewer.",
            });
        }
        let username = normalize_username(username);
        if !username.is_empty() && text.to_lowercase().contains(&username) {
            return Err(Problem {
                code: "contains_username",
                message: "The password must not contain the username.",
            });
        }
        Ok(Self(password))
    }

    pub(crate) fn secret(&self) -> &SecretString {
        &self.0
    }
}

fn argon2() -> Result<Argon2<'static>, AuthError> {
    let params =
        Params::new(MEMORY_KIB, ITERATIONS, LANES, None).map_err(|_| AuthError::PasswordHash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// The Argon2id PHC string for `password`, with a fresh random salt.
pub(crate) fn hash(password: &NewPassword) -> Result<String, AuthError> {
    hash_secret(password.secret())
}

/// Hashes a validated credential using the same salted Argon2id policy.
pub(crate) fn hash_secret(secret: &SecretString) -> Result<String, AuthError> {
    let hash = argon2()?
        .hash_password(secret.expose_secret().as_bytes())
        .map_err(|_| AuthError::PasswordHash)?;
    Ok(hash.to_string())
}

/// True when `password` matches the PHC string `stored` (constant-time compare).
pub(crate) fn verify(password: &SecretString, stored: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(stored).map_err(|_| AuthError::PasswordHash)?;
    match argon2()?.verify_password(password.expose_secret().as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(HashError::PasswordInvalid) => Ok(false),
        Err(_) => Err(AuthError::PasswordHash),
    }
}

/// Spends the same work as [`verify`] for a username that matches no account.
pub(crate) fn verify_unknown_user(password: &SecretString) -> Result<(), AuthError> {
    verify(password, UNKNOWN_USER_HASH).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(text: &str) -> SecretString {
        SecretString::from(text)
    }

    #[test]
    fn policy_enforces_length_and_username() {
        let code = |text: &str| {
            NewPassword::parse(secret(text), "jsmith")
                .err()
                .map(|p| p.code)
        };
        assert_eq!(code("short pass1"), Some("too_short"));
        assert_eq!(code(&"x".repeat(129)), Some("too_long"));
        assert_eq!(code("my-JSmith-password"), Some("contains_username"));
        assert_eq!(code("correct horse battery"), None);
        assert_eq!(
            code(&"é".repeat(12)),
            None,
            "characters, not bytes, are counted"
        );
    }

    #[test]
    fn hash_is_argon2id_with_owasp_parameters_and_verifies() {
        let password = NewPassword::parse(secret("synthetic passphrase"), "jsmith").unwrap();
        let stored = hash(&password).unwrap();
        assert!(
            stored.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "{stored}"
        );
        assert!(verify(&secret("synthetic passphrase"), &stored).unwrap());
        assert!(!verify(&secret("synthetic passphrasE"), &stored).unwrap());
        assert_ne!(
            stored,
            hash(&password).unwrap(),
            "each hash has its own salt"
        );
    }

    #[test]
    fn unknown_user_hash_is_well_formed_and_matches_nothing() {
        assert!(!verify(&secret(""), UNKNOWN_USER_HASH).unwrap());
        assert!(verify_unknown_user(&secret("anything at all")).is_ok());
    }

    #[test]
    fn malformed_stored_hash_is_an_error_not_a_mismatch() {
        assert!(matches!(
            verify(&secret("x"), "not a hash"),
            Err(AuthError::PasswordHash)
        ));
    }
}
