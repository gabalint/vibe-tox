//! Fresh-authentication passwords arrive as plain JSON strings and never show in `Debug`
//! output (security-audit-logging.md, "Forbidden fields"). The values are synthetic.

use secrecy::ExposeSecret;
use serde::de::DeserializeOwned;

use super::{CloseRecoveryInput, ExportRecoveryKitInput, RequestExitExportInput};

const PASSWORD: &str = "Synthetic-Password-4471";

/// Parses `json` as the desktop shell would and returns the input's `Debug` text.
fn parsed<T: DeserializeOwned + std::fmt::Debug>(json: &serde_json::Value) -> (T, String) {
    let input: T = serde_json::from_value(json.clone()).unwrap();
    let debug = format!("{input:?}");
    (input, debug)
}

#[test]
fn backup_area_passwords_are_secrets() {
    let (kit, kit_debug) = parsed::<ExportRecoveryKitInput>(&serde_json::json!({
        "diskEncryptionReference": "Synthetic envelope",
        "password": PASSWORD,
    }));
    let (exit, exit_debug) = parsed::<RequestExitExportInput>(&serde_json::json!({
        "reason": "Synthetic exit",
        "password": PASSWORD,
    }));
    let (close, close_debug) = parsed::<CloseRecoveryInput>(&serde_json::json!({
        "restoreId": "01900000-0000-7000-8000-0000000000aa",
        "attestedItems": [],
        "note": "Synthetic note",
        "password": PASSWORD,
    }));
    for (secret, debug) in [
        (kit.password, kit_debug),
        (exit.password, exit_debug),
        (close.password, close_debug),
    ] {
        let lis_auth::Reauthentication::Password(password) = secret else {
            panic!("a JSON password must deserialize as password re-authentication");
        };
        assert_eq!(password.expose_secret(), PASSWORD);
        assert!(!debug.contains(PASSWORD), "{debug}");
    }
}
