//! Validated account fields: usernames, display names and reasons.

use std::fmt;

use crate::error::Problem;

const USERNAME_LENGTH: std::ops::RangeInclusive<usize> = 3..=64;
const DISPLAY_NAME_MAX: usize = 100;
const REASON_MAX: usize = 500;

/// A username: 3–64 characters of ASCII letters, digits, `.`, `_` and `-`,
/// starting with a letter. Stored lowercase, so it is unique case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Username(String);

impl Username {
    pub(crate) fn parse(text: &str) -> Result<Self, Problem> {
        let name = normalize_username(text);
        let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c);
        if !USERNAME_LENGTH.contains(&name.len())
            || !name.starts_with(|c: char| c.is_ascii_lowercase())
            || !name.chars().all(allowed)
        {
            return Err(Problem {
                code: "invalid",
                message: "Use 3 to 64 letters, digits, dots, dashes or underscores, starting with a letter.",
            });
        }
        Ok(Self(name))
    }

    /// Trusted text read back from the database.
    pub(crate) fn from_stored(name: String) -> Self {
        Self(name)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The key under which a typed username is matched and throttled: trimmed and lowercase.
pub(crate) fn normalize_username(text: &str) -> String {
    text.trim().to_lowercase()
}

/// The person's name as shown in the app: 1–100 characters, no control characters.
pub(crate) fn parse_display_name(text: &str) -> Result<String, Problem> {
    let name = text.trim();
    if name.is_empty()
        || name.chars().count() > DISPLAY_NAME_MAX
        || name.chars().any(char::is_control)
    {
        return Err(Problem {
            code: "invalid",
            message: "Enter a name of up to 100 characters.",
        });
    }
    Ok(name.to_owned())
}

/// Why an administrator made a change: 1–500 characters, stored as entered.
pub(crate) fn parse_reason(text: &str) -> Result<String, Problem> {
    let reason = text.trim();
    if reason.is_empty() {
        return Err(Problem {
            code: "required",
            message: "Enter the reason for this change.",
        });
    }
    if reason.chars().count() > REASON_MAX {
        return Err(Problem {
            code: "too_long",
            message: "Use 500 characters or fewer.",
        });
    }
    Ok(reason.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_lowercased_and_restricted() {
        assert_eq!(
            Username::parse("  J.Smith-2 ").unwrap().as_str(),
            "j.smith-2"
        );
        for bad in [
            "",
            "ab",
            "2fast",
            "with space",
            "émile",
            "a@b.c",
            &"a".repeat(65),
        ] {
            assert!(Username::parse(bad).is_err(), "{bad}");
        }
        assert!(Username::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn display_names_and_reasons_are_bounded() {
        assert_eq!(
            parse_display_name("  Test Analyst ").unwrap(),
            "Test Analyst"
        );
        assert!(parse_display_name(" ").is_err());
        assert!(parse_display_name("Tab\there").is_err());
        assert!(parse_display_name(&"x".repeat(101)).is_err());
        assert_eq!(parse_reason(" Left the lab ").unwrap(), "Left the lab");
        assert_eq!(parse_reason("").unwrap_err().code, "required");
        assert_eq!(parse_reason(&"x".repeat(501)).unwrap_err().code, "too_long");
    }
}
