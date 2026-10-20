//! SQL for Standard Catalog load records (REQ-OPS-014).

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, params};

use crate::model::StandardCatalogLoadId;
use crate::standard::tree::StandardCatalogLoadView;

pub(crate) struct NewLoad<'a> {
    pub(crate) id: StandardCatalogLoadId,
    pub(crate) catalog_version: &'a str,
    pub(crate) file_digest: &'a str,
    pub(crate) loaded_by: UserId,
    pub(crate) loaded_at: UtcTimestamp,
}

/// Records a load and every record it created, as (record type, record ID).
pub(crate) fn insert_load(
    conn: &Connection,
    load: &NewLoad<'_>,
    items: &[(&'static str, String)],
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO cat_standard_loads (id, catalog_version, file_digest, loaded_by, loaded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        load.id,
        load.catalog_version,
        load.file_digest,
        load.loaded_by,
        load.loaded_at
    ])?;
    let mut item = conn.prepare_cached(
        "INSERT INTO cat_standard_load_items (load_id, record_type, record_id) \
         VALUES (?1, ?2, ?3)",
    )?;
    for (record_type, record_id) in items {
        item.execute(params![load.id, record_type, record_id])?;
    }
    Ok(())
}

/// Every load, oldest first, with the number of records it created.
pub(crate) fn loads(conn: &Connection) -> rusqlite::Result<Vec<StandardCatalogLoadView>> {
    conn.prepare_cached(
        "SELECT l.id, l.catalog_version, l.file_digest, l.loaded_by, l.loaded_at, \
         (SELECT count(*) FROM cat_standard_load_items i WHERE i.load_id = l.id) \
         FROM cat_standard_loads l ORDER BY l.loaded_at, l.id",
    )?
    .query_map([], |row| {
        Ok(StandardCatalogLoadView {
            id: row.get(0)?,
            catalog_version: row.get(1)?,
            file_digest: row.get(2)?,
            loaded_by: row.get(3)?,
            loaded_at: row.get(4)?,
            records: row.get(5)?,
        })
    })?
    .collect()
}

/// Loaded method and orderable versions that are still drafts.
pub(crate) fn loaded_drafts(conn: &Connection) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "SELECT count(*) FROM cat_standard_load_items i \
         LEFT JOIN cat_method_versions m \
           ON i.record_type = 'method_version' AND m.id = i.record_id \
         LEFT JOIN cat_orderable_versions o \
           ON i.record_type = 'orderable_version' AND o.id = i.record_id \
         WHERE m.status = 'draft' OR o.status = 'draft'",
    )?
    .query_row([], |row| row.get(0))
}
