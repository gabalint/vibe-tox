//! The HL7 Orders connections the intake worker reads now (ticket #4555 step 3c).

use rusqlite::Connection;

use super::delivery::Destination;
use super::error::PartnerError;
use super::model::{IntakeSettings, PartnerConnectionId, PartnerMessageType};
use super::repo::{connections, verification};

/// An HL7 Orders connection to read: started, with a destination check covering its
/// current folder or server.
#[derive(Debug, Clone)]
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
        let Some(intake) = row.settings.intake.clone() else {
            continue;
        };
        if !row.active || verification(conn, row.id, &row.destination)?.is_none() {
            continue;
        }
        sources.push(IntakeSource {
            connection_id: row.id,
            client_code: row.settings.partner.clone(),
            intake,
            destination: Destination::of(&row),
        });
    }
    Ok(sources)
}
