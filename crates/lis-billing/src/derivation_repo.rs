//! SQL for `bil_derivations` and its append-only facts. A group is one
//! subject and date of service (`date_of_service IS ?`, so the unknown date
//! is a group too).

use lis_core::{UserId, UtcTimestamp};
use lis_identity::SubjectId;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::derivation::DerivationOutcome;
use crate::error::BillingError;
use crate::model::{
    BillingClass, BillingClassMappingVersionId, BillingDerivationId, BillingDerivationState,
    BillingFlag, ClassFact, DerivationCompleteness, DerivedComponent, DerivedSpecimen,
};

/// One stored derivation.
#[derive(Debug, Clone)]
pub(crate) struct DerivationRow {
    pub(crate) id: BillingDerivationId,
    pub(crate) subject_id: SubjectId,
    pub(crate) date_of_service: Option<String>,
    pub(crate) dos_rule: String,
    pub(crate) derivation_no: i64,
    pub(crate) mapping_version_id: Option<BillingClassMappingVersionId>,
    pub(crate) inputs_sha256: String,
    pub(crate) completeness: DerivationCompleteness,
    pub(crate) state: BillingDerivationState,
    pub(crate) adjustment_of_id: Option<BillingDerivationId>,
    pub(crate) derived_at: UtcTimestamp,
    pub(crate) approved_at: Option<UtcTimestamp>,
    pub(crate) facts_sealed: bool,
    pub(crate) revision: i64,
}

const COLUMNS: &str = "id, subject_id, date_of_service, dos_rule, derivation_no, \
    mapping_version_id, inputs_sha256, completeness, state, adjustment_of_id, derived_at, \
    approved_at, facts_sealed, revision";

fn row(row: &Row<'_>) -> rusqlite::Result<DerivationRow> {
    Ok(DerivationRow {
        id: row.get(0)?,
        subject_id: row.get(1)?,
        date_of_service: row.get(2)?,
        dos_rule: row.get(3)?,
        derivation_no: row.get(4)?,
        mapping_version_id: row.get(5)?,
        inputs_sha256: row.get(6)?,
        completeness: row.get(7)?,
        state: row.get(8)?,
        adjustment_of_id: row.get(9)?,
        derived_at: row.get(10)?,
        approved_at: row.get(11)?,
        facts_sealed: row.get(12)?,
        revision: row.get(13)?,
    })
}

pub(crate) fn load(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<DerivationRow, BillingError> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM bil_derivations WHERE id = ?1"
    ))?
    .query_row([id], row)
    .optional()?
    .ok_or(BillingError::NotFound("billing derivation"))
}

/// The newest derivation of the group.
pub(crate) fn latest(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Option<DerivationRow>, BillingError> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM bil_derivations \
             WHERE subject_id = ?1 AND date_of_service IS ?2 \
             ORDER BY derivation_no DESC LIMIT 1"
        ))?
        .query_row(params![subject_id, date_of_service], row)
        .optional()?)
}

/// The newest derivation of the group that was actually handed off: a new
/// derivation adjusts it even when an intervening approval was superseded.
pub(crate) fn latest_handed_off(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Option<BillingDerivationId>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT id FROM bil_derivations \
             WHERE subject_id = ?1 AND date_of_service IS ?2 AND state = 'handed_off' \
             ORDER BY derivation_no DESC LIMIT 1",
        )?
        .query_row(params![subject_id, date_of_service], |row| row.get(0))
        .optional()?)
}

/// The newest derivation of every subject/date group.
pub(crate) fn latest_by_group(conn: &Connection) -> Result<Vec<DerivationRow>, BillingError> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {COLUMNS} FROM bil_derivations d \
             WHERE derivation_no = (SELECT max(newer.derivation_no) FROM bil_derivations newer \
                 WHERE newer.subject_id = d.subject_id \
                   AND newer.date_of_service IS d.date_of_service) \
             ORDER BY derived_at, id"
        ))?
        .query_map([], row)?
        .collect::<Result<_, _>>()?)
}

/// Derivations of the group not handed off or superseded, with their revision.
pub(crate) fn open_in_group(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Vec<(BillingDerivationId, i64)>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT id, revision FROM bil_derivations \
             WHERE subject_id = ?1 AND date_of_service IS ?2 \
               AND state IN ('held', 'candidate', 'approved') ORDER BY derivation_no",
        )?
        .query_map(params![subject_id, date_of_service], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?)
}

/// Who derived a new derivation, and which approved one it adjusts.
pub(crate) struct NewDerivation {
    pub(crate) id: BillingDerivationId,
    pub(crate) derivation_no: i64,
    pub(crate) adjustment_of_id: Option<BillingDerivationId>,
    pub(crate) derived_by: UserId,
    pub(crate) derived_at: UtcTimestamp,
}

