//! SQL for code mapping versions, their mappings and drafts.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use super::error::PartnerError;
use super::mapping_model::{LisRecordId, MappingKind, MappingValues};
use super::model::PartnerConnectionId;

/// A LIS test or analyte with the partner's side, approved or proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredMapping {
    pub(crate) kind: MappingKind,
    pub(crate) lis_id: LisRecordId,
    pub(crate) lis_code: String,
    pub(crate) lis_name: String,
    /// `None` in a draft: remove the mapping.
    pub(crate) values: Option<MappingValues>,
}

/// A draft and who made it.
pub(crate) struct StoredDraft {
    pub(crate) mapping: StoredMapping,
    pub(crate) drafted_by: String,
    pub(crate) drafted_at: UtcTimestamp,
}

/// The approved version in force: its number, who approved it and when.
pub(crate) fn current_version(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<Option<(i64, String, UtcTimestamp)>, PartnerError> {
    conn.prepare_cached(
        "SELECT version, approved_by, approved_at FROM ifc_partner_mapping_versions \
         WHERE connection_id = ?1 ORDER BY version DESC LIMIT 1",
    )?
    .query_row([connection_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .optional()
    .map_err(Into::into)
}

pub(crate) fn mappings(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    version: i64,
) -> Result<Vec<StoredMapping>, PartnerError> {
    conn.prepare_cached(
        "SELECT kind, lis_id, lis_code, lis_name, partner_code, loinc, ucum_unit, sequence_role \
         FROM ifc_partner_mappings WHERE connection_id = ?1 AND version = ?2 \
         ORDER BY kind DESC, lis_name COLLATE NOCASE",
    )?
    .query_map(params![connection_id, version], stored)?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

pub(crate) fn drafts(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<Vec<StoredDraft>, PartnerError> {
    conn.prepare_cached(
        "SELECT kind, lis_id, lis_code, lis_name, partner_code, loinc, ucum_unit, sequence_role, \
         drafted_by, drafted_at FROM ifc_partner_mapping_drafts WHERE connection_id = ?1 \
         ORDER BY kind DESC, lis_name COLLATE NOCASE",
    )?
    .query_map([connection_id], |row| {
        Ok(StoredDraft {
            mapping: stored(row)?,
            drafted_by: row.get(8)?,
            drafted_at: row.get(9)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

fn stored(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredMapping> {
    let partner_code: Option<String> = row.get(4)?;
    Ok(StoredMapping {
        kind: row.get(0)?,
        lis_id: row.get(1)?,
        lis_code: row.get(2)?,
        lis_name: row.get(3)?,
        values: match partner_code {
            Some(partner_code) => Some(MappingValues {
                partner_code,
                loinc: row.get(5)?,
                ucum_unit: row.get(6)?,
                sequence_role: row.get(7)?,
            }),
            None => None,
        },
    })
}

/// Records or replaces the draft of one test or analyte.
pub(crate) fn upsert_draft(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    draft: &StoredMapping,
    by: &str,
    at: UtcTimestamp,
) -> Result<(), PartnerError> {
    let values = draft.values.as_ref();
    conn.execute(
        "INSERT INTO ifc_partner_mapping_drafts (connection_id, kind, lis_id, lis_code, \
         lis_name, partner_code, loinc, ucum_unit, drafted_by, drafted_at, sequence_role) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
         ON CONFLICT (connection_id, kind, lis_id) DO UPDATE SET lis_code = excluded.lis_code, \
         lis_name = excluded.lis_name, partner_code = excluded.partner_code, \
         loinc = excluded.loinc, ucum_unit = excluded.ucum_unit, \
         drafted_by = excluded.drafted_by, drafted_at = excluded.drafted_at, \
         sequence_role = excluded.sequence_role",
        params![
            connection_id,
            draft.kind,
            draft.lis_id,
            draft.lis_code,
            draft.lis_name,
            values.map(|values| &values.partner_code),
            values.and_then(|values| values.loinc.as_ref()),
            values.and_then(|values| values.ucum_unit.as_ref()),
            by,
            at,
            values.and_then(|values| values.sequence_role.as_ref())
        ],
    )?;
    Ok(())
}

/// Removes one draft, or all of a connection's with `None`; returns how many.
pub(crate) fn delete_drafts(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    one: Option<(MappingKind, LisRecordId)>,
) -> Result<usize, PartnerError> {
    let (kind, lis_id) = one.unzip();
    conn.execute(
        "DELETE FROM ifc_partner_mapping_drafts WHERE connection_id = ?1 \
         AND (?2 IS NULL OR (kind = ?2 AND lis_id = ?3))",
        params![connection_id, kind, lis_id],
    )
    .map_err(Into::into)
}

/// A new approved version with all its mappings.
pub(crate) fn insert_version(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    version: i64,
    approval: (&str, &str, UtcTimestamp, bool),
    mappings: &[StoredMapping],
) -> Result<(), PartnerError> {
    let (reason, by, at, self_approval) = approval;
    conn.execute(
        "INSERT INTO ifc_partner_mapping_versions \
         (connection_id, version, reason, approved_by, approved_at, self_approval) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![connection_id, version, reason, by, at, self_approval],
    )?;
    for mapping in mappings {
        let Some(values) = &mapping.values else {
            continue;
        };
        conn.execute(
            "INSERT INTO ifc_partner_mappings (connection_id, version, kind, lis_id, lis_code, \
             lis_name, partner_code, loinc, ucum_unit, sequence_role) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                connection_id,
                version,
                mapping.kind,
                mapping.lis_id,
                mapping.lis_code,
                mapping.lis_name,
                values.partner_code,
                values.loinc,
                values.ucum_unit,
                values.sequence_role
            ],
        )?;
    }
    Ok(())
}
