//! SQL for Catalog Preview records and All Data Reviewed (REQ-OPS-015), and
//! the draft lookups they share. Table names come from [`Versioned`]
//! constants, never from input.

use lis_core::{Id, UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::lifecycle::Versioned;
use crate::model::{
    AnalyteId, CatalogDraft, CatalogPreviewId, CatalogReviewId, MethodVersion, MethodVersionId,
    OrderableVersion, ValidityPolicyVersion,
};

/// Every open draft of kind `K` with its revision.
pub(crate) fn drafts<K: Versioned>(conn: &Connection) -> rusqlite::Result<Vec<(Id<K>, i64)>> {
    let sql = format!(
        "SELECT id, revision FROM {} WHERE status = 'draft' ORDER BY id",
        K::TABLE
    );
    conn.prepare_cached(&sql)?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect()
}

/// The parent's code and the version number of version `id`.
pub(crate) fn code_and_version<K: Versioned>(
    conn: &Connection,
    id: Id<K>,
) -> rusqlite::Result<Option<(String, i64)>> {
    let sql = format!(
        "SELECT p.code, v.version FROM {} v JOIN {} p ON p.id = v.{} WHERE v.id = ?1",
        K::TABLE,
        K::PARENT_TABLE,
        K::PARENT_COLUMN
    );
    conn.prepare_cached(&sql)?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
}

pub(crate) struct NewPreview<'a> {
    pub(crate) id: CatalogPreviewId,
    pub(crate) generated_by: UserId,
    pub(crate) generated_at: UtcTimestamp,
    pub(crate) content_digest: &'a str,
    pub(crate) artifact_digest: &'a str,
}

/// Records a preview and the drafts its report lists.
pub(crate) fn insert_preview(
    conn: &Connection,
    preview: &NewPreview<'_>,
    drafts: &[CatalogDraft],
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_catalog_previews (id, generated_by, generated_at, content_digest, \
         artifact_digest) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        preview.id,
        preview.generated_by,
        preview.generated_at,
        preview.content_digest,
        preview.artifact_digest
    ])?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO cat_catalog_preview_drafts (preview_id, record_type, version_id, revision) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for draft in drafts {
        let (record_type, revision) = match *draft {
            CatalogDraft::MethodVersion { revision, .. } => (MethodVersion::RECORD_TYPE, revision),
            CatalogDraft::OrderableVersion { revision, .. } => {
                (OrderableVersion::RECORD_TYPE, revision)
            }
            CatalogDraft::ValidityPolicyVersion { revision, .. } => {
                (ValidityPolicyVersion::RECORD_TYPE, revision)
            }
        };
        insert.execute(params![preview.id, record_type, draft.id_text(), revision])?;
    }
    Ok(())
}

pub(crate) fn preview_exists(conn: &Connection, id: CatalogPreviewId) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM cat_catalog_previews WHERE id = ?1)")?
        .query_row([id], |row| row.get(0))
}

pub(crate) fn is_reviewed(conn: &Connection, id: CatalogPreviewId) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM cat_catalog_reviews WHERE preview_id = ?1)")?
        .query_row([id], |row| row.get(0))
}

/// The drafts a preview lists, in the order recorded.
pub(crate) fn preview_drafts(
    conn: &Connection,
    id: CatalogPreviewId,
) -> rusqlite::Result<Vec<CatalogDraft>> {
    conn.prepare_cached(
        "SELECT record_type, version_id, revision FROM cat_catalog_preview_drafts \
         WHERE preview_id = ?1 ORDER BY rowid",
    )?
    .query_map([id], listed_draft)?
    .collect()
}

fn listed_draft(row: &Row<'_>) -> rusqlite::Result<CatalogDraft> {
    let record_type: String = row.get(0)?;
    let revision = row.get(2)?;
    match record_type.as_str() {
        MethodVersion::RECORD_TYPE => Ok(CatalogDraft::MethodVersion {
            id: row.get(1)?,
            revision,
        }),
        OrderableVersion::RECORD_TYPE => Ok(CatalogDraft::OrderableVersion {
            id: row.get(1)?,
            revision,
        }),
        ValidityPolicyVersion::RECORD_TYPE => Ok(CatalogDraft::ValidityPolicyVersion {
            id: row.get(1)?,
            revision,
        }),
        _ => Err(rusqlite::Error::InvalidColumnType(
            0,
            "record_type".to_owned(),
            rusqlite::types::Type::Text,
        )),
    }
}

pub(crate) struct NewReview {
    pub(crate) id: CatalogReviewId,
    pub(crate) preview_id: CatalogPreviewId,
    pub(crate) reviewed_by: UserId,
    pub(crate) reviewed_at: UtcTimestamp,
}

/// Records a review and each blank cutoff confirmed as not applicable.
pub(crate) fn insert_review(
    conn: &Connection,
    review: &NewReview,
    confirmed: &[(MethodVersionId, AnalyteId)],
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_catalog_reviews (id, preview_id, reviewed_by, reviewed_at) \
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![
        review.id,
        review.preview_id,
        review.reviewed_by,
        review.reviewed_at
    ])?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO cat_catalog_review_confirmations (review_id, method_version_id, analyte_id) \
         VALUES (?1, ?2, ?3)",
    )?;
    for (method_version, analyte) in confirmed {
        insert.execute(params![review.id, method_version, analyte])?;
    }
    Ok(())
}

/// One recorded preview with its draft count and review, if any.
pub(crate) struct PreviewRow {
    pub(crate) id: CatalogPreviewId,
    pub(crate) generated_by: UserId,
    pub(crate) generated_at: UtcTimestamp,
    pub(crate) content_digest: String,
    pub(crate) drafts: i64,
    pub(crate) reviewed: Option<(UserId, UtcTimestamp)>,
    pub(crate) external_approval_reference: Option<String>,
}

/// Every recorded preview, newest first.
pub(crate) fn previews(conn: &Connection) -> rusqlite::Result<Vec<PreviewRow>> {
    conn.prepare_cached(
        "SELECT p.id, p.generated_by, p.generated_at, p.content_digest, \
         (SELECT count(*) FROM cat_catalog_preview_drafts d WHERE d.preview_id = p.id), \
         r.reviewed_by, r.reviewed_at, (SELECT external_reference FROM cat_approval_references a WHERE a.record_kind = 'catalog_review' AND a.record_id = r.id) \
         FROM cat_catalog_previews p LEFT JOIN cat_catalog_reviews r ON r.preview_id = p.id \
         ORDER BY p.generated_at DESC, p.id DESC",
    )?
    .query_map([], |row| {
        let reviewed_by: Option<UserId> = row.get(5)?;
        let reviewed_at: Option<UtcTimestamp> = row.get(6)?;
        Ok(PreviewRow {
            id: row.get(0)?,
            generated_by: row.get(1)?,
            generated_at: row.get(2)?,
            content_digest: row.get(3)?,
            drafts: row.get(4)?,
            reviewed: reviewed_by.zip(reviewed_at),
            external_approval_reference: row.get(7)?,
        })
    })?
    .collect()
}

/// The digest of a preview's stored report, if the preview exists.
pub(crate) fn preview_artifact(
    conn: &Connection,
    id: CatalogPreviewId,
) -> rusqlite::Result<Option<String>> {
    conn.prepare_cached("SELECT artifact_digest FROM cat_catalog_previews WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()
}
