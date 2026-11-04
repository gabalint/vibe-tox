//! How a connection is stored: its columns, read and written in one order.

use rusqlite::{Connection, OptionalExtension, Row, params};

use super::error::PartnerError;
use super::model::{
    IntakeSettings, MovedTo, PartnerConnectionId, PartnerConnectionSettings, PartnerMessageType,
    Schedule, SftpAuth, SftpSettings,
};
use super::repo::ConnectionRow;

const COLUMNS: &str = "id, name, partner, message_type, protocol, folder, folder_set_by, rules, \
     schedule, schedule_minutes, schedule_time, file_name_pattern, active, revision, sftp_host, \
     sftp_port, sftp_username, sftp_auth, sftp_fingerprint, sftp_password, destination, intake, \
     processed_folder, failed_folder";

/// The row, with its rules and intake settings still as JSON.
struct Stored {
    row: ConnectionRow,
    rules: String,
    intake: Option<String>,
}

fn connection_row(row: &Row<'_>) -> rusqlite::Result<Stored> {
    let schedule = match row.get::<_, String>(8)?.as_str() {
        "every_minutes" => Schedule::EveryMinutes {
            minutes: row.get(9)?,
        },
        "daily_at" => Schedule::DailyAt { time: row.get(10)? },
        _ => Schedule::Immediately,
    };
    let folder: String = row.get(5)?;
    let sftp = match row.get::<_, Option<String>>(14)? {
        Some(host) => Some(SftpSettings {
            host,
            port: row.get(15)?,
            username: row.get(16)?,
            auth: row.get(17)?,
            fingerprint: row.get(18)?,
            remote_folder: folder.clone(),
        }),
        None => None,
    };
    let moved_to = match (row.get(22)?, row.get(23)?) {
        (Some(processed), Some(failed)) => Some(MovedTo { processed, failed }),
        _ => None,
    };
    Ok(Stored {
        row: ConnectionRow {
            id: row.get(0)?,
            settings: PartnerConnectionSettings {
                name: row.get(1)?,
                partner: row.get(2)?,
                message_type: row.get(3)?,
                protocol: row.get(4)?,
                rules: Vec::new(),
                schedule,
                file_name_pattern: row.get(11)?,
                sftp,
                intake: None,
            },
            folder,
            moved_to,
            folder_set_by: row.get(6)?,
            sealed_password: row.get(19)?,
            destination: row.get(20)?,
            active: row.get(12)?,
            revision: row.get(13)?,
        },
        rules: row.get(7)?,
        intake: row.get(21)?,
    })
}

/// The row with its JSON read. An SFTP connection's Processed and Failed folders are on
/// the server, so its intake settings show them.
fn decoded(stored: Stored) -> Result<ConnectionRow, PartnerError> {
    let Stored {
        mut row,
        rules,
        intake,
    } = stored;
    row.settings.rules = serde_json::from_str(&rules)?;
    if let Some(intake) = intake {
        let intake: IntakeSettings = serde_json::from_str(&intake)?;
        let remote = |folder: fn(&MovedTo) -> &String| {
            row.settings
                .sftp
                .as_ref()
                .and(row.moved_to.as_ref())
                .map(|moved_to| folder(moved_to).clone())
        };
        row.settings.intake = Some(IntakeSettings {
            remote_processed_folder: remote(|moved_to| &moved_to.processed),
            remote_failed_folder: remote(|moved_to| &moved_to.failed),
            ..intake
        });
    }
    Ok(row)
}

/// The SFTP columns, all `None` for a folder.
type SftpColumns<'a> = (
    Option<&'a str>,
    Option<u16>,
    Option<&'a str>,
    Option<SftpAuth>,
    Option<&'a str>,
);

fn sftp_columns(settings: &PartnerConnectionSettings) -> SftpColumns<'_> {
    match &settings.sftp {
        Some(sftp) => (
            Some(&sftp.host),
            Some(sftp.port),
            Some(&sftp.username),
            Some(sftp.auth),
            Some(&sftp.fingerprint),
        ),
        None => (None, None, None, None, None),
    }
}

