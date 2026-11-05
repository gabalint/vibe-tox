//! The input of `list_audit_events` and the filter details its `access` event records.

use lis_audit::{AuditDetail, AuditEvent, EventCursor, MAX_MACHINE_TOKEN_BYTES, Outcome, Stream};
use lis_core::{ErrorCode, FieldError, UserId, UtcTimestamp};
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

/// Who acted, by kind, for the "By" filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AuditActorKind {
    /// Signed-in users.
    People,
    /// The system, instruments and partners.
    System,
}

impl AuditActorKind {
    fn code(self) -> &'static str {
        match self {
            Self::People => "people",
            Self::System => "system",
        }
    }
}

/// How the recorded action ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AuditOutcome {
    /// It took effect.
    Succeeded,
    /// Authorization refused it.
    Denied,
    /// It was attempted and failed.
    Failed,
}

impl AuditOutcome {
    pub(super) fn outcome(self) -> Outcome {
        match self {
            Self::Succeeded => Outcome::Succeeded,
            Self::Denied => Outcome::Denied,
            Self::Failed => Outcome::Failed,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Denied => "denied",
            Self::Failed => "failed",
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
    /// Only events recorded at or after this instant: the start of the first day shown.
    #[ts(optional)]
    pub recorded_from: Option<UtcTimestamp>,
    /// Only events recorded before this instant: the end of the last day shown.
    #[ts(optional)]
    pub recorded_before: Option<UtcTimestamp>,
    /// Only events by people, or only by the system.
    #[ts(optional)]
    pub by_kind: Option<AuditActorKind>,
    /// Only events by this user.
    #[ts(optional)]
    pub by_user: Option<UserId>,
    /// Only events with this outcome.
    #[ts(optional)]
    pub outcome: Option<AuditOutcome>,
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
    if let (Some(from), Some(before)) = (input.recorded_from, input.recorded_before)
        && from >= before
    {
        return Err(refused(
            "recordedBefore",
            "out_of_range",
            "Choose an end date on or after the start date.".to_owned(),
        ));
    }
    let before = input.before;
    let mut details: Vec<_> = [
        before.map(|before| {
            (
                "before_recorded_at",
                AuditDetail::timestamp(before.recorded_at),
            )
        }),
        before.map(|before| {
            (
                "before_stream",
                AuditDetail::code(before.stream.stream().as_str()),
            )
        }),
        before.map(|before| ("before_seq", AuditDetail::integer(before.seq))),
        (input.self_approvals_only == Some(true))
            .then(|| ("self_approvals_only", AuditDetail::boolean(true))),
        input
            .recorded_from
            .map(|time| ("recorded_from", AuditDetail::timestamp(time))),
        input
            .recorded_before
            .map(|time| ("recorded_before", AuditDetail::timestamp(time))),
        input
            .by_kind
            .map(|kind| ("by_kind", AuditDetail::code(kind.code()))),
        input.by_user.map(|user| ("by_user", AuditDetail::id(user))),
        input
            .outcome
            .map(|outcome| ("outcome", AuditDetail::code(outcome.code()))),
    ]
    .into_iter()
    .flatten()
    .collect();
    let tokens = [
        ("action", "action", input.action.as_deref()),
        ("record_type", "recordType", input.record_type.as_deref()),
        ("record_id", "recordId", input.record_id.as_deref()),
    ];
    for (detail_key, field, value) in tokens {
        if let Some(value) = value {
            let detail = AuditDetail::machine_token(value).map_err(|_| {
                refused(
                    field,
                    "invalid_machine_token",
                    format!(
                        "Use up to {MAX_MACHINE_TOKEN_BYTES} lowercase letters, digits, dots, colons, dashes or underscores."
                    ),
                )
            })?;
            details.push((detail_key, detail));
        }
    }
    Ok(details)
}

/// A validation refusal naming one input field.
fn refused(field: &str, code: &str, message: String) -> Failure {
    Failure::Refused {
        code: ErrorCode::Validation,
        message: "Check the highlighted fields.".to_owned(),
        field_errors: vec![FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message,
        }],
        gate_reasons: Vec::new(),
    }
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

    #[test]
    fn audit_filter_details_refuse_an_end_before_the_start() {
        let day = UtcTimestamp::parse("2026-10-12T05:00:00.000000000Z").unwrap();
        let input = ListAuditEventsInput {
            limit: 10,
            recorded_from: Some(day),
            recorded_before: Some(day),
            ..ListAuditEventsInput::default()
        };
        let Err(Failure::Refused { field_errors, .. }) = audit_filter_details(&input) else {
            panic!("an empty date range was not refused");
        };
        assert_eq!(field_errors[0].field, "recordedBefore");
    }
}
