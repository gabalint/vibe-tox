//! PDF export of issued files (brief §4.6, NFR-07, REQ-RPT-003). The engine
//! reads the stored bytes with digest verification and writes the file
//! outside any transaction; the bytes are never regenerated. An export is
//! not a delivery.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::user_id;
use crate::commands::obligations::open_obligation;
use crate::error::{ReportingError, invalid};
use crate::model::{ExportId, ExportPurpose, ObligationId, ObligationKind, ReportRevisionId};
use crate::repo::delivery;
use crate::repo::exports::{self, ExportRow};
use crate::repo::quality::IncidentSubject;
use crate::repo::reports;

/// Input of `export_report_pdf`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ExportReportPdfInput {
    /// The revision.
    pub revision_id: ReportRevisionId,
    /// Why it is exported.
    pub purpose: ExportPurpose,
    /// The obligation it serves; a withdrawal-notice obligation exports the notice.
    #[serde(default)]
    #[ts(optional)]
    pub obligation_id: Option<ObligationId>,
}

/// The stored file an export reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSource {
    /// The revision, or the withdrawn report for a notice.
    pub subject: IncidentSubject,
    /// The digest of the stored bytes.
    pub artifact_sha256: String,
    /// A PHI-free file name stem: the report number and revision.
    pub file_stem: String,
}

/// Which stored file `input` exports: the revision's PDF, or the withdrawal
/// notice when the obligation is a notice obligation.
pub fn export_source(
    conn: &Connection,
    input: &ExportReportPdfInput,
) -> Result<ExportSource, ReportingError> {
    let revision = reports::revision(conn, input.revision_id)?
        .ok_or(ReportingError::NotFound("report revision"))?;
    let report = reports::report(conn, revision.report_id)?
        .ok_or(ReportingError::Inconsistent("revision report"))?;
    let obligation = match input.obligation_id {
        Some(id) => Some(
            delivery::obligation(conn, id)?
                .ok_or(ReportingError::NotFound("delivery obligation"))?,
        ),
        None => None,
    };
    if obligation
        .as_ref()
        .is_some_and(|o| o.revision_id != revision.id)
    {
        return Err(invalid(
            "obligationId",
            "other_revision",
            "The obligation belongs to another revision.",
        ));
    }
    if obligation.is_some_and(|o| o.kind == ObligationKind::WithdrawalNotice) {
        let withdrawal = reports::withdrawal(conn, report.id)?
            .ok_or(ReportingError::Inconsistent("notice without withdrawal"))?;
        return Ok(ExportSource {
            subject: IncidentSubject::Withdrawal(report.id),
            artifact_sha256: withdrawal.notice_artifact_sha256,
            file_stem: format!("{}-withdrawal", report.report_number),
        });
    }
    Ok(ExportSource {
        subject: IncidentSubject::Revision(revision.id),
        artifact_sha256: revision.artifact_sha256,
        file_stem: format!("{}-r{}", report.report_number, revision.revision_no),
    })
}

/// Records an export the engine wrote as `file_name`, on the security
/// stream. An obligation, if given, must still be open.
pub fn record_export(
    ctx: &mut CommandContext<'_>,
    input: &ExportReportPdfInput,
    source: &ExportSource,
    file_name: &str,
) -> Result<ExportId, ReportingError> {
    ctx.require(Capability::DeliverReports)?;
    let exported_by = user_id(ctx)?;
    if let Some(id) = input.obligation_id {
        open_obligation(ctx.tx(), id)?;
    }
    let (revision_id, withdrawal_report_id) = match source.subject {
        IncidentSubject::Revision(id) => (Some(id), None),
        IncidentSubject::Withdrawal(report_id) => (None, Some(report_id)),
        IncidentSubject::Candidate(_) => return Err(ReportingError::Inconsistent("export source")),
    };
    let export = ExportRow {
        id: ExportId::generate(),
        revision_id,
        withdrawal_report_id,
        purpose: input.purpose,
        obligation_id: input.obligation_id,
        artifact_sha256: source.artifact_sha256.clone(),
        file_name: file_name.to_owned(),
        exported_by,
        exported_at: ctx.now(),
    };
    exports::insert_export(ctx.tx(), &export)?;
    let mut event = AuditEvent::new(Stream::Security, actions::EXPORT_PDF, Outcome::Succeeded)
        .record("report_export", export.id)
        .detail("revision_id", AuditDetail::id(input.revision_id))
        .detail("purpose", AuditDetail::code(export.purpose.as_str()))
        .detail(
            "artifact_sha256",
            AuditDetail::sha256(&export.artifact_sha256)?,
        )
        .detail(
            "withdrawal_notice",
            AuditDetail::boolean(withdrawal_report_id.is_some()),
        );
    if let Some(id) = input.obligation_id {
        event = event.detail("obligation_id", AuditDetail::id(id));
    }
    ctx.audit(event)?;
    Ok(export.id)
}
