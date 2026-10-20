//! Audit event types: what a command reports, and who and when the store adds.

use std::fmt;

use lis_core::{Capability, CorrelationId, Id, Principal, UtcTimestamp};
use serde_json::{Map, Value};

/// Largest serialized `details` object, in bytes. Details hold references, not content.
pub const MAX_DETAILS_BYTES: usize = 4096;
/// Largest dynamic machine token stored in an audit detail.
pub const MAX_MACHINE_TOKEN_BYTES: usize = 128;

/// One structurally non-PHI value accepted in an audit event's `details`.
///
/// There is intentionally no conversion from `String`, `serde_json::Value`
/// or `Phi<T>`. Dynamic strings must use a validating named constructor.
///
/// ```compile_fail
/// use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
///
/// let text = String::from("unclassified text");
/// let _ = AuditEvent::new(Stream::Clinical, "demo.change", Outcome::Succeeded)
///     .detail("unsafe", text);
/// ```
#[derive(Debug, Clone)]
pub struct AuditDetail(Value);

impl AuditDetail {
    /// A typed LIS identifier, serialized in its canonical UUIDv7 text form.
    #[must_use]
    pub fn id<K>(id: Id<K>) -> Self {
        Self(Value::String(id.to_string()))
    }

    /// A compile-time code, such as an enum or capability code.
    #[must_use]
    pub fn code(value: &'static str) -> Self {
        Self(Value::String(value.to_owned()))
    }

    /// A list of compile-time codes, preserving the existing JSON array shape.
    #[must_use]
    pub fn codes(values: impl IntoIterator<Item = &'static str>) -> Self {
        Self(Value::Array(
            values
                .into_iter()
                .map(|value| Value::String(value.to_owned()))
                .collect(),
        ))
    }

    /// A signed count, version, ordinal or revision.
    #[must_use]
    pub fn integer(value: i64) -> Self {
        Self(Value::from(value))
    }

    /// An unsigned count, version or ordinal.
    #[must_use]
    pub fn unsigned(value: u64) -> Self {
        Self(Value::from(value))
    }

    /// An in-memory collection count.
    #[must_use]
    pub fn count(value: usize) -> Self {
        Self(Value::from(value))
    }

    /// A true/false machine fact.
    #[must_use]
    pub fn boolean(value: bool) -> Self {
        Self(Value::Bool(value))
    }

    /// A validated lowercase hexadecimal SHA-256 digest.
    pub fn sha256(value: &str) -> Result<Self, AuditError> {
        if is_sha256_hex(value) {
            Ok(Self(Value::String(value.to_owned())))
        } else {
            Err(AuditError::InvalidDetailValue)
        }
    }

    /// A bounded dynamic machine identifier.
    ///
    /// Tokens contain only lowercase ASCII letters, digits and `_ . : -`.
    /// This is for identifiers such as IP addresses and audit-viewer action,
    /// record-type or record-ID filters, never human-authored text.
    pub fn machine_token(value: &str) -> Result<Self, AuditError> {
        let valid = !value.is_empty()
            && value.len() <= MAX_MACHINE_TOKEN_BYTES
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'.' | b':' | b'-')
            });
        if valid {
            Ok(Self(Value::String(value.to_owned())))
        } else {
            Err(AuditError::InvalidDetailValue)
        }
    }

    /// A validated workforce sign-in name, never a patient identifier or other PHI.
    ///
    /// Callers pass the auth domain's normalized username. The explicit name
    /// keeps this narrow security-event exception visible at every call site
    /// without duplicating the auth module's validation rule here.
    #[must_use]
    pub fn workforce_username(value: &str) -> Self {
        Self(Value::String(value.to_owned()))
    }
}

/// An independent hash chain with its own sequence (ADR 0008).
///
/// Streams are separate so that each record class can follow its own
/// retention and sealing schedule without breaking another chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stream {
    /// Clinical state changes, release manifests and interface receipts.
    Clinical,
    /// Sign-in, lock, user and grant changes, approvals, backup, restore and export.
    Security,
    /// Sensitive reads: opening patient or report records, printing, viewing the audit trail.
    Access,
}

