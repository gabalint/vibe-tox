//! SQL for subjects, identity assertions and subject identifiers.

use lis_core::{Phi, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    ActorRef, Demographics, Identifier, IdentityAssertionId, SubjectId, SubjectIdentifierId,
};
use crate::views::{AssertionView, IdentifierView};

/// A new assertion row.
pub(crate) struct NewAssertion<'a> {
    pub(crate) id: IdentityAssertionId,
    pub(crate) subject_id: SubjectId,
    pub(crate) sequence: i64,
    pub(crate) demographics: &'a Demographics,
    pub(crate) source: &'a str,
    pub(crate) reason: Option<&'a str>,
    pub(crate) asserted_at: UtcTimestamp,
    pub(crate) asserted_by: &'a ActorRef,
}

pub(crate) fn insert_subject(
    conn: &Connection,
    id: SubjectId,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached("INSERT INTO idn_subjects (id, revision, created_at) VALUES (?1, 1, ?2)")?
        .execute(params![id, now])?;
    Ok(())
}

pub(crate) fn subject_revision(conn: &Connection, id: SubjectId) -> rusqlite::Result<Option<i64>> {
    conn.prepare_cached("SELECT revision FROM idn_subjects WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()
}

pub(crate) fn set_subject_revision(
    conn: &Connection,
    id: SubjectId,
    revision: i64,
) -> rusqlite::Result<()> {
    conn.prepare_cached("UPDATE idn_subjects SET revision = ?2 WHERE id = ?1")?
        .execute(params![id, revision])?;
    Ok(())
}

pub(crate) fn next_assertion_sequence(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM idn_identity_assertions WHERE subject_id = ?1",
    )?
    .query_row([subject_id], |row| row.get(0))
}

