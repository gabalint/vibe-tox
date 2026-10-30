use std::net::IpAddr;

use lis_core::InstrumentId;
use rusqlite::{Connection, OptionalExtension, Row, params};

mod au;
mod files;
mod receipts;

pub(crate) use au::{au_profile, insert_au_profile};
mod au_guards;
pub(crate) use au_guards::{guard_au_multiblock, requires_au_restart};

pub(crate) use files::{
    in_baseline, insert_baseline_file, insert_file_receipt, insert_seen_file, receipt_file,
    result_folder_in_use, seen_receipt, update_result_folder,
};
pub(crate) use receipts::{
    complete_receipt, first_au_duplicate, first_duplicate, frames_for_receipt, insert_frame,
    insert_receipt, receipt_by_id, receiving_receipts, terminate_receipt,
};

use crate::InterfaceError;
use crate::model::{
    ChangeStamp, ConnectionSettings, DataBits, Instrument, InstrumentTransport, Parity, StopBits,
};

pub(crate) fn insert_instrument(
    conn: &Connection,
    instrument: &Instrument,
) -> Result<(), InterfaceError> {
    let fields = ConnectionFields::new(&instrument.connection)?;
    conn.execute(
        "INSERT INTO ifc_instruments (
            id, code, name, connection_kind, tcp_bind_address, tcp_port, tcp_allowed_peers,
            serial_port_path, serial_baud_rate, serial_data_bits, serial_parity,
            serial_stop_bits, result_folder, enabled, revision, au_settings,
            created_by, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            instrument.id,
            instrument.code,
            instrument.name,
            instrument.connection.transport().as_str(),
            fields.tcp_bind_address,
            fields.tcp_port,
            fields.tcp_allowed_peers,
            fields.serial_port_path,
            fields.serial_baud_rate,
            fields.serial_data_bits,
            fields.serial_parity,
            fields.serial_stop_bits,
            instrument.result_folder,
            instrument.enabled,
            instrument.revision,
            fields.au_settings,
            instrument.created.as_ref().map(|stamp| &stamp.by),
            instrument.created.as_ref().map(|stamp| stamp.at),
        ],
    )?;
    Ok(())
}

/// Records who changed the configuration and when, beside the change itself.
pub(crate) fn record_change(
    conn: &Connection,
    instrument_id: InstrumentId,
    stamp: &ChangeStamp,
) -> Result<(), InterfaceError> {
    conn.execute(
        "UPDATE ifc_instruments SET changed_by = ?, changed_at = ? WHERE id = ?",
        params![stamp.by, stamp.at, instrument_id],
    )?;
    Ok(())
}

pub(crate) fn update_connection(
    conn: &Connection,
    instrument_id: InstrumentId,
    connection: &ConnectionSettings,
    next_revision: i64,
) -> Result<(), InterfaceError> {
    let fields = ConnectionFields::new(connection)?;
    // A watched folder belongs to the result-file connection; another kind drops it.
    conn.execute(
        "UPDATE ifc_instruments SET
            connection_kind = ?1, tcp_bind_address = ?2, tcp_port = ?3, tcp_allowed_peers = ?4,
            serial_port_path = ?5, serial_baud_rate = ?6, serial_data_bits = ?7,
            serial_parity = ?8, serial_stop_bits = ?9, revision = ?10,
            result_folder = CASE WHEN ?1 = 'result_file' THEN result_folder END,
            au_settings = ?12
         WHERE id = ?11",
        params![
            connection.transport().as_str(),
            fields.tcp_bind_address,
            fields.tcp_port,
            fields.tcp_allowed_peers,
            fields.serial_port_path,
            fields.serial_baud_rate,
            fields.serial_data_bits,
            fields.serial_parity,
            fields.serial_stop_bits,
            next_revision,
            instrument_id,
            fields.au_settings,
        ],
    )?;
    Ok(())
}

pub(crate) fn update_enabled(
    conn: &Connection,
    instrument_id: InstrumentId,
    enabled: bool,
    next_revision: i64,
) -> Result<(), InterfaceError> {
    conn.execute(
        "UPDATE ifc_instruments SET enabled = ?, revision = ? WHERE id = ?",
        params![enabled, next_revision, instrument_id],
    )?;
    Ok(())
}

