//! Checks on the report data before it reaches a template.

use serde_json::Value;

use crate::error::DataError;

/// Parses the data and enforces the contract shared by every template: one
/// JSON object whose scalars are strings or null. Numbers and booleans are
/// refused so a lexical result such as `"5.0"` can never be re-formatted by
/// the renderer (NFR-07, REQ-RPT-008).
pub(crate) fn check(data_json: &str) -> Result<(), DataError> {
    let value: Value = serde_json::from_str(data_json).map_err(|error| DataError::Syntax {
        line: error.line(),
        column: error.column(),
    })?;
    if !value.is_object() {
        return Err(DataError::NotAnObject);
    }
    check_scalars(&value)
}

fn check_scalars(value: &Value) -> Result<(), DataError> {
    match value {
        Value::Null | Value::String(_) => Ok(()),
        Value::Number(_) | Value::Bool(_) => Err(DataError::NonStringScalar),
        Value::Array(items) => items.iter().try_for_each(check_scalars),
        Value::Object(fields) => fields.values().try_for_each(check_scalars),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_strings_nulls_lists_and_objects() {
        assert_eq!(
            check(r#"{"a": "1.0", "b": null, "c": [{"d": "x"}]}"#),
            Ok(())
        );
    }

    #[test]
    fn refuses_numbers_anywhere() {
        assert_eq!(
            check(r#"{"a": [{"b": 5.0}]}"#),
            Err(DataError::NonStringScalar)
        );
    }

    #[test]
    fn refuses_booleans() {
        assert_eq!(check(r#"{"a": true}"#), Err(DataError::NonStringScalar));
    }

    #[test]
    fn refuses_a_top_level_array() {
        assert_eq!(check(r#"["a"]"#), Err(DataError::NotAnObject));
    }

    #[test]
    fn reports_syntax_position_only() {
        let error = check("{\"name\": \"SECRET-VALUE\"").unwrap_err();
        assert!(matches!(error, DataError::Syntax { line: 1, .. }));
        assert!(!error.to_string().contains("SECRET-VALUE"));
    }
}
