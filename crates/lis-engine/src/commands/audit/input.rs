//! The input of `list_audit_events` and the filter details its `access` event records.

use lis_audit::{AuditDetail, AuditEvent, EventCursor, MAX_MACHINE_TOKEN_BYTES, Stream};
use lis_core::{ErrorCode, FieldError, UtcTimestamp};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::access_event;
use crate::pipeline::Failure;
use crate::registry::LIST_AUDIT_EVENTS;

/// An audit stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AuditStream {
    /// Clinical state changes and configuration.
    Clinical,
    /// Sign-in, sessions, users and other security events.
    Security,
    /// Sensitive reads.
    Access,
}

impl AuditStream {
    pub(super) fn stream(self) -> Stream {
        match self {
            Self::Clinical => Stream::Clinical,
            Self::Security => Stream::Security,
            Self::Access => Stream::Access,
        }
    }
}

impl From<Stream> for AuditStream {
    fn from(stream: Stream) -> Self {
        match stream {
            Stream::Clinical => Self::Clinical,
            Stream::Security => Self::Security,
            Stream::Access => Self::Access,
        }
    }
}

/// The last event of a page, where the next page starts. Sequence numbers are per stream,
/// so across streams the order is append time, then stream, then sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AuditCursor {
    /// When the event was appended.
    pub recorded_at: UtcTimestamp,
    /// Its stream.
    pub stream: AuditStream,
    /// Its sequence number in the stream.
    #[ts(type = "number")]
    pub seq: i64,
}

impl From<AuditCursor> for EventCursor {
    fn from(cursor: AuditCursor) -> Self {
        Self {
            recorded_at: cursor.recorded_at,
            stream: cursor.stream.stream(),
            seq: cursor.seq,
        }
    }
}

/// Input of `list_audit_events`.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListAuditEventsInput {
    /// The stream to read; every stream when absent.
    #[ts(optional)]
    pub stream: Option<AuditStream>,
    /// Only events after this one, newest first (the next page).
    #[ts(optional)]
    pub before: Option<AuditCursor>,
    /// Page size, 1–500.
    pub limit: u32,
    /// Only this action.
    #[ts(optional)]
    pub action: Option<String>,
    /// Only this record type.
    #[ts(optional)]
    pub record_type: Option<String>,
    /// Only this record ID.
    #[ts(optional)]
    pub record_id: Option<String>,
    /// Only self-approvals: steps done by the person who did the duty's first step,
    /// under the laboratory's separation-of-duties policy (ADR 0015).
    #[serde(default)]
    #[ts(optional)]
    pub self_approvals_only: Option<bool>,
}

/// The `access` event of a page read: the stream, cursor, filters and count, no content.
pub(super) fn page_event(
    stream: Option<AuditStream>,
    returned: usize,
    filter_details: Vec<(&'static str, AuditDetail)>,
) -> AuditEvent {
    let mut event = access_event(LIST_AUDIT_EVENTS.action, stream)
        .detail("returned", AuditDetail::count(returned));
    for (key, value) in filter_details {
        event = event.detail(key, value);
    }
    event
}

/// The cursor and filters as audit details; a filter that is not a machine token is refused.
pub(super) fn audit_filter_details(
    input: &ListAuditEventsInput,
) -> Result<Vec<(&'static str, AuditDetail)>, Failure> {
    let mut details = Vec::new();
    if let Some(before) = input.before {
        details.push(("before_recorded_at", AuditDetail::timestamp(before.recorded_at)));
        details.push(("before_stream", AuditDetail::code(before.stream.stream().as_str())));
        details.push(("before_seq", AuditDetail::integer(before.seq)));
    }
    if input.self_approvals_only == Some(true) {
        details.push(("self_approvals_only", AuditDetail::boolean(true)));
    }
    let filters = [
        ("action", "action", input.action.as_deref()),
        ("record_type", "recordType", input.record_type.as_deref()),
        ("record_id", "recordId", input.record_id.as_deref()),
    ];
    for (detail_key, field, value) in filters {
        if let Some(value) = value {
            let detail = AuditDetail::machine_token(value).map_err(|_| Failure::Refused {
                code: ErrorCode::Validation,
                message: "Check the highlighted fields.".to_owned(),
                field_errors: vec![FieldError {
                    field: field.to_owned(),
                    code: "invalid_machine_token".to_owned(),
                    message: format!(
                        "Use up to {MAX_MACHINE_TOKEN_BYTES} lowercase letters, digits, dots, colons, dashes or underscores."
                    ),
                }],
                gate_reasons: Vec::new(),
            })?;
            details.push((detail_key, detail));
        }
    }
    Ok(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_filter_details_reject_free_text() {
        let input = ListAuditEventsInput {
            limit: 10,
            record_id: Some("Patient Name".to_owned()),
            ..ListAuditEventsInput::default()
        };
        let error = audit_filter_details(&input).unwrap_err();
        let Failure::Refused {
            code, field_errors, ..
        } = error
        else {
            panic!("free-text filter was not a validation refusal");
        };
        assert_eq!(code, ErrorCode::Validation);
        assert_eq!(field_errors[0].field, "recordId");
    }
}
