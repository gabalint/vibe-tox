//! Report steps that render or move files, which never happen inside a
//! transaction (brief §3.1, §4.2, §4.6; NFR-01, NFR-07):
//!
//! - **Prepare**: T1 stores the candidate (`rendering`); the PDF is rendered
//!   and stored on a blocking thread; T2 records it (`ready`) or the failure.
//! - **Release**: the candidate's stored bytes are read back with digest
//!   verification first; a missing or damaged file records a preservation
//!   incident in its own transaction and refuses the release. Otherwise the
//!   final transaction rechecks everything.
//! - **Withdraw**: fresh authentication and preflight commit first; only then
//!   is the notice rendered, stored, staged durably and withdrawn.
//! - **Export**: verified bytes are published as a new file in the folder
//!   the desktop shell resolves (the OS Downloads folder) after an audited
//!   intent, then the export is recorded ([`export_file`]). Bytes are never
//!   regenerated.

mod export_file;

use std::path::PathBuf;
use std::sync::Arc;

use lis_core::{Capability, CommandError, ErrorCode};
use lis_reporting::{
    CandidateId, CandidateView, ExportReportPdfInput, IncidentSubject, NoticeArtifact, NoticeDraft,
    PrepareReportCandidateInput, PreparedCandidate, PreparedWithdrawal, PreservationIncidentKind,
    ReleaseReportInput, RenderFailure, RenderedArtifact, ReportDetail, ReportingError,
    RevisionSummary, WithdrawReportInput,
};
use lis_store::{ArtifactDigest, CommandContext, StoreError};
use serde::Serialize;
use ts_rs::TS;

use crate::commands::reporting::{
    EXPORT_REPORT_PDF, PREPARE_REPORT_CANDIDATE, RELEASE_REPORT, WITHDRAW_REPORT,
};
use crate::engine::Engine;
use crate::pipeline::{Call, Failure, accepted, access};

pub(crate) use export_file::ExportFile;

/// Where an export was written. The folder is always the user's Downloads
/// folder; the file name is built from the report number (PHI-free).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedReportFile {
    pub file_name: String,
    #[ts(type = "\"downloads\"")]
    pub folder: &'static str,
}

impl Engine {
    /// `prepare_report_candidate`: T1, render and store, T2 (brief §3.1).
    pub async fn prepare_report_candidate(
        &self,
        input: PrepareReportCandidateInput,
    ) -> Result<CandidateView, CommandError> {
        self.run(&PREPARE_REPORT_CANDIDATE, |call| async move {
            let prepared = self
                .write(&call, move |ctx| {
                    lis_reporting::prepare_candidate(ctx, input)
                })
                .await?;
            let candidate_id = prepared.candidate_id;
            let rendered = self.render_candidate(prepared).await;
            self.finish_preparation(&call, candidate_id, rendered).await
        })
        .await
    }

    /// T2: the candidate becomes `ready` and is returned with its access
    /// event, or `failed`, which the caller sees as an internal error.
    async fn finish_preparation(
        &self,
        call: &Call,
        candidate_id: CandidateId,
        rendered: Result<RenderedArtifact, (RenderFailure, String)>,
    ) -> Result<CandidateView, Failure> {
        let (failure, detail) = match rendered {
            Ok(rendered) => {
                return self
                    .write(call, move |ctx| {
                        ready_candidate(ctx, candidate_id, &rendered)
                    })
                    .await;
            }
            Err(failed) => failed,
        };
        self.write(call, move |ctx| {
            lis_reporting::record_render_failure(ctx, candidate_id, failure)
        })
        .await?;
        Err(Failure::Internal(format!(
            "report candidate {candidate_id} not rendered: {detail}"
        )))
    }

    /// Renders and stores a candidate's PDF on a blocking thread. Render
    /// errors are PHI-free by contract (`lis_report::RenderError`).
    async fn render_candidate(
        &self,
        prepared: PreparedCandidate,
    ) -> Result<RenderedArtifact, (RenderFailure, String)> {
        let db = Arc::clone(&self.db);
        let marking = crate::demo::report_marking(self.installation_kind);
        tokio::task::spawn_blocking(move || {
            let pdf = lis_reporting::render::render_report(
                &prepared.template,
                &prepared.data_json,
                prepared.prepared_at,
                marking,
            )
            .map_err(|error| (RenderFailure::Render, error.to_string()))?;
            let artifact = db
                .artifacts()
                .put(&pdf.bytes)
                .map_err(|error| (RenderFailure::Store, error.to_string()))?;
            Ok(RenderedArtifact {
                artifact,
                template_sha256: pdf.template_sha256,
                renderer_version: pdf.renderer_version.to_owned(),
                fonts: pdf.fonts,
            })
        })
        .await
        .map_err(|_| (RenderFailure::Render, "render task stopped".to_owned()))?
    }

