//! Plain display labels for audit action and record codes (REQ-OPS-001).
//! Codes remain the stable filter values; labels are presentation only.

/// A plain label for an audit action code.
#[must_use]
pub fn action_label(code: &str) -> String {
    match code {
        "auth.sign_in" => "Sign-in".to_owned(),
        "auth.mfa_verify" => "Verification code check".to_owned(),
        "auth.mfa_enroll" => "Authenticator enrolled".to_owned(),
        "auth.lockout" => "Account locked after failed attempts".to_owned(),
        "auth.lock" => "Session locked".to_owned(),
        "auth.unlock" => "Session unlocked".to_owned(),
        "auth.reauthenticate" => "Password re-entered to confirm".to_owned(),
        "auth.sign_out" => "Signed out".to_owned(),
        "auth.session_end" => "Session ended".to_owned(),
        "auth.password_change" => "Password changed".to_owned(),
        "auth.first_administrator_create" => "First administrator created".to_owned(),
        "auth.user_create" => "User created".to_owned(),
        "auth.user_roles_set" => "Roles changed".to_owned(),
        "auth.user_disable" => "User disabled".to_owned(),
        "auth.user_enable" => "User enabled".to_owned(),
        "auth.credentials_reset" => "Credentials reset".to_owned(),
        "auth.workstation_register" => "Workstation registered".to_owned(),
        "auth.set_security_policy" => "Security policy changed".to_owned(),
        _ => code_label(code, true),
    }
}

/// A plain label for an audit record-type code.
#[must_use]
pub fn record_type_label(code: &str) -> String {
    code_label(code, false)
}

fn code_label(code: &str, dotted: bool) -> String {
    let text = if dotted {
        code.rsplit_once('.').map_or(code, |(_, action)| action)
    } else {
        code
    };
    let mut label = text.replace('_', " ");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}

#[cfg(test)]
mod tests {
    use super::{action_label, record_type_label};

    #[test]
    fn closed_auth_actions_and_other_codes_are_plain() {
        assert_eq!(action_label("auth.sign_in"), "Sign-in");
        assert_eq!(action_label("report.record_delivery"), "Record delivery");
        assert_eq!(record_type_label("report_revision"), "Report revision");
    }
}
