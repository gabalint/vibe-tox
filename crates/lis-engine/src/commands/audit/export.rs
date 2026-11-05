//! `export_audit_events` (REQ-OPS-001): every event matching the viewer's
//! filters, recorded before the export instant, as CSV or PDF in the
//! Downloads folder, with each event's Changes as the viewer shows them.
//!
//! Needs `view_audit` and `export`. Patient values are included only where
//! the exporter may read the patient; otherwise the changed field names are.
//! The file goes through the one export path (intent, digest, no partial
//! file; NFR-07) and the export is recorded on the security stream with the
//! format, counts and every filter, so the same filters reproduce it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Actor, Capability, CommandError, ErrorCode, Principal, UserId, UtcTimestamp};
use lis_report::audit_export_v1 as doc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::changes::{GetAuditEventChangesInput, read_changes};
use super::export_rows::{self, ExportRow, Laboratory, Provenance, lab_time};
use super::input::audit_filter_details;
use super::labels::reads_patients;
use super::read::read_page;
use super::{
    AuditActorKind, AuditCursor, AuditOutcome, AuditReadError, AuditStream, ListAuditEventsInput,
    MAX_PAGE,
};
use crate::engine::Engine;
use crate::pipeline::Failure;
use crate::registry::EXPORT_AUDIT_EVENTS;
use crate::report_rendering::ExportFile;

/// Most events in one CSV export.
pub(super) const MAX_CSV_EVENTS: usize = 50_000;
/// Most events in one PDF export.
pub(super) const MAX_PDF_EVENTS: usize = 2_000;
/// Printed when the laboratory profile is not set yet.
const NO_PROFILE: &str = "Laboratory profile not set";

/// The file format of an audit export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AuditExportFormat {
    /// Comma-separated values, for analysis.
    Csv,
    /// A PDF document, for printing or sending to an auditor.
    Pdf,
}

impl AuditExportFormat {
    fn code(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Pdf => "pdf",
        }
    }

    pub(super) fn max_events(self) -> usize {
        match self {
            Self::Csv => MAX_CSV_EVENTS,
            Self::Pdf => MAX_PDF_EVENTS,
        }
    }
}

/// Input of `export_audit_events`: the viewer's filters and a format. The
/// filter's cursor and page size are ignored; every matching event is exported.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportAuditEventsInput {
    /// CSV or PDF.
    pub format: AuditExportFormat,
    /// The filters, as `list_audit_events` takes them.
    pub filter: ListAuditEventsInput,
}

/// Where the export was written: always the user's Downloads folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedAuditFile {
    /// The file's name.
    pub file_name: String,
    /// Always the Downloads folder.
    #[ts(type = "\"downloads\"")]
    pub folder: &'static str,
    /// Events in the file.
    #[ts(type = "number")]
    pub events: usize,
}

/// What one read gathers for the file.
struct ExportRead {
    rows: Vec<ExportRow>,
    /// Patient-read capabilities used for the values in the file.
    patient_capabilities: BTreeSet<Capability>,
    /// Distinct patients whose values or identification are in the file.
    patients: usize,
    lab: Laboratory,
    exported_by: String,
    by_user_name: Option<String>,
}