    /// `release_report`: verified bytes first, then the final transaction.
    pub async fn release_report(
        &self,
        input: ReleaseReportInput,
    ) -> Result<RevisionSummary, CommandError> {
        self.run(&RELEASE_REPORT, |call| async move {
            self.authorize(&call).await?;
            let candidate_id = input.candidate_id;
            let digest = self
                .report_read(&call, Capability::ReleaseReports, move |conn| {
                    lis_reporting::candidate_artifact(conn, candidate_id)
                })
                .await?;
            let subject = IncidentSubject::Candidate(candidate_id);
            self.read_artifact(&call, subject, &digest, Capability::ReleaseReports)
                .await?;
            let attempt = self
                .write(&call, move |ctx| {
                    ensure_outbound(ctx.tx())?;
                    lis_reporting::release_candidate(ctx, input, &digest)
                })
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// Reads a stored file back and verifies its digest, outside any
    /// transaction. A missing or damaged file is a preservation incident,
    /// recorded in its own transaction (NFR-07).
    async fn read_artifact(
        &self,
        call: &Call,
        subject: IncidentSubject,
        digest: &str,
        capability: Capability,
    ) -> Result<Vec<u8>, Failure> {
        let parsed: ArtifactDigest = digest.parse().map_err(Failure::from)?;
        let db = Arc::clone(&self.db);
        let read = tokio::task::spawn_blocking(move || db.artifacts().read(&parsed))
            .await
            .map_err(|_| Failure::Internal("artifact read task stopped".to_owned()))?;
        let kind = match read {
            Ok(bytes) => return Ok(bytes),
            Err(StoreError::ArtifactNotFound) => PreservationIncidentKind::Missing,
            Err(StoreError::ArtifactCorrupt) => PreservationIncidentKind::Corrupt,
            Err(error) => return Err(error.into()),
        };
        let digest = digest.to_owned();
        self.write(call, move |ctx| {
            lis_reporting::record_preservation_incident(ctx, capability, subject, &digest, kind)
        })
        .await?;
        Err(match subject {
            IncidentSubject::Candidate(_) => ReportingError::ArtifactMissing,
            IncidentSubject::Revision(_) | IncidentSubject::Withdrawal(_) => {
                ReportingError::ArtifactUnavailable
            }
        }
        .into())
    }

    /// `withdraw_report`: authenticate and preflight, then render, durably
    /// stage and withdraw the notice.
    pub async fn withdraw_report(
        &self,
        input: WithdrawReportInput,
    ) -> Result<ReportDetail, CommandError> {
        self.run(&WITHDRAW_REPORT, |call| async move {
            let prepared = self
                .write(&call, move |ctx| {
                    lis_reporting::preflight_withdrawal(ctx, input)
                })
                .await?;
            let prepared = accepted(prepared)?;
            let notice = self.store_notice(prepared.notice.clone()).await?;
            let staged = prepared.clone();
            let stored = notice.clone();
            self.write(&call, move |ctx| {
                lis_reporting::stage_withdrawal_notice(ctx, &staged, &stored)
            })
            .await?;
            self.write(&call, move |ctx| withdraw(ctx, prepared, &notice))
                .await
        })
        .await
    }

    async fn store_notice(&self, draft: NoticeDraft) -> Result<NoticeArtifact, Failure> {
        let db = Arc::clone(&self.db);
        let marking = crate::demo::report_marking(self.installation_kind);
        tokio::task::spawn_blocking(move || {
            let pdf =
                lis_reporting::render::render_notice(&draft.data_json, draft.issued_at, marking)
                    .map_err(|error| Failure::Internal(format!("notice not rendered: {error}")))?;
            let stored = db.artifacts().put(&pdf.bytes).map_err(Failure::from)?;
            Ok(NoticeArtifact {
                artifact_sha256: stored.digest.to_string(),
                template_version: i64::from(pdf.template.version),
                revision_no: draft.revision_no,
            })
        })
        .await
        .map_err(|_| Failure::Internal("notice task stopped".to_owned()))?
    }

    /// `export_report_pdf`: verified bytes to a new file in `folder`, the
    /// Downloads folder the desktop shell resolves (`None` when the OS has
    /// none); never a delivery.
    pub async fn export_report_pdf(
        &self,
        input: ExportReportPdfInput,
        folder: Option<PathBuf>,
    ) -> Result<ExportedReportFile, CommandError> {
        self.run(&EXPORT_REPORT_PDF, |call| async move {
            self.authorize(&call).await?;
            let folder = folder.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Unavailable,
                    "The Downloads folder is not available on this computer.",
                )
            })?;
            let request = input.clone();
            let source = self
                .read(&call, move |conn| {
                    lis_reporting::export_source(conn, &request)
                })
                .await?;
            // §3.8: a delivery export is refused before any file is reserved or written;
            // the recording transaction checks the gate again.
            let purpose = input.purpose;
            self.read(&call, move |conn| ensure_delivery_outbound(conn, purpose))
                .await?;
            let bytes = self
                .read_artifact(
                    &call,
                    source.subject,
                    &source.artifact_sha256,
                    Capability::DeliverReports,
                )
                .await?;
            let file = ExportFile {
                folder,
                stem: source.file_stem.clone(),
                extension: "pdf",
                sha256: source.artifact_sha256.clone(),
                source: source.subject.audit_detail(),
            };
            let file_name = self
                .export_file(&call, file, bytes, move |ctx, name| {
                    ensure_delivery_outbound(ctx.tx(), input.purpose)?;
                    lis_reporting::record_export(ctx, &input, &source, name)
                })
                .await?;
            Ok(ExportedReportFile {
                file_name,
                folder: "downloads",
            })
        })
        .await
    }
}

