//! SQL for validity policy versions and their categories (REQ-SPC-008).

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, params};

use crate::content::{ValidityCategory, ValidityPolicyContent};
use crate::model::{SpecimenTypeId, ValidityPolicyVersionId};
use crate::repo::lifecycle::{LIFECYCLE_COLUMNS, lifecycle_at};
use crate::views::ValidityPolicyView;

pub(crate) struct NewPolicyVersion {
    pub(crate) id: ValidityPolicyVersionId,
    pub(crate) specimen_type_id: SpecimenTypeId,
    pub(crate) version: i64,
    pub(crate) created_by: UserId,
    pub(crate) created_at: UtcTimestamp,
}

pub(crate) fn insert_version(conn: &Connection, new: &NewPolicyVersion) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_validity_policy_versions (id, specimen_type_id, version, status, \
         revision, created_by, created_at) VALUES (?1, ?2, ?3, 'draft', 1, ?4, ?5)",
    )?
    .execute(params![
        new.id,
        new.specimen_type_id,
        new.version,
        new.created_by,
        new.created_at
    ])?;
    Ok(())
}

/// Replaces a draft's categories. Triggers refuse this for an approved or
/// retired version.
pub(crate) fn replace_categories(
    conn: &Connection,
    id: ValidityPolicyVersionId,
    content: &ValidityPolicyContent,
) -> rusqlite::Result<()> {
    conn.prepare_cached("DELETE FROM cat_validity_categories WHERE policy_version_id = ?1")?
        .execute([id])?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO cat_validity_categories (policy_version_id, code, label, action, \
         qualifier_text) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for category in &content.categories {
        insert.execute(params![
            id,
            category.code,
            category.label,
            category.action,
            category.qualifier_text
        ])?;
    }
    Ok(())
}

/// The version with empty categories (see [`categories`]).
pub(crate) fn version(
    conn: &Connection,
    id: ValidityPolicyVersionId,
) -> rusqlite::Result<Option<ValidityPolicyView>> {
    let sql = format!(
        "SELECT v.id, v.specimen_type_id, s.code, v.version, {LIFECYCLE_COLUMNS} \
         FROM cat_validity_policy_versions v \
         JOIN cat_specimen_types s ON s.id = v.specimen_type_id WHERE v.id = ?1"
    );
    conn.prepare_cached(&sql)?
        .query_row([id], |row| {
            Ok(ValidityPolicyView {
                id: row.get(0)?,
                specimen_type_id: row.get(1)?,
                specimen_type_code: row.get(2)?,
                version: row.get(3)?,
                lifecycle: lifecycle_at(row, 4)?,
                categories: Vec::new(),
            })
        })
        .optional()
}

pub(crate) fn categories(
    conn: &Connection,
    id: ValidityPolicyVersionId,
) -> rusqlite::Result<Vec<ValidityCategory>> {
    conn.prepare_cached(
        "SELECT code, label, action, qualifier_text FROM cat_validity_categories \
         WHERE policy_version_id = ?1 ORDER BY code",
    )?
    .query_map([id], |row| {
        Ok(ValidityCategory {
            code: row.get(0)?,
            label: row.get(1)?,
            action: row.get(2)?,
            qualifier_text: row.get(3)?,
        })
    })?
    .collect()
}

/// The current approved version for the specimen type with `code`.
pub(crate) fn current_for_specimen_type(
    conn: &Connection,
    code: &str,
) -> rusqlite::Result<Option<ValidityPolicyVersionId>> {
    conn.prepare_cached(
        "SELECT v.id FROM cat_validity_policy_versions v \
         JOIN cat_specimen_types s ON s.id = v.specimen_type_id \
         WHERE s.code = ?1 AND v.status = 'approved' AND v.effective_to IS NULL",
    )?
    .query_row([code], |row| row.get(0))
    .optional()
}