impl Stream {
    /// Every stream, for verifying all chains.
    pub const ALL: [Self; 3] = [Self::Clinical, Self::Security, Self::Access];

    /// The stored code.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clinical => "clinical",
            Self::Security => "security",
            Self::Access => "access",
        }
    }

    /// The stream with stored code `code`.
    pub(crate) fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stream| stream.as_str() == code)
    }
}

impl fmt::Display for Stream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the audited action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The action took effect.
    Succeeded,
    /// Authorization refused the action.
    Denied,
    /// The action was attempted and failed.
    Failed,
}

impl Outcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

/// What a command reports: the action, its outcome and the affected record.
///
/// Holds record identifiers and non-PHI references only: never names, birth
/// dates, MRNs, result values or free-text comments. A reason typed by the
/// user is stored as entered, because it is part of the record.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub(crate) stream: Stream,
    pub(crate) action: &'static str,
    pub(crate) outcome: Outcome,
    pub(crate) record_type: Option<&'static str>,
    pub(crate) record_id: Option<String>,
    pub(crate) old_revision: Option<i64>,
    pub(crate) new_revision: Option<i64>,
    pub(crate) reason: Option<String>,
    pub(crate) details: Map<String, Value>,
}

impl AuditEvent {
    /// An event for `action`, written `<module>.<verb>` in lowercase, such as `specimen.receive`.
    #[must_use]
    pub fn new(stream: Stream, action: &'static str, outcome: Outcome) -> Self {
        Self {
            stream,
            action,
            outcome,
            record_type: None,
            record_id: None,
            old_revision: None,
            new_revision: None,
            reason: None,
            details: Map::new(),
        }
    }

    /// The affected record, such as `("specimen", specimen_id)`.
    #[must_use]
    pub fn record(mut self, record_type: &'static str, record_id: impl fmt::Display) -> Self {
        self.record_type = Some(record_type);
        self.record_id = Some(record_id.to_string());
        self
    }

    /// The record revision before the change.
    #[must_use]
    pub fn old_revision(mut self, revision: i64) -> Self {
        self.old_revision = Some(revision);
        self
    }

    /// The record revision after the change.
    #[must_use]
    pub fn new_revision(mut self, revision: i64) -> Self {
        self.new_revision = Some(revision);
        self
    }

    /// Why: required for corrections, overrides, cancellations and policy exceptions.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Adds one typed, non-PHI reference while retaining the stored JSON object shape.
    #[must_use]
    pub fn detail(mut self, key: &'static str, value: AuditDetail) -> Self {
        self.details.insert(key.to_owned(), value.0);
        self
    }

    /// Marks a self-approval (ADR 0015): the actor also did the first step of the
    /// duty, under the laboratory's separation-of-duties policy and with their
    /// confirmation. The audit viewer lists these events on request.
    #[must_use]
    pub fn self_approval(self) -> Self {
        self.detail(SELF_APPROVAL_DETAIL, AuditDetail::boolean(true))
    }
}

/// The detail key of [`AuditEvent::self_approval`].
pub(crate) const SELF_APPROVAL_DETAIL: &str = "self_approval";

/// Who acted, when, and under which authority; filled in by the store.
#[derive(Debug, Clone, Copy)]
pub struct Attribution<'a> {
    /// The acting principal.
    pub principal: &'a Principal,
    /// When the action happened (the command's clock time).
    pub occurred_at: UtcTimestamp,
    /// When the event was appended.
    pub recorded_at: UtcTimestamp,
    /// Shared with the diagnostic log line and any returned error.
    pub correlation_id: CorrelationId,
    /// The capabilities that permitted the action; empty when none was checked.
    pub authority: &'a [Capability],
}

