//! Validation and persistence helpers for billing class mappings.

use std::collections::{BTreeMap, BTreeSet};

use lis_core::{Actor, Principal, UserId, required_text};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{BillingError, invalid};
use crate::model::{
    BillingClassMappingEntry, BillingClassMappingVersionId, BillingClassMappingVersionSummary,
};

const CODE_MAX: usize = 64;
const LABEL_MAX: usize = 200;
const ENTRIES_MAX: usize = 10_000;

pub(crate) fn acting_user(principal: &Principal) -> Result<UserId, BillingError> {
    match principal.actor() {
        Actor::User { user_id, .. } => Ok(*user_id),
        Actor::Instrument { .. } | Actor::System { .. } => Err(BillingError::UserRequired),
    }
}

pub(crate) fn validate_entries(
    conn: &Connection,
    entries: Vec<BillingClassMappingEntry>,
) -> Result<Vec<BillingClassMappingEntry>, BillingError> {
    if entries.is_empty() || entries.len() > ENTRIES_MAX {
        return Err(invalid(lis_core::FieldError {
            field: "entries".to_owned(),
            code: "out_of_range".to_owned(),
            message: "Enter between 1 and 10000 mapping entries.".to_owned(),
        }));
    }
    let known: BTreeSet<_> = lis_catalog::list_analytes(conn)?
        .into_iter()
        .map(|analyte| analyte.id)
        .collect();
    let mut seen = BTreeSet::new();
    let mut labels = BTreeMap::new();
    let mut validated = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        if !known.contains(&entry.analyte_id) {
            return Err(BillingError::AnalyteNotFound);
        }
        if !seen.insert(entry.analyte_id) {
            return Err(invalid(lis_core::FieldError {
                field: format!("entries[{index}].analyteId"),
                code: "duplicate".to_owned(),
                message: "Map each analyte once.".to_owned(),
            }));
        }
        let class_code = required_text(
            &format!("entries[{index}].classCode"),
            &entry.class_code,
            CODE_MAX,
        )
        .map_err(invalid)?;
        let class_label = required_text(
            &format!("entries[{index}].classLabel"),
            &entry.class_label,
            LABEL_MAX,
        )
        .map_err(invalid)?;
        if let Some(existing) = labels.insert(class_code.clone(), class_label.clone())
            && existing != class_label
        {
            return Err(invalid(lis_core::FieldError {
                field: format!("entries[{index}].classLabel"),
                code: "inconsistent".to_owned(),
                message: "Use one label for each class code.".to_owned(),
            }));
        }
        validated.push(BillingClassMappingEntry {
            analyte_id: entry.analyte_id,
            class_code,
            class_label,
        });
    }
    validated.sort_by_key(|entry| entry.analyte_id);
    Ok(validated)
}

pub(crate) fn insert_entries(
    conn: &Connection,
    version_id: BillingClassMappingVersionId,
    entries: &[BillingClassMappingEntry],
) -> Result<(), BillingError> {
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_class_mapping_entries
         (version_id, analyte_id, class_code, class_label) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for entry in entries {
        statement.execute(params![
            version_id,
            entry.analyte_id,
            entry.class_code,
            entry.class_label
        ])?;
    }
    Ok(())
}

pub(crate) const SUMMARY_COLUMNS: &str = "id, version, status, revision, created_by, created_at,
    approved_by, approved_at, confirmed_by, confirmed_at, effective_from, effective_to,
    retired_by, retired_at, retire_reason";

pub(crate) fn summary_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<BillingClassMappingVersionSummary> {
    Ok(BillingClassMappingVersionSummary {
        version_id: row.get(0)?,
        version: row.get(1)?,
        status: row.get(2)?,
        revision: row.get(3)?,
        created_by: row.get(4)?,
        created_by_display: None,
        created_at: row.get(5)?,
        approved_by: row.get(6)?,
        approved_by_display: None,
        approved_at: row.get(7)?,
        confirmed_by: row.get(8)?,
        confirmed_at: row.get(9)?,
        effective_from: row.get(10)?,
        effective_to: row.get(11)?,
        retired_by: row.get(12)?,
        retired_at: row.get(13)?,
        retire_reason: row.get(14)?,
    })
}

pub(crate) fn load_summary(
    conn: &Connection,
    id: BillingClassMappingVersionId,
) -> Result<BillingClassMappingVersionSummary, BillingError> {
    conn.query_row(
        &format!("SELECT {SUMMARY_COLUMNS} FROM bil_class_mapping_versions WHERE id = ?1"),
        [id],
        summary_row,
    )
    .optional()?
    .ok_or(BillingError::NotFound("billing class mapping version"))
}

pub(crate) fn current_mapping(
    conn: &Connection,
) -> Result<Option<BillingClassMappingVersionSummary>, BillingError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {SUMMARY_COLUMNS} FROM bil_class_mapping_versions
                 WHERE status = 'approved' AND effective_from IS NOT NULL
                   AND effective_to IS NULL"
            ),
            [],
            summary_row,
        )
        .optional()?)
}

pub(crate) fn has_draft(conn: &Connection) -> Result<bool, BillingError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM bil_class_mapping_versions WHERE status = 'draft')",
        [],
        |row| row.get(0),
    )?)
}

pub(crate) fn next_version(conn: &Connection) -> Result<i64, BillingError> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM bil_class_mapping_versions",
        [],
        |row| row.get(0),
    )?)
}

pub(crate) fn ensure_revision(
    current: &BillingClassMappingVersionSummary,
    expected: i64,
) -> Result<(), BillingError> {
    if current.revision == expected {
        Ok(())
    } else {
        Err(BillingError::RevisionConflict)
    }
}
