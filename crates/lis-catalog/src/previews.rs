//! Recorded Catalog Previews for the Starter data screen (REQ-OPS-015,
//! proposed): who generated each report and when, how many drafts it lists,
//! its review, and the blank cutoffs All Data Reviewed must confirm. The
//! engine checks [`catalog_preview_capability`](crate::catalog_preview_capability)
//! before calling these queries.

use lis_core::{UserId, UtcTimestamp};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::model::{AnalyteId, CatalogDraft, CatalogPreviewId, MethodVersionId};
use crate::repo::{methods, review};

/// Columns of this module holding content-addressed artifact digests, which
/// backups copy and restores verify (NFR-07).
pub const ARTIFACT_COLUMNS: &[(&str, &str)] = &[("cat_catalog_previews", "artifact_digest")];

/// One generated Catalog Preview report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogPreviewSummary {
    /// The report's ID, printed on every page.
    pub id: CatalogPreviewId,
    /// When it was recorded.
    pub generated_at: UtcTimestamp,
    /// Who generated it.
    pub generated_by: UserId,
    /// Their display name; the engine fills it in.
    #[ts(optional)]
    pub generated_by_display: Option<String>,
    /// SHA-256 of the report's content, printed on it.
    pub content_digest: String,
    /// Drafts the report lists; All Data Reviewed approves exactly these.
    #[ts(type = "number")]
    pub drafts: i64,
    /// When All Data Reviewed approved it; none while unreviewed.
    pub reviewed_at: Option<UtcTimestamp>,
    /// Who approved it.
    pub reviewed_by: Option<UserId>,
    /// Optional external approval record retained with this review.
    #[ts(optional)]
    pub external_approval_reference: Option<String>,
    /// Their display name; the engine fills it in.
    #[ts(optional)]
    pub reviewed_by_display: Option<String>,
    /// While unreviewed: the blank cutoffs of the listed method drafts, each
    /// of which All Data Reviewed must confirm as not applicable (REQ-ANA-009).
    /// Empty once reviewed.
    pub blank_cutoffs: Vec<CatalogPreviewBlankCutoff>,
}

/// A blank cutoff of a method draft a preview lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CatalogPreviewBlankCutoff {
    /// The method draft; send it back in `confirmedBlankCutoffs`.
    pub method_version_id: MethodVersionId,
    /// The analyte; send it back in `confirmedBlankCutoffs`.
    pub analyte_id: AnalyteId,
    /// The method's code.
    pub method_code: String,
    /// The draft's version number.
    #[ts(type = "number")]
    pub method_version: i64,
    /// The analyte's code.
    pub analyte_code: String,
}

/// Every recorded Catalog Preview, newest first.
pub fn list_catalog_previews(
    conn: &Connection,
) -> Result<Vec<CatalogPreviewSummary>, CatalogError> {
    let mut summaries = Vec::new();
    for row in review::previews(conn)? {
        let blank_cutoffs = if row.reviewed.is_some() {
            Vec::new()
        } else {
            blank_cutoffs(conn, row.id)?
        };
        summaries.push(CatalogPreviewSummary {
            id: row.id,
            generated_at: row.generated_at,
            generated_by: row.generated_by,
            generated_by_display: None,
            content_digest: row.content_digest,
            drafts: row.drafts,
            reviewed_at: row.reviewed.map(|(_, at)| at),
            reviewed_by: row.reviewed.map(|(by, _)| by),
            reviewed_by_display: None,
            external_approval_reference: row.external_approval_reference,
            blank_cutoffs,
        });
    }
    Ok(summaries)
}

/// The digest of a preview's stored PDF, for export.
pub fn catalog_preview_artifact(
    conn: &Connection,
    id: CatalogPreviewId,
) -> Result<String, CatalogError> {
    review::preview_artifact(conn, id)?.ok_or(CatalogError::NotFound("catalog_preview"))
}

/// The blank cutoffs of the method drafts `id` lists, as they are now; a
/// draft changed since the report makes All Data Reviewed refuse anyway.
fn blank_cutoffs(
    conn: &Connection,
    id: CatalogPreviewId,
) -> Result<Vec<CatalogPreviewBlankCutoff>, CatalogError> {
    let mut cutoffs = Vec::new();
    for draft in review::preview_drafts(conn, id)? {
        let CatalogDraft::MethodVersion { id: version, .. } = draft else {
            continue;
        };
        let Some((method_code, method_version)) = review::code_and_version(conn, version)? else {
            continue;
        };
        for (analyte_id, analyte_code) in methods::blank_cutoffs(conn, version)? {
            cutoffs.push(CatalogPreviewBlankCutoff {
                method_version_id: version,
                analyte_id,
                method_code: method_code.clone(),
                method_version,
                analyte_code,
            });
        }
    }
    Ok(cutoffs)
}