pub(crate) fn instrument_by_id(
    conn: &Connection,
    instrument_id: InstrumentId,
) -> Result<Option<Instrument>, InterfaceError> {
    conn.query_row(
        "SELECT id, code, name, connection_kind, tcp_bind_address, tcp_port, tcp_allowed_peers,
                serial_port_path, serial_baud_rate, serial_data_bits, serial_parity,
                serial_stop_bits, enabled, revision, result_folder, au_settings,
                created_by, created_at, changed_by, changed_at
         FROM ifc_instruments WHERE id = ?",
        [instrument_id],
        map_instrument,
    )
    .optional()
    .map_err(InterfaceError::from)
}

pub(crate) fn all_instruments(conn: &Connection) -> Result<Vec<Instrument>, InterfaceError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, code, name, connection_kind, tcp_bind_address, tcp_port, tcp_allowed_peers,
                serial_port_path, serial_baud_rate, serial_data_bits, serial_parity,
                serial_stop_bits, enabled, revision, result_folder, au_settings,
                created_by, created_at, changed_by, changed_at
         FROM ifc_instruments ORDER BY code",
    )?;
    let rows = statement.query_map([], map_instrument)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(InterfaceError::from)
}

pub(crate) fn code_in_use(conn: &Connection, code: &str) -> Result<bool, InterfaceError> {
    let found: i64 = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_instruments WHERE code = ?)",
        [code],
        |row| row.get(0),
    )?;
    Ok(found != 0)
}

pub(crate) fn connection_in_use(
    conn: &Connection,
    connection: &ConnectionSettings,
    excluding: Option<InstrumentId>,
) -> Result<bool, InterfaceError> {
    let (sql, value) = match connection {
        ConnectionSettings::AstmTcp {
            bind_address, port, ..
        } | ConnectionSettings::AuTcp { bind_address, port, .. } => (
            "SELECT EXISTS(SELECT 1 FROM ifc_instruments
             WHERE connection_kind IN ('astm_tcp', 'au_tcp') AND tcp_bind_address = ?1 AND tcp_port = ?2
             AND (?3 IS NULL OR id <> ?3))",
            (bind_address.to_string(), i64::from(*port)),
        ),
        ConnectionSettings::AstmSerial { port_path, .. } | ConnectionSettings::AuSerial { port_path, .. } => {
            let found: i64 = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM ifc_instruments
                 WHERE connection_kind IN ('astm_serial', 'au_serial') AND serial_port_path = ?1
                 AND (?2 IS NULL OR id <> ?2))",
                params![port_path, excluding],
                |row| row.get(0),
            )?;
            return Ok(found != 0);
        }
        // A result-file connection has no endpoint; its folder is checked when set.
        ConnectionSettings::ResultFile {} => return Ok(false),
    };
    let found: i64 = conn.query_row(sql, params![value.0, value.1, excluding], |row| row.get(0))?;
    Ok(found != 0)
}

fn map_instrument(row: &Row<'_>) -> rusqlite::Result<Instrument> {
    let kind = InstrumentTransport::parse(row.get_ref(3)?.as_str()?)?;
    let connection = match kind {
        InstrumentTransport::AstmTcp => ConnectionSettings::AstmTcp {
            bind_address: parse_ip(row.get_ref(4)?.as_str()?)?,
            port: row.get(5)?,
            allowed_peers: parse_allowed_peers(row.get_ref(6)?.as_str()?)?,
        },
        InstrumentTransport::AstmSerial => ConnectionSettings::AstmSerial {
            port_path: row.get(7)?,
            baud_rate: row.get(8)?,
            data_bits: DataBits::parse(row.get(9)?)?,
            parity: Parity::parse(row.get_ref(10)?.as_str()?)?,
            stop_bits: StopBits::parse(row.get(11)?)?,
        },
        InstrumentTransport::AuTcp => ConnectionSettings::AuTcp {
            bind_address: parse_ip(row.get_ref(4)?.as_str()?)?,
            port: row.get(5)?,
            allowed_peers: parse_allowed_peers(row.get_ref(6)?.as_str()?)?,
            au: parse_au_settings(row)?,
        },
        InstrumentTransport::AuSerial => ConnectionSettings::AuSerial {
            port_path: row.get(7)?,
            baud_rate: row.get(8)?,
            data_bits: DataBits::parse(row.get(9)?)?,
            parity: Parity::parse(row.get_ref(10)?.as_str()?)?,
            stop_bits: StopBits::parse(row.get(11)?)?,
            au: parse_au_settings(row)?,
        },
        InstrumentTransport::ResultFile => ConnectionSettings::ResultFile {},
    };
    Ok(Instrument {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        connection,
        result_folder: row.get(14)?,
        enabled: row.get(12)?,
        revision: row.get(13)?,
        created: change_stamp(row, 16)?,
        changed: change_stamp(row, 18)?,
    })
}

