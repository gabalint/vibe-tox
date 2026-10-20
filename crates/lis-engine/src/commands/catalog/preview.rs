//! Generating, showing and exporting the Catalog Preview report
//! (REQ-OPS-015, proposed). Generation takes the snapshot on a read
//! connection, renders and stores the PDF outside any transaction, then
//! records the preview, which refuses if any draft changed meanwhile. Showing
//! and export use the stored, verified bytes; they are never regenerated
//! (NFR-07).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_catalog::{
    CatalogError, CatalogPreviewId, CatalogPreviewSnapshot, DemoValues, RecordCatalogPreview,
};
use lis_core::{Actor, CommandError, ErrorCode, UserId, UtcTimestamp};
use lis_report::catalog_preview_v1 as doc;
use lis_reporting::display::lab_time;
use lis_store::{ArtifactDigest, StoreError};
use serde::Deserialize;
use ts_rs::TS;

use super::preview_document::{Header, Inconsistent, document};
use super::standard::{
    EXPORT_CATALOG_PREVIEW_PDF, GENERATE_CATALOG_PREVIEW, VIEW_CATALOG_PREVIEW_PDF, standard_file,
};
use super::versions;
use crate::engine::Engine;
use crate::pipeline::{Call, Failure, access};
use crate::report_rendering::{ExportFile, ExportedReportFile};

/// Printed when the laboratory profile is not set yet.
const NO_PROFILE: &str = "Laboratory profile not set";

/// Input of `view_catalog_preview_pdf` and `export_catalog_preview_pdf`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CatalogPreviewPdfInput {
    /// The recorded preview.
    pub preview_id: CatalogPreviewId,
}

/// The snapshot and what the header needs from the same read.
struct Snapshot {
    snapshot: CatalogPreviewSnapshot,
    generated_by: String,
    drafts_remaining: bool,
}