fn schedule_columns(schedule: &Schedule) -> (&'static str, Option<u16>, Option<&str>) {
    match schedule {
        Schedule::Immediately => ("immediately", None, None),
        Schedule::EveryMinutes { minutes } => ("every_minutes", Some(*minutes), None),
        Schedule::DailyAt { time } => ("daily_at", None, Some(time.as_str())),
    }
}

/// The intake settings as stored: the server folders live in their own columns.
fn intake_column(settings: &PartnerConnectionSettings) -> Result<Option<String>, PartnerError> {
    settings
        .intake
        .clone()
        .map(|intake| {
            serde_json::to_string(&IntakeSettings {
                remote_processed_folder: None,
                remote_failed_folder: None,
                ..intake
            })
        })
        .transpose()
        .map_err(Into::into)
}

pub(crate) fn insert_connection(
    conn: &Connection,
    row: &ConnectionRow,
) -> Result<(), PartnerError> {
    write_connection(
        conn,
        &format!(
            "INSERT INTO ifc_partner_connections ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
             ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, \
             ?24)"
        ),
        row,
    )
}

/// Every column but the ID; the message type cannot change, the protocol can.
pub(crate) fn update_connection(
    conn: &Connection,
    row: &ConnectionRow,
) -> Result<(), PartnerError> {
    write_connection(
        conn,
        "UPDATE ifc_partner_connections SET name = ?2, partner = ?3, message_type = ?4, \
         protocol = ?5, folder = ?6, folder_set_by = ?7, rules = ?8, schedule = ?9, \
         schedule_minutes = ?10, schedule_time = ?11, file_name_pattern = ?12, active = ?13, \
         revision = ?14, sftp_host = ?15, sftp_port = ?16, sftp_username = ?17, \
         sftp_auth = ?18, sftp_fingerprint = ?19, sftp_password = ?20, destination = ?21, \
         intake = ?22, processed_folder = ?23, failed_folder = ?24 WHERE id = ?1",
        row,
    )
}

/// Runs `sql` with the row's values as `?1`–`?24`, in [`COLUMNS`] order.
fn write_connection(conn: &Connection, sql: &str, row: &ConnectionRow) -> Result<(), PartnerError> {
    let (schedule, minutes, time) = schedule_columns(&row.settings.schedule);
    let (host, port, username, auth, fingerprint) = sftp_columns(&row.settings);
    let moved_to = row.moved_to.as_ref();
    conn.execute(
        sql,
        params![
            row.id,
            row.settings.name,
            row.settings.partner,
            row.settings.message_type,
            row.settings.protocol,
            row.folder,
            row.folder_set_by,
            serde_json::to_string(&row.settings.rules)?,
            schedule,
            minutes,
            time,
            row.settings.file_name_pattern,
            row.active,
            row.revision,
            host,
            port,
            username,
            auth,
            fingerprint,
            row.sealed_password,
            row.destination,
            intake_column(&row.settings)?,
            moved_to.map(|moved_to| &moved_to.processed),
            moved_to.map(|moved_to| &moved_to.failed)
        ],
    )?;
    Ok(())
}

pub(crate) fn load_connection(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<Option<ConnectionRow>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ifc_partner_connections WHERE id = ?1"
    ))?
    .query_row([id], connection_row)
    .optional()?
    .map(decoded)
    .transpose()
}

/// Every connection by name, or those carrying `message_type`.
pub(crate) fn connections(
    conn: &Connection,
    message_type: Option<PartnerMessageType>,
) -> Result<Vec<ConnectionRow>, PartnerError> {
    conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM ifc_partner_connections \
         WHERE ?1 IS NULL OR message_type = ?1 ORDER BY name COLLATE NOCASE"
    ))?
    .query_map([message_type], connection_row)?
    .map(|row| decoded(row?))
    .collect()
}