impl Engine {
    /// `export_audit_events`: the events matching `input.filter` as a new file in
    /// `folder`, the Downloads folder the desktop shell resolves (`None` when the OS
    /// has none). Refused, writing nothing, when more events match than the format holds.
    pub async fn export_audit_events(
        &self,
        input: ExportAuditEventsInput,
        folder: Option<PathBuf>,
    ) -> Result<ExportedAuditFile, CommandError> {
        self.run(&EXPORT_AUDIT_EVENTS, |call| async move {
            self.authorize(&call).await?;
            if call.principal.require(Capability::Export).is_err() {
                // Records the denial as every refused command does.
                self.write(&call, |ctx| ctx.require(Capability::Export))
                    .await?;
            }
            let folder = folder.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Unavailable,
                    "The Downloads folder is not available on this computer.",
                )
            })?;
            let now = self.clock.now();
            let filter = bounded(input.filter.clone(), now);
            let filter_details = audit_filter_details(&filter)?;
            let format = input.format;
            let principal = call.principal.clone();
            let read_filter = filter.clone();
            let read = self
                .read(&call, move |conn| {
                    read_export(conn, &read_filter, format.max_events(), &principal)
                })
                .await?;
            let provenance = Provenance {
                exported_at: now,
                exported_by: read.exported_by.clone(),
                recorded_before: now,
                filters: filter_words(&input.filter, read.by_user_name.as_deref(), &read.lab)?,
            };
            let bytes = self.export_bytes(format, &read, &provenance).await?;
            let date = lab_time(now, &read.lab.time_zone)?;
            let count = read.rows.len();
            let file = ExportFile {
                folder,
                stem: format!("audit-trail-{}", &date[..10]),
                extension: format.code(),
                sha256: hex::encode(Sha256::digest(&bytes)),
                source: ("rows", AuditDetail::count(count)),
            };
            let (capabilities, patients) = (read.patient_capabilities, read.patients);
            let stream = filter.stream;
            let file_name = self
                .export_file(&call, file, bytes, move |ctx, name| {
                    ctx.require(Capability::ViewAudit)?;
                    ctx.require(Capability::Export)?;
                    capabilities
                        .into_iter()
                        .try_for_each(|capability| ctx.require(capability))?;
                    let event = export_event(name, format, (count, patients), stream);
                    let event = filter_details
                        .into_iter()
                        .fold(event, |event, (key, value)| event.detail(key, value));
                    ctx.audit(event).map(drop)
                })
                .await?;
            Ok(ExportedAuditFile {
                file_name,
                folder: "downloads",
                events: count,
            })
        })
        .await
    }

    /// The file's bytes: CSV text, or the PDF rendered on a blocking thread.
    async fn export_bytes(
        &self,
        format: AuditExportFormat,
        read: &ExportRead,
        provenance: &Provenance,
    ) -> Result<Vec<u8>, Failure> {
        if format == AuditExportFormat::Csv {
            return Ok(export_rows::csv(&read.rows, &read.lab)?.into_bytes());
        }
        let document = export_rows::document(&read.rows, &read.lab, provenance)?;
        let marking = crate::demo::report_marking(self.installation_kind);
        let issued_at: jiff::Timestamp = provenance
            .exported_at
            .to_string()
            .parse()
            .map_err(|_| Failure::Internal("audit export time".to_owned()))?;
        tokio::task::spawn_blocking(move || {
            let json = doc::to_json(&document)
                .map_err(|error| Failure::Internal(format!("audit export data: {error}")))?;
            doc::render_json(&json, issued_at, marking)
                .map(|pdf| pdf.bytes)
                .map_err(|error| Failure::Internal(format!("audit export not rendered: {error}")))
        })
        .await
        .map_err(|_| Failure::Internal("audit export render task stopped".to_owned()))?
    }
}

/// The filter without a cursor, read in full pages, and bounded to the events
/// recorded up to and including `now`.
fn bounded(mut filter: ListAuditEventsInput, now: UtcTimestamp) -> ListAuditEventsInput {
    let after_now = now
        .checked_add(std::time::Duration::from_nanos(1))
        .unwrap_or(now);
    filter.before = None;
    filter.limit = MAX_PAGE;
    filter.recorded_before = Some(
        filter
            .recorded_before
            .map_or(after_now, |end| end.min(after_now)),
    );
    filter
}

/// The security event recording the export, before its filter details.
fn export_event(
    name: &str,
    format: AuditExportFormat,
    (rows, patients): (usize, usize),
    stream: Option<AuditStream>,
) -> AuditEvent {
    AuditEvent::new(
        Stream::Security,
        EXPORT_AUDIT_EVENTS.action,
        Outcome::Succeeded,
    )
    .record("audit_export", name)
    .detail("format", AuditDetail::code(format.code()))
    .detail("rows", AuditDetail::count(rows))
    .detail("patient_records", AuditDetail::count(patients))
    .detail(
        "audit_stream",
        AuditDetail::code(stream.map_or("all", |stream| stream.stream().as_str())),
    )
}

/// Every matching event with its Changes, newest first; refused above `max_events`.
fn read_export(
    conn: &rusqlite::Connection,
    filter: &ListAuditEventsInput,
    max_events: usize,
    principal: &Principal,
) -> Result<ExportRead, AuditReadError> {
    let mut rows = Vec::new();
    let mut patient_capabilities = BTreeSet::new();
    let mut patients = BTreeSet::new();
    let mut page_filter = filter.clone();
    loop {
        let page = read_page(conn, &page_filter, reads_patients(principal))?;
        if rows.len() + page.len() > max_events {
            return Err(AuditReadError::ExportTooLarge(max_events));
        }
        let full = u32::try_from(page.len()).is_ok_and(|len| len == page_filter.limit);
        page_filter.before = page.last().map(|event| AuditCursor {
            recorded_at: event.recorded_at,
            stream: event.stream,
            seq: event.seq,
        });
        for event in page {
            let input = GetAuditEventChangesInput {
                stream: event.stream,
                seq: event.seq,
            };
            let viewed = read_changes(conn, input, principal)?;
            if let Some((subject_id, capability)) = viewed.patient_read {
                patients.insert(subject_id);
                patient_capabilities.insert(capability);
            }
            rows.push(ExportRow {
                event,
                changes: viewed.changes,
            });
        }
        if !full {
            break;
        }
    }
    let (exported_by, by_user_name) = names(conn, principal, filter.by_user)?;
    let lab = lis_identity::current_laboratory_profile(conn)?.map_or_else(
        || Laboratory {
            name: NO_PROFILE.to_owned(),
            time_zone: "UTC".to_owned(),
        },
        |profile| Laboratory {
            name: profile.name,
            time_zone: profile.time_zone,
        },
    );
    Ok(ExportRead {
        rows,
        patient_capabilities,
        patients: patients.len(),
        lab,
        exported_by,
        by_user_name,
    })
}

