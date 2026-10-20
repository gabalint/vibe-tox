//! Bounded, trimmed free text from user input: one rule for every module.

use crate::FieldError;

/// Trims `value` and refuses it when empty, longer than `max` characters or
/// holding control characters. The error is a PHI-free field error.
pub fn required_text(field: &str, value: &str, max: usize) -> Result<String, FieldError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(field_error(field, "required", "Enter a value."));
    }
    if value.chars().count() > max {
        return Err(field_error(field, "too_long", "The value is too long."));
    }
    if value.chars().any(char::is_control) {
        return Err(field_error(
            field,
            "control_character",
            "Remove control characters.",
        ));
    }
    Ok(value.to_owned())
}

/// Like [`required_text`], but absent or blank text is `None`.
pub fn optional_text(
    field: &str,
    value: Option<&str>,
    max: usize,
) -> Result<Option<String>, FieldError> {
    match value {
        Some(text) if !text.trim().is_empty() => required_text(field, text, max).map(Some),
        Some(_) | None => Ok(None),
    }
}

fn field_error(field: &str, code: &str, message: &str) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_keeps_inner_text() {
        assert_eq!(
            required_text("name", "  Synthetic  Name ", 50).unwrap(),
            "Synthetic  Name"
        );
    }

    #[test]
    fn refuses_blank_long_and_control_text_with_codes() {
        let code = |result: Result<String, FieldError>| result.unwrap_err().code;
        assert_eq!(code(required_text("name", "   ", 5)), "required");
        assert_eq!(code(required_text("name", "abcdef", 5)), "too_long");
        assert_eq!(
            code(required_text("name", "a\u{7}b", 5)),
            "control_character"
        );
        assert_eq!(required_text("name", "ééééé", 5).unwrap(), "ééééé");
    }

    #[test]
    fn optional_text_maps_blank_to_none() {
        assert_eq!(optional_text("note", None, 5), Ok(None));
        assert_eq!(optional_text("note", Some("  "), 5), Ok(None));
        assert_eq!(
            optional_text("note", Some(" x "), 5),
            Ok(Some("x".to_owned()))
        );
    }
}
