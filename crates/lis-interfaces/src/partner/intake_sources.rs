//! The HL7 Orders connections the intake worker reads now (ticket #4555 step 3c).

use rusqlite::Connection;

use super::acceptance::accepted;
use super::delivery::Destination;
use super::error::PartnerError;
use super::model::{IntakeSettings, PartnerConnectionId, PartnerMessageType};
use super::repo::{ConnectionRow, connections, load_connection, verification};

/// An HL7 Orders connection to read, or to listen for: started, with a destination check
/// covering its current folder, server or listener, and passed acceptance tests for it as it
/// is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakeSource {
    pub connection_id: PartnerConnectionId,
    /// The laboratory's client the orders are for, by code.
    pub client_code: String,
    pub intake: IntakeSettings,
    /// The input folder or server, with the Processed and Failed folders.
    pub destination: Destination,
}

/// Every HL7 Orders connection to read now, by name.
pub fn intake_sources(conn: &Connection) -> Result<Vec<IntakeSource>, PartnerError> {
    let mut sources = Vec::new();
    for row in connections(conn, Some(PartnerMessageType::Hl7Orders))? {
        if !row.active
            || verification(conn, row.id, &row.destination)?.is_none()
            || !accepted(conn, row.id)?
        {
            continue;
        }
        sources.extend(source_of(&row));
    }
    Ok(sources)
}

/// An HL7 Orders connection as its acceptance tests read it, whether or not it reads now;
/// `None` for another message type.
pub fn intake_source(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<Option<IntakeSource>, PartnerError> {
    let row = load_connection(conn, id)?.ok_or(PartnerError::NotFound("connection"))?;
    Ok(source_of(&row))
}

fn source_of(row: &ConnectionRow) -> Option<IntakeSource> {
    Some(IntakeSource {
        connection_id: row.id,
        client_code: row.settings.partner.clone(),
        intake: row.settings.intake.clone()?,
        destination: Destination::of(row),
    })
}