/// The exporter as `Name (ID)`, and the name of the filter's user, if any.
fn names(
    conn: &rusqlite::Connection,
    principal: &Principal,
    by_user: Option<UserId>,
) -> Result<(String, Option<String>), AuditReadError> {
    let exporter = match principal.actor() {
        Actor::User { user_id, .. } => Some(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => None,
    };
    let ids: BTreeSet<UserId> = exporter.into_iter().chain(by_user).collect();
    let names = lis_auth::display_names(conn, &ids)?;
    let exported_by = exporter.map_or_else(
        || principal.actor().id_text(),
        |id| match names.get(&id) {
            Some(name) => format!("{name} ({id})"),
            None => id.to_string(),
        },
    );
    Ok((exported_by, by_user.and_then(|id| names.get(&id).cloned())))
}

/// The filters the user applied, in words, as the PDF lists them.
fn filter_words(
    filter: &ListAuditEventsInput,
    by_user_name: Option<&str>,
    lab: &Laboratory,
) -> Result<Vec<(String, String)>, Failure> {
    let stream = match filter.stream {
        None => "All streams",
        Some(AuditStream::Clinical) => "Clinical",
        Some(AuditStream::Security) => "Security",
        Some(AuditStream::Access) => "Access",
    };
    let by = match (filter.by_user, filter.by_kind) {
        (Some(id), _) => Some(by_user_name.map_or_else(|| id.to_string(), str::to_owned)),
        (None, Some(AuditActorKind::People)) => Some("People only".to_owned()),
        (None, Some(AuditActorKind::System)) => Some("System only".to_owned()),
        (None, None) => None,
    };
    let outcome = filter.outcome.map(|outcome| match outcome {
        AuditOutcome::Succeeded => "Succeeded",
        AuditOutcome::Denied => "Denied",
        AuditOutcome::Failed => "Failed",
    });
    let time = |at: Option<UtcTimestamp>| at.map(|at| lab_time(at, &lab.time_zone)).transpose();
    let words = [
        ("Stream", Some(stream.to_owned())),
        (
            "Action",
            filter.action.as_deref().map(lis_audit::action_label),
        ),
        (
            "Record type",
            filter
                .record_type
                .as_deref()
                .map(lis_audit::record_type_label),
        ),
        ("Record ID", filter.record_id.clone()),
        ("By", by),
        ("Outcome", outcome.map(str::to_owned)),
        ("Recorded from", time(filter.recorded_from)?),
        ("Recorded before", time(filter.recorded_before)?),
        (
            "Self-approvals only",
            (filter.self_approvals_only == Some(true)).then(|| "Yes".to_owned()),
        ),
    ];
    Ok(words
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name.to_owned(), value)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> UtcTimestamp {
        UtcTimestamp::parse(text).unwrap()
    }

    #[test]
    fn an_export_reads_every_page_up_to_the_export_instant() {
        let now = at("2026-11-05T10:00:00.000000000Z");
        let earlier = at("2026-11-04T10:00:00.000000000Z");
        let cursor = AuditCursor {
            recorded_at: earlier,
            stream: AuditStream::Security,
            seq: 3,
        };
        let filter = bounded(
            ListAuditEventsInput {
                limit: 10,
                before: Some(cursor),
                ..ListAuditEventsInput::default()
            },
            now,
        );
        assert_eq!(filter.limit, MAX_PAGE);
        assert!(filter.before.is_none());
        assert!(filter.recorded_before.is_some_and(|end| end > now));
        let ended = bounded(
            ListAuditEventsInput {
                limit: 10,
                recorded_before: Some(earlier),
                ..ListAuditEventsInput::default()
            },
            now,
        );
        assert!(ended.recorded_before == Some(earlier));
    }

    #[test]
    fn filters_are_listed_in_words() {
        let lab = Laboratory {
            name: "Synthetic Laboratory".to_owned(),
            time_zone: "UTC".to_owned(),
        };
        let filter = ListAuditEventsInput {
            limit: 10,
            action: Some("auth.sign_in".to_owned()),
            by_kind: Some(AuditActorKind::People),
            outcome: Some(AuditOutcome::Denied),
            ..ListAuditEventsInput::default()
        };
        let words = filter_words(&filter, None, &lab).unwrap();
        let pairs: Vec<(&str, &str)> = words
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("Stream", "All streams"),
                ("Action", "Sign-in"),
                ("By", "People only"),
                ("Outcome", "Denied"),
            ]
        );
    }
}
