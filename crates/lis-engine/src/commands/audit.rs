//! The audit trail viewer (capability `view_audit`; REQ-OPS-001). Every call
//! is a sensitive read and records an `access` event first.

use lis_audit::{AuditDetail, AuditEvent, Finding, Outcome, Stream};
use lis_auth::AuthError;
use lis_core::{Capability, CommandError, ErrorCode, FieldError, UtcTimestamp};
use lis_store::StoreError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Failure, ModuleError};
use crate::registry::{
    FIND_AUDIT_RECORDS, GET_AUDIT_EVENT_CHANGES, LIST_AUDIT_EVENTS, VERIFY_AUDIT_CHAIN,
};

mod changes;
mod export;
mod export_rows;
mod find;
mod input;
mod labels;
mod read;
pub use changes::{
    AuditChangeStatus, AuditEventChanges, AuditFieldChange, AuditPatient, GetAuditEventChangesInput,
};
use changes::{changes_event, read_changes};
pub use export::{AuditExportFormat, ExportAuditEventsInput, ExportedAuditFile};
pub use find::{AuditDay, FindAuditRecordsInput, FoundAuditRecord, FoundAuditRecords};
use find::{find_details, find_event, find_records};
pub use input::{AuditActorKind, AuditCursor, AuditOutcome, AuditStream, ListAuditEventsInput};
use input::{audit_filter_details, page_event};
use labels::reads_patients;
use read::read_page;

/// Largest page of `list_audit_events`.
const MAX_PAGE: u32 = 500;
type FilterOptions = Vec<(String, String)>;

#[derive(Debug, thiserror::Error)]
enum AuditReadError {
    #[error("audit event not found")]
    EventNotFound,
    #[error("more events match than one export holds")]
    ExportTooLarge(usize),
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
    Orders(#[from] lis_orders::OrderError),
    #[error(transparent)]
    Interfaces(#[from] lis_interfaces::InterfaceError),
    #[error(transparent)]
    Ops(#[from] lis_ops::RuntimeError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl AuditReadError {
    /// The store failure inside, whichever module returned it.
    fn store(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error)
            | Self::Auth(AuthError::Store(error))
            | Self::Catalog(lis_catalog::CatalogError::Store(error))
            | Self::Identity(lis_identity::IdentityError::Store(error))
            | Self::Reporting(lis_reporting::ReportingError::Store(error))
            | Self::Specimens(lis_specimens::SpecimenError::Store(error))
            | Self::Billing(lis_billing::BillingError::Store(error))
            | Self::Orders(lis_orders::OrderError::Store(error))
            | Self::Interfaces(lis_interfaces::InterfaceError::Store(error))
            | Self::Ops(lis_ops::RuntimeError::Store(error)) => Some(error),
            Self::EventNotFound
            | Self::ExportTooLarge(_)
            | Self::Auth(_)
            | Self::Catalog(_)
            | Self::Identity(_)
            | Self::Reporting(_)
            | Self::Specimens(_)
            | Self::Billing(_)
            | Self::Orders(_)
            | Self::Interfaces(_)
            | Self::Ops(_) => None,
        }
    }
}

impl ModuleError for AuditReadError {
    fn denied(&self) -> Option<lis_core::Denied> {
        self.store().and_then(StoreError::denied)
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
            | AuditReadError::Billing(lis_billing::BillingError::Store(error))
            | AuditReadError::Orders(lis_orders::OrderError::Store(error))
            | AuditReadError::Interfaces(lis_interfaces::InterfaceError::Store(error))
            | AuditReadError::Ops(lis_ops::RuntimeError::Store(error)) => error.into(),
            AuditReadError::Auth(error) => error.into(),
            AuditReadError::EventNotFound => {
                Failure::refused(ErrorCode::NotFound, "This audit event does not exist.")
            }
            AuditReadError::ExportTooLarge(limit) => Failure::Refused {
                code: ErrorCode::Validation,
                message: format!(
                    "More than {limit} events match these filters, more than one file of this \
                     format holds. Narrow the filters, for example to fewer days, and export again."
                ),
                field_errors: Vec::new(),
                gate_reasons: Vec::new(),
            },
            other => Failure::Internal(other.to_string()),
        }
    }
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
    /// The display name of a known user, or the current name of a known instrument.
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
    /// Every action code present in the stream or streams read, for the filter control.
    #[ts(optional)]
    #[ts(type = "Array<[string, string]>")]
    pub available_actions: Option<Vec<(String, String)>>,
    /// Every record type present in the stream or streams read, for the filter control.
    #[ts(optional)]
    #[ts(type = "Array<[string, string]>")]
    pub available_record_types: Option<Vec<(String, String)>>,
    /// Every user's ID and name, for the "By" filter control.
    #[ts(optional)]
    #[ts(type = "Array<[string, string]>")]
    pub available_users: Option<Vec<(String, String)>>,
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

/// The `access` event for a read of `stream`, or of every stream (record ID `all`).
fn access_event(action: &'static str, stream: Option<AuditStream>) -> AuditEvent {
    AuditEvent::new(Stream::Access, action, Outcome::Succeeded).record(
        "audit_stream",
        stream.map_or("all", |stream| stream.stream().as_str()),
    )
}

impl Engine {
    /// `list_audit_events`: one page of one stream or of every stream, newest first.
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
            let patients = reads_patients(&call.principal);
            self.audited_read(
                &call,
                move |conn| read_page(conn, &input, patients),
                move |page| page_event(stream, page.len(), filter_details),
            )
            .await
        })
        .await
    }

    /// `get_audit_event_changes`: the Changes section of one event. Patient
    /// values need `view_audit` and the patient record's read capability; the
    /// `access` event naming the patient is written before they are returned.
    pub async fn get_audit_event_changes(
        &self,
        input: GetAuditEventChangesInput,
    ) -> Result<AuditEventChanges, CommandError> {
        self.run(&GET_AUDIT_EVENT_CHANGES, |call| async move {
            let principal = call.principal.clone();
            let viewed = self
                .read(&call, move |conn| read_changes(conn, input, &principal))
                .await?;
            let event = changes_event(GET_AUDIT_EVENT_CHANGES.action, input, &viewed);
            let patient_capability = viewed
                .patient_read
                .as_ref()
                .map(|(_, capability)| *capability);
            self.write(&call, move |ctx| {
                ctx.require(Capability::ViewAudit)?;
                patient_capability
                    .into_iter()
                    .try_for_each(|capability| ctx.require(capability))?;
                ctx.audit(event).map(drop)
            })
            .await?;
            Ok(viewed.changes)
        })
        .await
    }

    /// `find_audit_records`: the records whose readable values start with the typed text,
    /// or whose record ID it is, to filter the trail by one of them. Patients are searched
    /// only for users who may read them; the text is never recorded.
    pub async fn find_audit_records(
        &self,
        input: FindAuditRecordsInput,
    ) -> Result<FoundAuditRecords, CommandError> {
        self.run(&FIND_AUDIT_RECORDS, |call| async move {
            let patients = reads_patients(&call.principal);
            let details = find_details(&input, patients)?;
            let stream = input.stream;
            self.audited_read(
                &call,
                move |conn| find_records(conn, &input, patients),
                move |found| find_event(stream, found, details),
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
                    access_event(VERIFY_AUDIT_CHAIN.action, Some(stream))
                        .detail("intact", AuditDetail::boolean(result.intact))
                },
            )
            .await
        })
        .await
    }
}