/// The stamp in columns `by` and `by + 1`; absent when it was never recorded.
fn change_stamp(row: &Row<'_>, by: usize) -> rusqlite::Result<Option<ChangeStamp>> {
    let who: Option<String> = row.get(by)?;
    let at = row.get(by + 1)?;
    Ok(who.zip(at).map(|(by, at)| ChangeStamp {
        by,
        by_display: None,
        at,
    }))
}

fn parse_au_settings(row: &Row<'_>) -> rusqlite::Result<crate::AuConnectionSettings> {
    let settings: crate::AuConnectionSettings = serde_json::from_str(row.get_ref(15)?.as_str()?)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    settings
        .validate()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(settings)
}

fn parse_ip(value: &str) -> rusqlite::Result<IpAddr> {
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_allowed_peers(value: &str) -> rusqlite::Result<Vec<IpAddr>> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error))
    })
}

struct ConnectionFields<'a> {
    au_settings: Option<String>,
    tcp_bind_address: Option<String>,
    tcp_port: Option<u16>,
    tcp_allowed_peers: Option<String>,
    serial_port_path: Option<&'a str>,
    serial_baud_rate: Option<u32>,
    serial_data_bits: Option<u8>,
    serial_parity: Option<&'static str>,
    serial_stop_bits: Option<u8>,
}

impl<'a> ConnectionFields<'a> {
    fn new(connection: &'a ConnectionSettings) -> Result<Self, InterfaceError> {
        let au_settings = connection
            .au_settings()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| InterfaceError::InvalidAuConfiguration)?;
        let fields = match connection {
            ConnectionSettings::AstmTcp {
                bind_address,
                port,
                allowed_peers,
            }
            | ConnectionSettings::AuTcp {
                bind_address,
                port,
                allowed_peers,
                ..
            } => Self {
                au_settings,
                tcp_bind_address: Some(bind_address.to_string()),
                tcp_port: Some(*port),
                tcp_allowed_peers: Some(
                    serde_json::to_string(allowed_peers)
                        .map_err(|_| InterfaceError::InvalidTcpAllowedPeers)?,
                ),
                serial_port_path: None,
                serial_baud_rate: None,
                serial_data_bits: None,
                serial_parity: None,
                serial_stop_bits: None,
            },
            ConnectionSettings::AstmSerial {
                port_path,
                baud_rate,
                data_bits,
                parity,
                stop_bits,
            }
            | ConnectionSettings::AuSerial {
                port_path,
                baud_rate,
                data_bits,
                parity,
                stop_bits,
                ..
            } => Self {
                au_settings,
                tcp_bind_address: None,
                tcp_port: None,
                tcp_allowed_peers: None,
                serial_port_path: Some(port_path),
                serial_baud_rate: Some(*baud_rate),
                serial_data_bits: Some(data_bits.value()),
                serial_parity: Some(parity.as_str()),
                serial_stop_bits: Some(stop_bits.value()),
            },
            ConnectionSettings::ResultFile {} => Self {
                au_settings: None,
                tcp_bind_address: None,
                tcp_port: None,
                tcp_allowed_peers: None,
                serial_port_path: None,
                serial_baud_rate: None,
                serial_data_bits: None,
                serial_parity: None,
                serial_stop_bits: None,
            },
        };
        Ok(fields)
    }
}
