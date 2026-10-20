//! The audit trail viewer (capability `view_audit`; REQ-OPS-001). Every call
//! is a sensitive read and records an `access` event first.

use lis_audit::{AuditDetail, AuditEvent, Finding, MAX_MACHINE_TOKEN_BYTES, Outcome, Stream};
use lis_auth::AuthError;
use lis_core::{CommandError, ErrorCode, FieldError, UtcTimestamp};
use lis_store::StoreError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Failure, ModuleError};
use crate::registry::{LIST_AUDIT_EVENTS, VERIFY_AUDIT_CHAIN};

mod read;
use read::read_page;

/// Largest page of `list_audit_events`.
const MAX_PAGE: u32 = 500;
type FilterOptions = Vec<(String, String)>;

#[derive(Debug, thiserror::Error)]
enum AuditReadError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    #[error(transparent)]
    Reporting(#[from] lis_reporting::ReportingError),
    #[error(transparent)]
    Specimens(#[from] lis_specimens::SpecimenError),
    #[error(transparent)]
    Billing(#[from] lis_billing::BillingError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ModuleError for AuditReadError {
    fn denied(&self) -> Option<lis_core::Denied> {
        match self {
            Self::Store(error) => error.denied(),
            Self::Auth(AuthError::Store(error)) => error.denied(),
            Self::Catalog(lis_catalog::CatalogError::Store(error)) => error.denied(),
            Self::Identity(lis_identity::IdentityError::Store(error)) => error.denied(),
            Self::Reporting(lis_reporting::ReportingError::Store(error)) => error.denied(),
            Self::Specimens(lis_specimens::SpecimenError::Store(error)) => error.denied(),
            Self::Billing(lis_billing::BillingError::Store(error)) => error.denied(),
            Self::Auth(_)
            | Self::Catalog(_)
            | Self::Identity(_)
            | Self::Reporting(_)
            | Self::Specimens(_)
            | Self::Billing(_) => None,
        }
    }
}

impl From<AuditReadError> for Failure {
    fn from(error: AuditReadError) -> Self {
        match error {
            AuditReadError::Store(error)
            | AuditReadError::Auth(AuthError::Store(error))
            | AuditReadError::Catalog(lis_catalog::CatalogError::Store(error))
            | AuditReadError::Identity(lis_identity::IdentityError::Store(error))
            | AuditReadError::Reporting(lis_reporting::ReportingError::Store(error))
            | AuditReadError::Specimens(lis_specimens::SpecimenError::Store(error))
            | AuditReadError::Billing(lis_billing::BillingError::Store(error)) => error.into(),
            AuditReadError::Auth(error) => error.into(),
            other => Failure::Internal(other.to_string()),
        }
    }
}

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
    fn stream(self) -> Stream {
        match self {
            Self::Clinical => Stream::Clinical,
            Self::Security => Stream::Security,
            Self::Access => Stream::Access,
        }
    }
}

/// Input of `list_audit_events`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListAuditEventsInput {
    /// The stream to read.
    pub stream: AuditStream,
    /// Only events before this sequence number (the next page).
    #[ts(optional, type = "number")]
    pub before_seq: Option<i64>,
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

/// One audit event as the viewer shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditEventView {
    /// Its stream.
    pub stream: AuditStream,
    /// Sequence number within the stream.
    #[ts(type = "number")]
    pub seq: i64,
    /// When the action happened.
    pub occurred_at: UtcTimestamp,
    /// When the event was appended.
    pub recorded_at: UtcTimestamp,
    /// Kind of principal.
    #[ts(type = r#""user" | "instrument" | "partner" | "system""#)]
    pub principal_kind: String,
    /// User or instrument ID, or system component name.
    pub principal_id: String,
    /// The user's display name, when the principal is a known user.
    pub principal_display: Option<String>,
    /// `<module>.<verb>`.
    pub action: String,
    /// Plain wording for `action`; the code remains the filter value.
    #[ts(optional)]
    pub action_label: Option<String>,
    /// How it ended.
    #[ts(type = r#""succeeded" | "denied" | "failed""#)]
    pub outcome: String,
    /// Type of the affected record.
    pub record_type: Option<String>,
    /// Plain wording for `record_type`.
    #[ts(optional)]
    pub record_type_label: Option<String>,
    /// ID of the affected record.
    pub record_id: Option<String>,
    /// Human identifier beside `record_id`, when its owner has one.
    #[ts(optional)]
    pub record_label: Option<String>,
    /// Why, as entered.
    pub reason: Option<String>,
    /// Capabilities that permitted the action.
    pub authority: Option<String>,
    /// Finds the diagnostic log line of the same command.
    pub correlation_id: String,
    /// Every action code present in this stream, for the filter control.
    #[ts(optional)]
    #[ts(type = "Array<[string, string]>")]
    pub available_actions: Option<Vec<(String, String)>>,
    /// Every record type present in this stream, for the filter control.
    #[ts(optional)]
    #[ts(type = "Array<[string, string]>")]
    pub available_record_types: Option<Vec<(String, String)>>,
}

