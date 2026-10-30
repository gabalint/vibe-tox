//! SQL for the reference rows: specimen types, analytes, methods and
//! orderables. Analytes are written through [`super::analytes`]. The
//! directories carry each record's version state.

use lis_core::{Id, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::lifecycle::Versioned;
use crate::model::{
    MethodId, MethodVersion, OrderableId, OrderableKind, OrderableVersion, SpecimenTypeId,
    ValidityPolicyVersion,
};
use crate::repo::lifecycle::version_state_columns;
use crate::views::{AnalyteView, MethodView, OrderableView, SpecimenTypeView};

/// The reference tables, so a code check names its table explicitly.
#[derive(Clone, Copy)]
pub(crate) enum CodeTable {
    SpecimenTypes,
    Analytes,
    Methods,
    Orderables,
}

impl CodeTable {
    fn name(self) -> &'static str {
        match self {
            Self::SpecimenTypes => "cat_specimen_types",
            Self::Analytes => "cat_analytes",
            Self::Methods => "cat_methods",
            Self::Orderables => "cat_orderables",
        }
    }
}

pub(crate) fn code_exists(
    conn: &Connection,
    table: CodeTable,
    code: &str,
) -> rusqlite::Result<bool> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE code = ?1)",
        table.name()
    );
    conn.prepare_cached(&sql)?
        .query_row([code], |row| row.get(0))
}

pub(crate) fn specimen_type_exists(
    conn: &Connection,
    id: SpecimenTypeId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM cat_specimen_types WHERE id = ?1)")?
        .query_row([id], |row| row.get(0))
}

pub(crate) fn insert_specimen_type(
    conn: &Connection,
    id: SpecimenTypeId,
    code: &str,
    name: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_specimen_types (id, code, name, created_at) VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![id, code, name, now])?;
    Ok(())
}

pub(crate) fn insert_method(
    conn: &Connection,
    id: MethodId,
    code: &str,
    name: &str,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_methods (id, code, name, created_at) VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![id, code, name, now])?;
    Ok(())
}

pub(crate) fn insert_orderable(
    conn: &Connection,
    id: OrderableId,
    code: &str,
    name: &str,
    kind: OrderableKind,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_orderables (id, code, name, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![id, code, name, kind, now])?;
    Ok(())
}

pub(crate) fn orderable_kind(
    conn: &Connection,
    id: OrderableId,
) -> rusqlite::Result<Option<OrderableKind>> {
    conn.prepare_cached("SELECT kind FROM cat_orderables WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()
}

/// Every specimen type by code, with its validity policy version state.
pub(crate) fn specimen_types(conn: &Connection) -> rusqlite::Result<Vec<SpecimenTypeView>> {
    directory::<ValidityPolicyVersion, _>(conn, "", |(id, code, name, in_effect, draft), _| {
        Ok(SpecimenTypeView {
            id,
            code,
            name,
            version_in_effect: in_effect,
            draft_version: draft,
        })
    })
}

pub(crate) fn analytes(conn: &Connection) -> rusqlite::Result<Vec<AnalyteView>> {
    conn.prepare_cached(
        "SELECT id, code, name, loinc_code, drug_class_code, active, revision \
         FROM cat_analytes ORDER BY code",
    )?
    .query_map([], super::analytes::view)?
    .collect()
}

/// Every method by code, with its version state.
pub(crate) fn methods(conn: &Connection) -> rusqlite::Result<Vec<MethodView>> {
    directory::<MethodVersion, _>(conn, "", |(id, code, name, in_effect, draft), _| {
        Ok(MethodView {
            id,
            code,
            name,
            version_in_effect: in_effect,
            draft_version: draft,
        })
    })
}

/// Every orderable by code, with its version state.
pub(crate) fn orderables(conn: &Connection) -> rusqlite::Result<Vec<OrderableView>> {
    directory::<OrderableVersion, _>(
        conn,
        ", p.kind",
        |(id, code, name, in_effect, draft), row| {
            Ok(OrderableView {
                id,
                code,
                name,
                kind: row.get(5)?,
                version_in_effect: in_effect,
                draft_version: draft,
            })
        },
    )
}

/// A directory row's shared columns: ID, code, name, then the numbers of the
/// version in effect and of the open draft.
type DirectoryColumns<P> = (Id<P>, String, String, Option<i64>, Option<i64>);

/// One query over the parent table of `K`, aliased `p` and ordered by code:
/// [`DirectoryColumns`] (indexes 0–4), then `extra_columns` from index 5.
/// `extra_columns` is a constant column list, never input.
fn directory<K: Versioned, T>(
    conn: &Connection,
    extra_columns: &str,
    build: impl Fn(DirectoryColumns<K::Parent>, &Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    let sql = format!(
        "SELECT p.id, p.code, p.name, {}{extra_columns} FROM {} p ORDER BY p.code",
        version_state_columns::<K>(),
        K::PARENT_TABLE
    );
    conn.prepare_cached(&sql)?
        .query_map([], |row| {
            let columns = (
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            );
            build(columns, row)
        })?
        .collect()
}
