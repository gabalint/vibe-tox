//! Value parsing errors and the command error contract with the UI.

use serde::Serialize;
use ts_rs::TS;

use crate::id::CorrelationId;

/// Text that is not a valid kernel value. Messages never echo the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// Not a UUIDv7 in canonical lowercase hyphenated form.
    #[error("not a valid identifier")]
    Id,
    /// Not `-?digits(.digits)?`.
    #[error("not a plain decimal number")]
    DecimalSyntax,
    /// Outside the exact range or scale supported for decimals.
    #[error("decimal outside the supported range or precision")]
    DecimalRange,
    /// Not the canonical UTC timestamp form.
    #[error("not a canonical UTC timestamp")]
    Timestamp,
    /// Not one of `<`, `<=`, `>` or `>=`.
    #[error("not a comparator")]
    Comparator,
}

/// The stable error code the UI branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorCode {
    /// Input failed validation; see the field errors.
    Validation,
    /// The record does not exist, or the principal may not know it exists.
    NotFound,
    /// A stale revision or conflicting duplicate.
    Conflict,
    /// The principal lacks the required capability or scope.
    Forbidden,
    /// No one is signed in.
    Unauthenticated,
    /// The session is locked and needs re-authentication.
    SessionLocked,
    /// A safety gate (hold, QC, identity) blocks the action; see the gate reasons.
    GateBlocked,
    /// The engine cannot serve the request now (for example, the writer stopped).
    Unavailable,
    /// An unexpected failure; the correlation ID finds the log line.
    Internal,
}

impl ErrorCode {
    /// The serialized code, for diagnostic log lines.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Forbidden => "forbidden",
            Self::Unauthenticated => "unauthenticated",
            Self::SessionLocked => "session_locked",
            Self::GateBlocked => "gate_blocked",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }
}

/// A validation problem with one input field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FieldError {
    /// Path of the input field, such as `items[2].code`.
    pub field: String,
    /// Stable machine-readable code, such as `required`.
    pub code: String,
    /// Safe, PHI-free text the user can act on.
    pub message: String,
}

/// One gate that blocks the action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GateReason {
    /// Stable machine-readable code, such as `qc_hold`.
    pub code: String,
    /// Safe, PHI-free text the user can act on.
    pub message: String,
}

/// The only error type that crosses into the UI.
///
/// It never carries SQL, stack traces, file paths, internal error text or PHI.
/// Internal failures are logged with the same correlation ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandError {
    code: ErrorCode,
    message: String,
    field_errors: Vec<FieldError>,
    gate_reasons: Vec<GateReason>,
    correlation_id: CorrelationId,
}

impl CommandError {
    /// An error with no field errors or gate reasons.
    pub fn new(code: ErrorCode, message: impl Into<String>, correlation_id: CorrelationId) -> Self {
        Self {
            code,
            message: message.into(),
            field_errors: Vec::new(),
            gate_reasons: Vec::new(),
            correlation_id,
        }
    }

    /// Adds field errors, for `validation`.
    #[must_use]
    pub fn with_field_errors(mut self, field_errors: Vec<FieldError>) -> Self {
        self.field_errors = field_errors;
        self
    }

    /// Adds gate reasons, for `gate_blocked`.
    #[must_use]
    pub fn with_gate_reasons(mut self, gate_reasons: Vec<GateReason>) -> Self {
        self.gate_reasons = gate_reasons;
        self
    }

    /// The error code, for the engine's completion log line.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape_matches_the_ui_contract() {
        let correlation_id = CorrelationId::generate();
        let error = CommandError::new(
            ErrorCode::GateBlocked,
            "Release is blocked.",
            correlation_id,
        )
        .with_field_errors(vec![FieldError {
            field: "comment".into(),
            code: "too_long".into(),
            message: "Use 500 characters or fewer.".into(),
        }])
        .with_gate_reasons(vec![GateReason {
            code: "qc_hold".into(),
            message: "A QC hold is open.".into(),
        }]);
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "gate_blocked",
                "message": "Release is blocked.",
                "fieldErrors": [
                    {"field": "comment", "code": "too_long", "message": "Use 500 characters or fewer."}
                ],
                "gateReasons": [{"code": "qc_hold", "message": "A QC hold is open."}],
                "correlationId": correlation_id.to_string(),
            })
        );
    }

    #[test]
    fn every_code_serializes_to_the_contract_string() {
        let codes = [
            (ErrorCode::Validation, "validation"),
            (ErrorCode::NotFound, "not_found"),
            (ErrorCode::Conflict, "conflict"),
            (ErrorCode::Forbidden, "forbidden"),
            (ErrorCode::Unauthenticated, "unauthenticated"),
            (ErrorCode::SessionLocked, "session_locked"),
            (ErrorCode::GateBlocked, "gate_blocked"),
            (ErrorCode::Unavailable, "unavailable"),
            (ErrorCode::Internal, "internal"),
        ];
        for (code, text) in codes {
            assert_eq!(serde_json::to_value(code).unwrap(), text);
            assert_eq!(code.as_str(), text);
        }
    }

    #[test]
    fn empty_lists_are_still_present() {
        let error = CommandError::new(ErrorCode::Internal, "Try again.", CorrelationId::generate());
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["fieldErrors"], serde_json::json!([]));
        assert_eq!(json["gateReasons"], serde_json::json!([]));
    }

    #[test]
    fn typescript_declaration_matches_the_contract() {
        let cfg = ts_rs::Config::default();
        assert_eq!(
            CommandError::decl(&cfg),
            "type CommandError = { code: ErrorCode, message: string, \
             fieldErrors: Array<FieldError>, gateReasons: Array<GateReason>, \
             correlationId: string, };"
        );
    }
}