impl Engine {
    /// `generate_catalog_preview` (`manage_configuration` or
    /// `approve_configuration`): renders, stores and records a Catalog
    /// Preview; returns its ID. Refused with `conflict` when the catalog
    /// changed while the report was generated.
    pub async fn generate_catalog_preview(&self) -> Result<CatalogPreviewId, CommandError> {
        self.run(&GENERATE_CATALOG_PREVIEW, |call| async move {
            self.require_catalog_preview_access(&call).await?;
            let read = self.preview_snapshot(&call).await?;
            let profile = self
                .read(&call, lis_identity::current_laboratory_profile)
                .await?;
            let id = CatalogPreviewId::generate();
            let now = self.clock.now();
            let (laboratory_name, zone) = profile.map_or_else(
                || (NO_PROFILE.to_owned(), "UTC".to_owned()),
                |p| (p.name, p.time_zone),
            );
            let header = Header {
                report: doc::CatalogPreviewHeader {
                    id: id.to_string(),
                    generated_at: lab_time(now, &zone)?,
                    generated_at_utc: lab_time(now, "UTC")?,
                    generated_by: read.generated_by,
                    content_digest: read.snapshot.content_digest.clone(),
                },
                laboratory_name,
                disclaimer: read
                    .drafts_remaining
                    .then(|| read.snapshot.standard.disclaimer.clone()),
            };
            let document = document(&read.snapshot, header)
                .map_err(|Inconsistent(what)| Failure::Internal(format!("preview data: {what}")))?;
            let artifact_digest = self.store_preview(document, now).await?;
            let input = RecordCatalogPreview {
                preview_id: id,
                content_digest: read.snapshot.content_digest,
                artifact_digest,
                drafts: read.snapshot.drafts,
            };
            let recorded = self
                .write(
                    &call,
                    move |ctx| match lis_catalog::record_catalog_preview(ctx, input) {
                        Err(CatalogError::CatalogChanged { .. }) => Ok(None),
                        result => result.map(Some),
                    },
                )
                .await?;
            recorded.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Conflict,
                    "The catalog changed while the report was generated. Generate it again.",
                )
            })
        })
        .await
    }

    /// The snapshot, the generating user's name and whether loaded drafts remain.
    async fn preview_snapshot(&self, call: &Call) -> Result<Snapshot, Failure> {
        let user_id = match call.principal.actor() {
            Actor::User { user_id, .. } => *user_id,
            Actor::Instrument { .. } | Actor::System { .. } => {
                return Err(CatalogError::UserRequired.into());
            }
        };
        let file = standard_file()?;
        let demo_values = self.demo_cutoff_values();
        self.read(call, move |conn| -> Result<_, CatalogError> {
            let snapshot = lis_catalog::catalog_preview_snapshot(conn, &file, &demo_values)?;
            let status = lis_catalog::standard_catalog_status(conn)?;
            let names = versions::display_names(conn, &BTreeSet::from([user_id]))?;
            Ok(Snapshot {
                snapshot,
                generated_by: display_name(&names, user_id),
                drafts_remaining: status.drafts_remaining > 0,
            })
        })
        .await
    }

    /// The demo laboratory's values for the catalog's blank cutoffs, which the
    /// report labels "demo value"; empty in a real laboratory (REQ-OPS-016,
    /// proposed). The demo installation supplies them.
    fn demo_cutoff_values(&self) -> DemoValues {
        crate::demo::demo_cutoff_values(self.installation_kind)
    }

    /// Renders the report and stores it, on a blocking thread; returns the
    /// digest of the stored, synced file. Render errors are PHI-free.
    async fn store_preview(
        &self,
        document: doc::CatalogPreviewDocument,
        issued_at: UtcTimestamp,
    ) -> Result<String, Failure> {
        let db = Arc::clone(&self.db);
        let marking = crate::demo::report_marking(self.installation_kind);
        tokio::task::spawn_blocking(move || {
            let json = doc::to_json(&document)
                .map_err(|error| Failure::Internal(format!("preview data: {error}")))?;
            let issued_at = issued_at
                .to_string()
                .parse()
                .map_err(|_| Failure::Internal("preview time".to_owned()))?;
            let pdf = doc::render_json(&json, issued_at, marking)
                .map_err(|error| Failure::Internal(format!("preview not rendered: {error}")))?;
            let stored = db.artifacts().put(&pdf.bytes).map_err(Failure::from)?;
            Ok(stored.digest.to_string())
        })
        .await
        .map_err(|_| Failure::Internal("preview render task stopped".to_owned()))?
    }

    /// `view_catalog_preview_pdf` (`manage_configuration` or
    /// `approve_configuration`): the stored PDF's verified bytes, which the
    /// desktop shell shows in a window of their own. Audited as a read.
    pub async fn view_catalog_preview_pdf(
        &self,
        input: CatalogPreviewPdfInput,
    ) -> Result<Vec<u8>, CommandError> {
        self.run(&VIEW_CATALOG_PREVIEW_PDF, |call| async move {
            self.require_catalog_preview_access(&call).await?;
            let preview_id = input.preview_id;
            let (digest, bytes) = self.stored_preview(&call, preview_id).await?;
            self.write(&call, move |ctx| {
                ctx.require(lis_catalog::catalog_preview_capability(ctx.principal()))?;
                let event = access(VIEW_CATALOG_PREVIEW_PDF.action)
                    .record("catalog_preview", preview_id)
                    .detail("artifact_sha256", AuditDetail::sha256(&digest)?);
                ctx.audit(event).map(drop)
            })
            .await?;
            Ok(bytes)
        })
        .await
    }

    /// `export_catalog_preview_pdf` (`manage_configuration` or
    /// `approve_configuration`): the stored PDF's verified bytes as a new
    /// file in `folder`, the Downloads folder the desktop shell resolves
    /// (`None` when the OS has none).
    pub async fn export_catalog_preview_pdf(
        &self,
        input: CatalogPreviewPdfInput,
        folder: Option<PathBuf>,
    ) -> Result<ExportedReportFile, CommandError> {
        self.run(&EXPORT_CATALOG_PREVIEW_PDF, |call| async move {
            self.require_catalog_preview_access(&call).await?;
            let folder = folder.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Unavailable,
                    "The Downloads folder is not available on this computer.",
                )
            })?;
            let preview_id = input.preview_id;
            let (digest, bytes) = self.stored_preview(&call, preview_id).await?;
            let file = ExportFile {
                folder,
                stem: format!("catalog-preview-{preview_id}"),
                extension: "pdf",
                sha256: digest.clone(),
                source: ("preview_id", AuditDetail::id(preview_id)),
            };
            let file_name = self
                .export_file(&call, file, bytes, move |ctx, _name| {
                    ctx.require(lis_catalog::catalog_preview_capability(ctx.principal()))?;
                    let event = AuditEvent::new(
                        Stream::Security,
                        EXPORT_CATALOG_PREVIEW_PDF.action,
                        Outcome::Succeeded,
                    )
                    .record("catalog_preview", preview_id)
                    .detail("artifact_sha256", AuditDetail::sha256(&digest)?);
                    ctx.audit(event).map(drop)
                })
                .await?;
            Ok(ExportedReportFile {
                file_name,
                folder: "downloads",
            })
        })
        .await
    }

    /// The recorded preview's artifact digest and its stored bytes, verified
    /// against it.
    async fn stored_preview(
        &self,
        call: &Call,
        preview_id: CatalogPreviewId,
    ) -> Result<(String, Vec<u8>), Failure> {
        let digest = self
            .read(call, move |conn| {
                lis_catalog::catalog_preview_artifact(conn, preview_id)
            })
            .await?;
        let bytes = self.verified_preview(&digest).await?;
        Ok((digest, bytes))
    }

    /// The stored preview bytes, verified against their digest. A missing or
    /// damaged file is refused and logged; a new preview can be generated.
    async fn verified_preview(&self, digest: &str) -> Result<Vec<u8>, Failure> {
        let parsed: ArtifactDigest = digest.parse().map_err(Failure::from)?;
        let db = Arc::clone(&self.db);
        match tokio::task::spawn_blocking(move || db.artifacts().read(&parsed))
            .await
            .map_err(|_| Failure::Internal("preview read task stopped".to_owned()))?
        {
            Ok(bytes) => Ok(bytes),
            Err(error @ (StoreError::ArtifactNotFound | StoreError::ArtifactCorrupt)) => {
                tracing::error!(%error, "stored catalog preview unavailable");
                Err(Failure::refused(
                    ErrorCode::Unavailable,
                    "The stored report file is missing or damaged. Generate a new Catalog Preview.",
                ))
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn display_name(names: &std::collections::HashMap<UserId, String>, user: UserId) -> String {
    names
        .get(&user)
        .cloned()
        .unwrap_or_else(|| user.to_string())
}