pub(crate) fn insert_assertion(conn: &Connection, new: &NewAssertion<'_>) -> rusqlite::Result<()> {
    let d = new.demographics;
    conn.prepare_cached(
        "INSERT INTO idn_identity_assertions (id, subject_id, sequence, family_name, \
         given_names, birth_date, administrative_sex, source, reason, match_key, asserted_at, \
         asserted_by_kind, asserted_by_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?
    .execute(params![
        new.id,
        new.subject_id,
        new.sequence,
        d.family_name.expose(),
        d.given_names.as_ref().map(Phi::expose),
        d.birth_date.expose(),
        d.administrative_sex.as_ref().map(Phi::expose),
        new.source,
        new.reason,
        d.match_key().expose(),
        new.asserted_at,
        new.asserted_by.kind,
        new.asserted_by.id,
    ])?;
    Ok(())
}

/// Subjects with any assertion (current or earlier) under `match_key`, in ID order.
pub(crate) fn subjects_by_match_key(
    conn: &Connection,
    match_key: &Phi<String>,
    excluding: Option<SubjectId>,
) -> rusqlite::Result<Vec<SubjectId>> {
    conn.prepare_cached(
        "SELECT DISTINCT subject_id FROM idn_identity_assertions WHERE match_key = ?1 \
         AND (?2 IS NULL OR subject_id <> ?2) ORDER BY subject_id",
    )?
    .query_map(params![match_key.expose(), excluding], |row| row.get(0))?
    .collect()
}

pub(crate) fn assertions(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<Vec<AssertionView>> {
    conn.prepare_cached(
        "SELECT id, sequence, family_name, given_names, birth_date, administrative_sex, source, \
         reason, asserted_at, asserted_by_kind, asserted_by_id \
         FROM idn_identity_assertions WHERE subject_id = ?1 ORDER BY sequence",
    )?
    .query_map([subject_id], assertion_view)?
    .collect()
}

/// The subject's latest assertion: its current demographics.
pub(crate) fn current_assertion(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<Option<AssertionView>> {
    conn.prepare_cached(
        "SELECT id, sequence, family_name, given_names, birth_date, administrative_sex, source, \
         reason, asserted_at, asserted_by_kind, asserted_by_id \
         FROM idn_identity_assertions WHERE subject_id = ?1 ORDER BY sequence DESC LIMIT 1",
    )?
    .query_row([subject_id], assertion_view)
    .optional()
}

fn assertion_view(row: &Row<'_>) -> rusqlite::Result<AssertionView> {
    Ok(AssertionView {
        id: row.get(0)?,
        sequence: row.get(1)?,
        family_name: Phi::new(row.get(2)?),
        given_names: row.get::<_, Option<String>>(3)?.map(Phi::new),
        birth_date: Phi::new(row.get(4)?),
        administrative_sex: row.get::<_, Option<String>>(5)?.map(Phi::new),
        source: row.get(6)?,
        reason: row.get(7)?,
        asserted_at: row.get(8)?,
        asserted_by_kind: row.get(9)?,
        asserted_by_id: row.get(10)?,
    })
}

/// The subject holding `identifier` as an active identifier, if any.
pub(crate) fn active_identifier_owner(
    conn: &Connection,
    identifier: &Identifier,
) -> rusqlite::Result<Option<SubjectId>> {
    conn.prepare_cached(
        "SELECT subject_id FROM idn_subject_identifiers WHERE assigning_authority = ?1 \
         AND identifier_type = ?2 AND value = ?3 AND status = 'active'",
    )?
    .query_row(
        params![
            identifier.assigning_authority,
            identifier.identifier_type,
            identifier.value.expose()
        ],
        |row| row.get(0),
    )
    .optional()
}

/// The subject that has ever held `identifier`, whether active or inactive.
pub(crate) fn historical_identifier_owner(
    conn: &Connection,
    identifier: &Identifier,
) -> rusqlite::Result<Option<SubjectId>> {
    conn.prepare_cached(
        "SELECT subject_id FROM idn_subject_identifiers WHERE assigning_authority = ?1 \
         AND identifier_type = ?2 AND value = ?3",
    )?
    .query_row(
        params![
            identifier.assigning_authority,
            identifier.identifier_type,
            identifier.value.expose()
        ],
        |row| row.get(0),
    )
    .optional()
}

pub(crate) fn insert_identifier(
    conn: &Connection,
    id: SubjectIdentifierId,
    subject_id: SubjectId,
    identifier: &Identifier,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO idn_subject_identifiers (id, subject_id, assigning_authority, \
         identifier_type, value, status, added_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
    )?
    .execute(params![
        id,
        subject_id,
        identifier.assigning_authority,
        identifier.identifier_type,
        identifier.value.expose(),
        now,
    ])?;
    Ok(())
}

/// `(subject, active)` for an identifier row.
pub(crate) fn identifier_state(
    conn: &Connection,
    id: SubjectIdentifierId,
) -> rusqlite::Result<Option<(SubjectId, bool)>> {
    conn.prepare_cached(
        "SELECT subject_id, status = 'active' FROM idn_subject_identifiers WHERE id = ?1",
    )?
    .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
    .optional()
}

pub(crate) fn deactivate_identifier(
    conn: &Connection,
    id: SubjectIdentifierId,
    now: UtcTimestamp,
    reason: &str,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "UPDATE idn_subject_identifiers SET status = 'inactive', deactivated_at = ?2, \
         deactivation_reason = ?3 WHERE id = ?1",
    )?
    .execute(params![id, now, reason])?;
    Ok(())
}

pub(crate) fn identifiers(
    conn: &Connection,
    subject_id: SubjectId,
) -> rusqlite::Result<Vec<IdentifierView>> {
    conn.prepare_cached(
        "SELECT id, assigning_authority, identifier_type, value, status = 'active', added_at, \
         deactivated_at, deactivation_reason FROM idn_subject_identifiers \
         WHERE subject_id = ?1 ORDER BY id",
    )?
    .query_map([subject_id], |row| {
        Ok(IdentifierView {
            id: row.get(0)?,
            assigning_authority: row.get(1)?,
            identifier_type: row.get(2)?,
            value: Phi::new(row.get(3)?),
            active: row.get(4)?,
            added_at: row.get(5)?,
            deactivated_at: row.get(6)?,
            deactivation_reason: row.get(7)?,
        })
    })?
    .collect()
}
