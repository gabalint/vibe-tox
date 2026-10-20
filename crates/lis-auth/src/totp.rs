//! TOTP second factor (NFR-19): RFC 6238 with HMAC-SHA-1, 6 digits, 30-second
//! steps and one step of tolerance either side, through `totp-rs`. Callers
//! refuse a code whose step is not later than the last accepted step, so each
//! code works once (RFC 6238 section 5.2).

use std::fmt;

use lis_core::UtcTimestamp;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;
use totp_rs::{Algorithm, Builder, Secret, Totp};
use ts_rs::TS;
use zeroize::{Zeroize, Zeroizing};

use crate::account::Username;
use crate::cipher::random_bytes;
use crate::error::AuthError;

const DIGITS: u8 = 6;
const STEP_SECONDS: u64 = 30;
const SKEW_STEPS: u16 = 1;
/// 160 bits, the RFC 4226 recommendation.
const SECRET_BYTES: usize = 20;
/// Issuer shown in authenticator apps, URI-encoded. The product name is a
/// placeholder until the publisher is chosen (Q-19).
const ISSUER: &str = "Vibe%20Tox%20LIS";

/// A TOTP shared secret, held in memory only while it is needed.
#[derive(Clone)]
pub(crate) struct TotpSecret(Zeroizing<Vec<u8>>);

impl TotpSecret {
    /// A new random secret.
    pub(crate) fn generate() -> Result<Self, AuthError> {
        let bytes = Zeroizing::new(random_bytes::<SECRET_BYTES>()?);
        Ok(Self(Zeroizing::new(bytes.to_vec())))
    }

    /// A secret opened from storage.
    pub(crate) fn from_bytes(bytes: Zeroizing<Vec<u8>>) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    fn totp(&self) -> Result<Totp, AuthError> {
        Builder::new()
            .with_algorithm(Algorithm::SHA1)
            .with_digits(DIGITS)
            .with_skew(SKEW_STEPS)
            .with_step_duration(STEP_SECONDS)
            .with_secret(self.as_bytes())
            .build()
            .map_err(|_| AuthError::Totp)
    }

    /// The time step `code` belongs to at `now`, if it is valid within the
    /// tolerance. Spaces in the code are ignored.
    pub(crate) fn matching_step(
        &self,
        code: &str,
        now: UtcTimestamp,
    ) -> Result<Option<i64>, AuthError> {
        let Ok(time) = u64::try_from(now.unix_seconds()) else {
            return Ok(None);
        };
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let step = self.totp()?.check(&code, time);
        Ok(step.and_then(|step| i64::try_from(step).ok()))
    }

    /// What an authenticator app needs to enrol this secret for `username`.
    pub(crate) fn enrollment(&self, username: &Username) -> Result<TotpEnrollment, AuthError> {
        let secret_base32 = Secret::from(self.as_bytes()).to_base32();
        let otpauth_uri = format!(
            "otpauth://totp/{ISSUER}:{username}?secret={secret_base32}&issuer={ISSUER}\
             &algorithm=SHA1&digits={DIGITS}&period={STEP_SECONDS}"
        );
        let qr_svg = QrCode::new(otpauth_uri.as_bytes())
            .map_err(|_| AuthError::QrCode)?
            .render::<svg::Color<'_>>()
            .min_dimensions(200, 200)
            .build();
        Ok(TotpEnrollment {
            secret_base32,
            otpauth_uri,
            qr_svg,
        })
    }
}

impl fmt::Debug for TotpSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TotpSecret([redacted])")
    }
}

/// An authenticator enrolment shown once to the person enrolling. It carries
/// the secret: `Debug` redacts it and dropping it wipes the text.
#[derive(Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TotpEnrollment {
    /// The secret in base32, for manual entry.
    pub secret_base32: String,
    /// The `otpauth://` URI the QR code encodes.
    pub otpauth_uri: String,
    /// The QR code as SVG text.
    pub qr_svg: String,
}

impl fmt::Debug for TotpEnrollment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TotpEnrollment([redacted])")
    }
}

impl Drop for TotpEnrollment {
    fn drop(&mut self) {
        self.secret_base32.zeroize();
        self.otpauth_uri.zeroize();
        self.qr_svg.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RFC 6238 Appendix B SHA-1 seed.
    fn rfc_secret() -> TotpSecret {
        TotpSecret::from_bytes(Zeroizing::new(b"12345678901234567890".to_vec()))
    }

    fn at(text: &str) -> UtcTimestamp {
        UtcTimestamp::parse(text).unwrap()
    }

    // Expected codes: the last six digits of the RFC 6238 Appendix B values.
    #[test]
    fn accepts_rfc_6238_test_vectors() {
        let secret = rfc_secret();
        let cases = [
            ("1970-01-01T00:00:59.000000000Z", "287082", 1),
            ("2005-03-18T01:58:29.000000000Z", "081804", 37_037_036),
            ("2009-02-13T23:31:30.000000000Z", "005924", 41_152_263),
        ];
        for (time, code, step) in cases {
            assert_eq!(
                secret.matching_step(code, at(time)).unwrap(),
                Some(step),
                "{time}"
            );
        }
    }

    #[test]
    fn tolerates_one_step_either_side_and_no_more() {
        let secret = rfc_secret();
        assert_eq!(
            secret
                .matching_step("287 082", at("1970-01-01T00:01:29.000000000Z"))
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            secret
                .matching_step("287082", at("1970-01-01T00:00:00.000000000Z"))
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            secret
                .matching_step("287082", at("1970-01-01T00:01:30.000000000Z"))
                .unwrap(),
            None
        );
        assert_eq!(
            secret
                .matching_step("287083", at("1970-01-01T00:00:59.000000000Z"))
                .unwrap(),
            None
        );
        assert_eq!(
            secret
                .matching_step("+87082", at("1970-01-01T00:00:59.000000000Z"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn enrollment_encodes_the_secret_for_authenticator_apps() {
        let username = Username::parse("jsmith").unwrap();
        let enrollment = rfc_secret().enrollment(&username).unwrap();
        assert_eq!(enrollment.secret_base32, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(
            enrollment.otpauth_uri,
            "otpauth://totp/Vibe%20Tox%20LIS:jsmith?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\
             &issuer=Vibe%20Tox%20LIS&algorithm=SHA1&digits=6&period=30"
        );
        assert!(enrollment.qr_svg.contains("<svg"));
        assert_eq!(format!("{enrollment:?}"), "TotpEnrollment([redacted])");
    }

    #[test]
    fn generated_secrets_are_random_and_160_bits() {
        let first = TotpSecret::generate().unwrap();
        assert_eq!(first.as_bytes().len(), 20);
        assert_ne!(first.as_bytes(), TotpSecret::generate().unwrap().as_bytes());
        assert_eq!(format!("{first:?}"), "TotpSecret([redacted])");
    }
}