/// Inserts the derivation row and every fact of `outcome`.
pub(crate) fn insert(
    conn: &Connection,
    new: &NewDerivation,
    outcome: &DerivationOutcome,
) -> Result<(), BillingError> {
    let facts = &outcome.facts;
    conn.prepare_cached(
        "INSERT INTO bil_derivations (id, subject_id, date_of_service, dos_rule, derivation_no, \
             mapping_version_id, inputs_sha256, completeness, state, adjustment_of_id, \
             derived_by, derived_at, facts_sealed, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 1)",
    )?
    .execute(params![
        new.id,
        facts.subject_id,
        facts.date_of_service,
        facts.dos_rule,
        new.derivation_no,
        facts.mapping_version_id,
        outcome.inputs_sha256,
        facts.completeness,
        facts.state,
        new.adjustment_of_id,
        new.derived_by,
        new.derived_at,
    ])?;
    insert_facts(conn, new.id, outcome)?;
    conn.prepare_cached("UPDATE bil_derivations SET facts_sealed = 1 WHERE id = ?1")?
        .execute([new.id])?;
    Ok(())
}

fn insert_facts(
    conn: &Connection,
    id: BillingDerivationId,
    outcome: &DerivationOutcome,
) -> Result<(), BillingError> {
    let facts = &outcome.facts;
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_derivation_specimens \
         (derivation_id, specimen_id, order_id, order_version, report_revision_id) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for s in &facts.specimens {
        statement.execute(params![
            id,
            s.specimen_id,
            s.order_id,
            s.order_version,
            s.report_revision_id
        ])?;
    }
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_derivation_components \
         (derivation_id, order_component_id, specimen_id, disposition) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for c in &facts.components {
        statement.execute(params![id, c.component_id, c.specimen_id, c.disposition])?;
    }
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_derivation_classes (derivation_id, specimen_id, class_code, \
             class_label, certainty, ordered, performed, reported) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?;
    for c in &facts.classes {
        statement.execute(params![
            id,
            c.specimen_id,
            c.class.code,
            c.class.label,
            c.certainty,
            c.ordered,
            c.performed,
            c.reported
        ])?;
    }
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_derivation_flags (derivation_id, code, ref_id, blocking) \
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for f in &facts.flags {
        statement.execute(params![id, f.code, f.ref_id, f.blocking])?;
    }
    let mut statement = conn.prepare_cached(
        "INSERT INTO bil_derivation_inputs (derivation_id, kind, ref_id) VALUES (?1, ?2, ?3)",
    )?;
    for input in &outcome.input_refs {
        statement.execute(params![id, input.kind, input.ref_id])?;
    }
    Ok(())
}

/// Moves a derivation to `state` at `revision`.
pub(crate) fn set_state(
    conn: &Connection,
    id: BillingDerivationId,
    state: BillingDerivationState,
    revision: i64,
) -> Result<(), BillingError> {
    conn.prepare_cached("UPDATE bil_derivations SET state = ?2, revision = ?3 WHERE id = ?1")?
        .execute(params![id, state, revision])?;
    Ok(())
}

/// Records approval by `by` at `at`.
pub(crate) fn approve(
    conn: &Connection,
    id: BillingDerivationId,
    revision: i64,
    by: UserId,
    at: UtcTimestamp,
) -> Result<(), BillingError> {
    conn.prepare_cached(
        "UPDATE bil_derivations SET state = 'approved', revision = ?2, approved_by = ?3,              approved_at = ?4 WHERE id = ?1",
    )?
    .execute(params![id, revision, by, at])?;
    Ok(())
}

pub(crate) fn specimens(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<DerivedSpecimen>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT specimen_id, order_id, order_version, report_revision_id \
             FROM bil_derivation_specimens WHERE derivation_id = ?1 ORDER BY specimen_id",
        )?
        .query_map([id], |row| {
            Ok(DerivedSpecimen {
                specimen_id: row.get(0)?,
                order_id: row.get(1)?,
                order_version: row.get(2)?,
                report_revision_id: row.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

pub(crate) fn components(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<DerivedComponent>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT order_component_id, specimen_id, disposition FROM bil_derivation_components \
             WHERE derivation_id = ?1 ORDER BY specimen_id, order_component_id",
        )?
        .query_map([id], |row| {
            Ok(DerivedComponent {
                component_id: row.get(0)?,
                specimen_id: row.get(1)?,
                disposition: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

pub(crate) fn classes(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<ClassFact>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT specimen_id, class_code, class_label, certainty, ordered, performed, reported \
             FROM bil_derivation_classes WHERE derivation_id = ?1 \
             ORDER BY specimen_id, certainty, class_code",
        )?
        .query_map([id], |row| {
            Ok(ClassFact {
                specimen_id: row.get(0)?,
                class: BillingClass {
                    code: row.get(1)?,
                    label: row.get(2)?,
                },
                certainty: row.get(3)?,
                ordered: row.get(4)?,
                performed: row.get(5)?,
                reported: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

pub(crate) fn flags(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<BillingFlag>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT code, ref_id, blocking FROM bil_derivation_flags \
             WHERE derivation_id = ?1 ORDER BY code, ref_id",
        )?
        .query_map([id], |row| {
            Ok(BillingFlag {
                code: row.get(0)?,
                ref_id: row.get(1)?,
                blocking: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}
