//! Bounded patient directory reads. All matching values are bound parameters.

use lis_core::Phi;
use rusqlite::{Connection, params};

use crate::model::{SubjectId, name_key};

pub(crate) struct PatientSearch {
    pub(crate) identifier_owner: Option<SubjectId>,
    pub(crate) has_identifier: bool,
    pub(crate) name_prefix: Option<Phi<String>>,
    pub(crate) query: Option<Phi<String>>,
    pub(crate) birth_date: Option<Phi<String>>,
    pub(crate) limit: u32,
    pub(crate) offset: u32,
}

/// Prefix comparisons use substr rather than LIKE so %, _ and escape characters
/// remain literal. The normalized name key already handles Unicode composition.
pub(crate) fn patients(
    conn: &Connection,
    search: &PatientSearch,
) -> rusqlite::Result<Vec<SubjectId>> {
    let query = search.query.as_ref().map(Phi::expose);
    let normalized = query.map(|value| Phi::new(name_key(value)));
    let browse = !search.has_identifier && search.name_prefix.is_none() && query.is_none();
    conn.prepare_cached(
        "WITH names AS (
            SELECT subject_id, match_key, birth_date,
                   substr(match_key, 1, instr(match_key, char(31)) - 1) AS family,
                   substr(match_key, instr(match_key, char(31)) + 1,
                          length(match_key) - instr(match_key, char(31)) - 11) AS given
            FROM idn_identity_assertions
         )
         SELECT s.id FROM idn_subjects s
         JOIN idn_identity_assertions current ON current.subject_id = s.id
           AND current.sequence = (SELECT max(sequence) FROM idn_identity_assertions
                                   WHERE subject_id = s.id)
         WHERE s.id = ?1 OR EXISTS (
            SELECT 1 FROM names n WHERE n.subject_id = s.id
            AND (?5 IS NULL OR n.birth_date = ?5)
            AND (?6 OR (?2 IS NOT NULL AND substr(n.match_key, 1, length(?2)) = ?2)
                OR (?3 IS NOT NULL AND (
                    substr(n.family, 1, length(?3)) = ?3
                    OR substr(n.given, 1, length(?3)) = ?3
                    OR substr(n.given || ' ' || n.family, 1, length(?3)) = ?3
                    OR substr(n.family || ' ' || n.given, 1, length(?3)) = ?3
                    OR substr(n.family || ', ' || n.given, 1, length(?3)) = ?3
                    OR EXISTS (SELECT 1 FROM idn_subject_identifiers i
                               WHERE i.subject_id = s.id AND i.status = 'active'
                               AND substr(i.value, 1, length(?4)) = ?4)))))
         ORDER BY CASE WHEN s.id = ?1 THEN 0 ELSE 1 END, current.match_key, s.id
         LIMIT ?7 OFFSET ?8",
    )?
    .query_map(
        params![
            search.identifier_owner,
            search.name_prefix.as_ref().map(Phi::expose),
            normalized.as_ref().map(Phi::expose),
            query,
            search.birth_date.as_ref().map(Phi::expose),
            browse,
            search.limit,
            search.offset,
        ],
        |row| row.get(0),
    )?
    .collect()
}