/// A position in a stream's chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainLink {
    /// The stream.
    pub stream: Stream,
    /// Sequence number within the stream, from 1; 0 only for the head of an
    /// empty stream (see [`heads`](crate::heads)).
    pub seq: i64,
    /// Lowercase hex SHA-256 of the event at `seq`.
    pub hash: String,
}

/// Failure to append or read audit events. Messages contain no PHI.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// The action is not `<module>.<verb>` in lowercase.
    #[error("audit action must be <module>.<verb> in lowercase")]
    InvalidAction,
    /// A digest or machine token failed its constructor.
    #[error("audit detail value has an invalid machine format")]
    InvalidDetailValue,
    /// Typed details exceed [`MAX_DETAILS_BYTES`].
    #[error("audit details exceed the size limit")]
    InvalidDetails,
    /// An anchor line is not a well-formed `vibe-tox-anchor-1` line.
    #[error("anchor line is not a valid vibe-tox-anchor-1 line")]
    InvalidAnchor,
    /// The database failed.
    #[error("audit database error: {0}")]
    Sql(#[from] rusqlite::Error),
}

/// True for a lowercase hexadecimal SHA-256 digest (64 characters).
pub(crate) fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// `<module>.<verb>`: two non-empty segments of `[a-z0-9_]`, each starting with a letter.
pub(crate) fn is_valid_action(action: &str) -> bool {
    let segment = |part: &str| {
        part.starts_with(|c: char| c.is_ascii_lowercase())
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    action
        .split_once('.')
        .is_some_and(|(module, verb)| segment(module) && segment(verb))
}

/// Serializes the typed details, refusing oversized objects.
pub(crate) fn details_json(details: &Map<String, Value>) -> Result<String, AuditError> {
    let json = serde_json::to_string(details).map_err(|_| AuditError::InvalidDetails)?;
    if json.len() > MAX_DETAILS_BYTES {
        return Err(AuditError::InvalidDetails);
    }
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_must_be_module_dot_verb() {
        for action in [
            "specimen.receive",
            "report.release",
            "auth.sign_in",
            "ifc2.receive",
        ] {
            assert!(is_valid_action(action), "{action}");
        }
        for action in [
            "",
            "specimen",
            "Specimen.receive",
            "specimen.",
            ".receive",
            "specimen.receive.now",
            "specimen receive",
            "2fa.verify",
            "specimen._x",
        ] {
            assert!(!is_valid_action(action), "{action}");
        }
    }

    #[test]
    fn typed_details_keep_the_existing_json_shape() {
        let digest = "ab".repeat(32);
        let ok = AuditEvent::new(Stream::Clinical, "a.b", Outcome::Succeeded)
            .detail("revision", AuditDetail::integer(3))
            .detail("digest", AuditDetail::sha256(&digest).unwrap())
            .detail("codes", AuditDetail::codes(["one", "two"]))
            .detail("username", AuditDetail::workforce_username("analyst1"))
            .detail("active", AuditDetail::boolean(true));
        assert_eq!(
            details_json(&ok.details).unwrap(),
            format!(
                r#"{{"active":true,"codes":["one","two"],"digest":"{digest}","revision":3,"username":"analyst1"}}"#
            )
        );
    }

    #[test]
    fn dynamic_detail_constructors_reject_unclassified_text() {
        assert!(AuditDetail::sha256("ab12").is_err());
        assert!(AuditDetail::sha256(&"A0".repeat(32)).is_err());
        assert!(AuditDetail::machine_token("has space").is_err());
        assert!(AuditDetail::machine_token(&"x".repeat(MAX_MACHINE_TOKEN_BYTES + 1)).is_err());
    }

    #[test]
    fn details_refuse_oversized_objects() {
        let big = AuditEvent::new(Stream::Clinical, "a.b", Outcome::Succeeded)
            .detail("codes", AuditDetail::codes(vec!["code"; 1000]));
        assert!(matches!(
            details_json(&big.details),
            Err(AuditError::InvalidDetails)
        ));
    }
}
