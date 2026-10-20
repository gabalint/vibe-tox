use lis_core::{Capability, InstrumentId, Principal};
use lis_store::StoreError;
use rusqlite::Connection;

use crate::model::{Instrument, InstrumentTransport, Receipt, ReceiptFrame, ReceiptId};
use crate::{InterfaceError, repo};

/// Returns one instrument configuration to an authorized administrator.
pub fn get_instrument(
    conn: &Connection,
    principal: &Principal,
    instrument_id: InstrumentId,
) -> Result<Instrument, InterfaceError> {
    require(principal, Capability::ManageConfiguration)?;
    repo::instrument_by_id(conn, instrument_id)?.ok_or(InterfaceError::InstrumentNotFound)
}

/// Lists instrument configurations for administration.
pub fn list_instruments(
    conn: &Connection,
    principal: &Principal,
) -> Result<Vec<Instrument>, InterfaceError> {
    require(principal, Capability::ManageConfiguration)?;
    repo::all_instruments(conn)
}

/// Returns one durable receipt for an authorized intake worker.
pub fn get_receipt(
    conn: &Connection,
    principal: &Principal,
    receipt_id: ReceiptId,
) -> Result<Receipt, InterfaceError> {
    require(principal, Capability::InstrumentIntake)?;
    repo::receipt_by_id(conn, receipt_id)?.ok_or(InterfaceError::ReceiptNotFound)
}

/// The AU wire profile frozen when a receipt began, for authorized import/recovery.
pub fn receipt_au_profile(
    conn: &Connection,
    principal: &Principal,
    receipt_id: ReceiptId,
) -> Result<Option<lis_au::AuConfig>, InterfaceError> {
    require(principal, Capability::InstrumentIntake)?;
    repo::au_profile(conn, receipt_id)
}

/// Returns exact accepted frame bytes in durable order.
pub fn list_receipt_frames(
    conn: &Connection,
    principal: &Principal,
    receipt_id: ReceiptId,
) -> Result<Vec<ReceiptFrame>, InterfaceError> {
    require(principal, Capability::InstrumentIntake)?;
    if repo::receipt_by_id(conn, receipt_id)?.is_none() {
        return Err(InterfaceError::ReceiptNotFound);
    }
    repo::frames_for_receipt(conn, receipt_id)
}

fn require(principal: &Principal, capability: Capability) -> Result<(), InterfaceError> {
    principal
        .require(capability)
        .map_err(StoreError::from)
        .map_err(InterfaceError::from)
}

/// Instrument IDs and display names, ordered by name, for the listener status
/// shown even at the sign-in screen (REQ-OPS-013). Names are laboratory
/// configuration, not patient data, so no capability is required.
pub fn instrument_names(conn: &Connection) -> Result<Vec<(InstrumentId, String)>, StoreError> {
    let mut statement =
        conn.prepare_cached("SELECT id, name FROM ifc_instruments ORDER BY name")?;
    let names = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(names.collect::<Result<_, _>>()?)
}

/// The connection type of one instrument, for the bench readiness view.
/// Laboratory configuration, not patient data, so no capability is required.
pub fn instrument_transport(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<InstrumentTransport, InterfaceError> {
    Ok(repo::instrument_by_id(conn, instrument_id)?
        .ok_or(InterfaceError::InstrumentNotFound)?
        .connection
        .transport())
}

/// Current AU format for configuration approval; contains no patient data.
pub fn instrument_au_profile(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<Option<lis_au::AuConfig>, InterfaceError> {
    Ok(repo::instrument_by_id(conn, instrument_id)?
        .ok_or(InterfaceError::InstrumentNotFound)?
        .connection
        .au_profile()
        .cloned())
}