/// T2 after a successful render, with the access event of the candidate
/// returned to the releaser.
fn ready_candidate(
    ctx: &mut CommandContext<'_>,
    candidate_id: CandidateId,
    rendered: &RenderedArtifact,
) -> Result<CandidateView, ReportingError> {
    let mut view = lis_reporting::record_rendered(ctx, candidate_id, rendered)?;
    if let Some(gate) = recovery_gate(ctx.tx())? {
        view.gates.push(gate);
    }
    ctx.audit(access(PREPARE_REPORT_CANDIDATE.action).record("report_candidate", candidate_id))?;
    Ok(view)
}

pub(crate) fn recovery_gate(
    conn: &rusqlite::Connection,
) -> Result<Option<lis_core::GateReason>, ReportingError> {
    lis_ops::recovery::outbound_gate(conn).map_err(|error| match error {
        lis_ops::backup::BackupError::Store(error) => ReportingError::Store(error),
        _ => ReportingError::Inconsistent("recovery reconciliation"),
    })
}

pub(crate) fn ensure_outbound(conn: &rusqlite::Connection) -> Result<(), ReportingError> {
    match recovery_gate(conn)? {
        Some(gate) => Err(ReportingError::GateBlocked(vec![gate])),
        None => Ok(()),
    }
}

fn ensure_delivery_outbound(
    conn: &rusqlite::Connection,
    purpose: lis_reporting::ExportPurpose,
) -> Result<(), ReportingError> {
    match purpose {
        lis_reporting::ExportPurpose::Delivery => ensure_outbound(conn),
        lis_reporting::ExportPurpose::Records | lis_reporting::ExportPurpose::Internal => Ok(()),
    }
}

/// The withdrawal transaction, with the access event of the report detail
/// returned. Fresh authentication has already committed in preflight.
fn withdraw(
    ctx: &mut CommandContext<'_>,
    prepared: PreparedWithdrawal,
    notice: &NoticeArtifact,
) -> Result<ReportDetail, ReportingError> {
    let report_id = prepared.report_id();
    let detail = lis_reporting::withdraw_report(ctx, prepared, notice)?;
    ctx.audit(access(WITHDRAW_REPORT.action).record("report", report_id))?;
    Ok(detail)
}
