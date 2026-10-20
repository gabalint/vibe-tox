//! No password, TOTP secret, code or hash reaches the diagnostic log or the
//! audit trail (security-audit-logging.md, "Forbidden fields"). The capture
//! subscriber is global, so it also sees what concurrent tests log.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use lis_auth::{ChangePasswordInput, SignInStep};

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, secret};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Everything logged at any level since the first call.
fn captured() -> &'static Capture {
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).unwrap();
        capture
    })
}

/// Installs the capture once and returns everything logged so far (shared with `m4_canary`).
pub(crate) fn logged() -> String {
    String::from_utf8(captured().0.lock().unwrap().clone()).unwrap()
}

/// True when `code` occurs as a whole run of digits.
fn contains_code(text: &str, code: &str) -> bool {
    text.match_indices(code).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + code.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_digit()) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

#[tokio::test]
async fn secrets_never_reach_logs_or_audit_events() {
    let capture = captured();
    let t = TestEngine::start().await;
    t.bootstrap().await;
    assert!(t.sign_in(ADMIN, "wrong secret passphrase").await.is_err());
    let step = t.sign_in(ADMIN, PASSWORD).await.unwrap();
    let SignInStep::MfaEnrollmentRequired { enrollment, .. } = &step else {
        panic!("{step:?}");
    };
    let totp_secret = enrollment.secret_base32.clone();
    let code = t.code(&totp_secret);
    let wrong_code = if code == "000001" { "000002" } else { "000001" };
    assert!(t.verify(&step, wrong_code).await.is_err());
    t.verify(&step, &code).await.unwrap();
    t.engine
        .change_password(ChangePasswordInput {
            current_password: secret(PASSWORD),
            new_password: secret(NEW_PASSWORD),
        })
        .await
        .unwrap();
    let (hash, sealed): (String, Vec<u8>) = t
        .conn()
        .query_row(
            "SELECT password_hash, totp_secret FROM auth_users",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("command completed"), "the capture works");
    let audit: String = t
        .conn()
        .query_row(
            "SELECT group_concat(concat_ws('|', principal_id, action, record_id, reason, \
             authority, details), char(10)) FROM audit_events",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let sealed_hex: String = sealed.iter().map(|byte| format!("{byte:02x}")).collect();
    for (name, text) in [("log", &logs), ("audit", &audit)] {
        for forbidden in [
            PASSWORD,
            NEW_PASSWORD,
            "wrong secret passphrase",
            &totp_secret,
            &hash,
            &sealed_hex,
        ] {
            assert!(!text.contains(forbidden), "{name} contains a secret");
        }
        assert!(!text.contains("$argon2"), "{name} contains a password hash");
        for digits in [code.as_str(), wrong_code] {
            assert!(!contains_code(text, digits), "{name} contains a TOTP code");
        }
    }
}