/// Input of `verify_audit_chain`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct VerifyAuditChainInput {
    /// The stream to verify.
    pub stream: AuditStream,
}

/// The result of verifying one stream's hash chain (NFR-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainVerification {
    /// The verified stream.
    pub stream: AuditStream,
    /// True when no problem was found.
    pub intact: bool,
    /// Events read.
    #[ts(type = "number")]
    pub events_checked: u64,
    /// Problems, in sequence order.
    pub findings: Vec<ChainFinding>,
}

/// One integrity problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainFinding {
    /// `hash_mismatch`, `broken_link`, `missing_events`, `unknown_algorithm` or
    /// `seal_mismatch`.
    pub kind: String,
    /// The event concerned; for `missing_events`, the first missing one; none for a seal.
    #[ts(type = "number | null")]
    pub seq: Option<i64>,
}

impl From<&Finding> for ChainFinding {
    fn from(finding: &Finding) -> Self {
        let (kind, seq) = match *finding {
            Finding::HashMismatch { seq } => ("hash_mismatch", Some(seq)),
            Finding::BrokenLink { seq } => ("broken_link", Some(seq)),
            Finding::MissingEvents { first, .. } => ("missing_events", Some(first)),
            Finding::UnknownAlgorithm { seq } => ("unknown_algorithm", Some(seq)),
            Finding::SealMismatch { .. } => ("seal_mismatch", None),
        };
        Self {
            kind: kind.to_owned(),
            seq,
        }
    }
}

/// The `access` event for a read of `stream`.
fn access_event(action: &'static str, stream: AuditStream) -> AuditEvent {
    AuditEvent::new(Stream::Access, action, Outcome::Succeeded)
        .record("audit_stream", stream.stream())
}

/// The `access` event of a page read: the stream, filters and count, no content.
fn page_event(
    stream: AuditStream,
    before_seq: Option<i64>,
    returned: usize,
    filter_details: Vec<(&'static str, AuditDetail)>,
) -> AuditEvent {
    let mut event = access_event(LIST_AUDIT_EVENTS.action, stream)
        .detail("returned", AuditDetail::count(returned));
    if let Some(before_seq) = before_seq {
        event = event.detail("before_seq", AuditDetail::integer(before_seq));
    }
    for (key, value) in filter_details {
        event = event.detail(key, value);
    }
    event
}

fn audit_filter_details(
    input: &ListAuditEventsInput,
) -> Result<Vec<(&'static str, AuditDetail)>, Failure> {
    let filters = [
        ("action", "action", input.action.as_deref()),
        ("record_type", "recordType", input.record_type.as_deref()),
        ("record_id", "recordId", input.record_id.as_deref()),
    ];
    let mut details = Vec::new();
    if input.self_approvals_only == Some(true) {
        details.push(("self_approvals_only", AuditDetail::boolean(true)));
    }
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

impl Engine {
    /// `list_audit_events`: one page, newest first.
    pub async fn list_audit_events(
        &self,
        input: ListAuditEventsInput,
    ) -> Result<Vec<AuditEventView>, CommandError> {
        self.run(&LIST_AUDIT_EVENTS, |call| async move {
            if !(1..=MAX_PAGE).contains(&input.limit) {
                return Err(Failure::Refused {
                    code: ErrorCode::Validation,
                    message: "Check the highlighted fields.".to_owned(),
                    field_errors: vec![FieldError {
                        field: "limit".into(),
                        code: "out_of_range".into(),
                        message: "Use a page size from 1 to 500.".into(),
                    }],
                    gate_reasons: Vec::new(),
                });
            }
            let filter_details = audit_filter_details(&input)?;
            let stream = input.stream;
            let before_seq = input.before_seq;
            self.audited_read(
                &call,
                move |conn| read_page(conn, &input),
                move |page| page_event(stream, before_seq, page.len(), filter_details),
            )
            .await
        })
        .await
    }

    /// `verify_audit_chain`: recomputes one stream's chain.
    pub async fn verify_audit_chain(
        &self,
        input: VerifyAuditChainInput,
    ) -> Result<ChainVerification, CommandError> {
        self.run(&VERIFY_AUDIT_CHAIN, |call| async move {
            let stream = input.stream;
            self.audited_read(
                &call,
                move |conn| {
                    let report =
                        lis_audit::verify(conn, stream.stream()).map_err(StoreError::from)?;
                    Ok::<_, StoreError>(ChainVerification {
                        stream,
                        intact: report.is_intact(),
                        events_checked: report.events_checked,
                        findings: report.findings.iter().map(ChainFinding::from).collect(),
                    })
                },
                move |result| {
                    access_event(VERIFY_AUDIT_CHAIN.action, stream)
                        .detail("intact", AuditDetail::boolean(result.intact))
                },
            )
            .await
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_filter_details_reject_free_text() {
        let input = ListAuditEventsInput {
            stream: AuditStream::Security,
            before_seq: None,
            limit: 10,
            action: None,
            record_type: None,
            record_id: Some("Patient Name".to_owned()),
            self_approvals_only: None,
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
