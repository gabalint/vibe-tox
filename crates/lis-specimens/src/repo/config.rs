//! SQL for storage locations and label printers (configuration).

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{LocationId, PrinterId, PrinterTransport};
use crate::views::{LabelPrinterView, StorageLocationView};

/// Whether another location than `except` uses `code`.
pub(crate) fn location_code_taken(
    conn: &Connection,
    code: &str,
    except: LocationId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM spc_locations WHERE code = ?1 AND id <> ?2)")?
        .query_row(params![code, except], |row| row.get(0))
}

/// Inserts a location at revision 1, or updates it to `location.revision`.
pub(crate) fn save_location(
    conn: &Connection,
    location: &StorageLocationView,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_locations (id, code, name, storage_condition, active, created_at, \
         revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT (id) DO UPDATE SET code = excluded.code, name = excluded.name, \
         storage_condition = excluded.storage_condition, active = excluded.active, \
         revision = excluded.revision",
    )?
    .execute(params![
        location.location_id,
        location.code,
        location.name,
        location.storage_condition,
        location.active,
        now,
        location.revision
    ])?;
    Ok(())
}

const LOCATION_COLUMNS: &str =
    "id, code, name, storage_condition, active, revision FROM spc_locations";

fn location_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<StorageLocationView> {
    Ok(StorageLocationView {
        location_id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        storage_condition: row.get(3)?,
        active: row.get(4)?,
        revision: row.get(5)?,
    })
}

pub(crate) fn location(
    conn: &Connection,
    id: LocationId,
) -> rusqlite::Result<Option<StorageLocationView>> {
    conn.prepare_cached(&format!("SELECT {LOCATION_COLUMNS} WHERE id = ?1"))?
        .query_row([id], location_view)
        .optional()
}

pub(crate) fn locations(conn: &Connection) -> rusqlite::Result<Vec<StorageLocationView>> {
    conn.prepare_cached(&format!("SELECT {LOCATION_COLUMNS} ORDER BY code"))?
        .query_map([], location_view)?
        .collect()
}

/// Whether another printer than `except` uses `code`.
pub(crate) fn printer_code_taken(
    conn: &Connection,
    code: &str,
    except: PrinterId,
) -> rusqlite::Result<bool> {
    conn.prepare_cached("SELECT EXISTS (SELECT 1 FROM spc_printers WHERE code = ?1 AND id <> ?2)")?
        .query_row(params![code, except], |row| row.get(0))
}

/// Inserts a printer at revision 1, or updates it to `printer.revision`.
pub(crate) fn save_printer(
    conn: &Connection,
    printer: &LabelPrinterView,
    now: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO spc_printers (id, code, name, transport, ip_address, port, queue, active, \
         created_at, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         ON CONFLICT (id) DO UPDATE SET code = excluded.code, name = excluded.name, \
         transport = excluded.transport, ip_address = excluded.ip_address, port = excluded.port, \
         queue = excluded.queue, active = excluded.active, revision = excluded.revision",
    )?
    .execute(params![
        printer.printer_id,
        printer.code,
        printer.name,
        printer.transport,
        printer.ip_address,
        printer.port,
        printer.queue,
        printer.active,
        now,
        printer.revision
    ])?;
    Ok(())
}

const PRINTER_COLUMNS: &str =
    "id, code, name, transport, ip_address, port, queue, active, revision FROM spc_printers";

fn printer_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<LabelPrinterView> {
    let transport: PrinterTransport = row.get(3)?;
    Ok(LabelPrinterView {
        printer_id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        transport,
        ip_address: row.get(4)?,
        port: row.get(5)?,
        queue: row.get(6)?,
        active: row.get(7)?,
        revision: row.get(8)?,
    })
}

pub(crate) fn printer(
    conn: &Connection,
    id: PrinterId,
) -> rusqlite::Result<Option<LabelPrinterView>> {
    conn.prepare_cached(&format!("SELECT {PRINTER_COLUMNS} WHERE id = ?1"))?
        .query_row([id], printer_view)
        .optional()
}

pub(crate) fn printers(conn: &Connection) -> rusqlite::Result<Vec<LabelPrinterView>> {
    conn.prepare_cached(&format!("SELECT {PRINTER_COLUMNS} ORDER BY code"))?
        .query_map([], printer_view)?
        .collect()
}
