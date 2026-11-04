//! SQL for connections, their history and destination checks.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};
use sha2::{Digest, Sha256};

use super::error::PartnerError;
use super::model::{
    ConnectionChange, PartnerConnectionId, PartnerConnectionSettings, PartnerMessageType, Schedule,
    SftpAuth, SftpSettings,
};

/// A connection as stored.
#[derive(Debug, Clone)]
pub(crate) struct ConnectionRow {
    pub(crate) id: PartnerConnectionId,
    pub(crate) settings: PartnerConnectionSettings,
    /// The local folder, or the remote folder (also in `settings.sftp`).
    pub(crate) folder: String,
    /// Who chose the current destination.
    pub(crate) folder_set_by: String,
    /// The SFTP password, sealed with the installation key.
    pub(crate) sealed_password: Option<Vec<u8>>,
    /// What a destination check covers: the folder, or the server, user, port, remote
    /// folder and fingerprint ([`destination_of`](super::settings::destination_of)).
    pub(crate) destination: String,
    pub(crate) active: bool,
    pub(crate) revision: i64,
}

/// One history entry as stored.
pub(crate) struct ChangeRow {
    pub(crate) revision: i64,
    pub(crate) change: ConnectionChange,
    pub(crate) settings: String,
    pub(crate) reason: Option<String>,
    pub(crate) changed_by: String,
    pub(crate) changed_at: UtcTimestamp,
}

/// The destination check that covers a folder.
pub(crate) struct VerificationRow {
    pub(crate) method: String,
    pub(crate) verified_by: String,
    pub(crate) verified_at: UtcTimestamp,
    pub(crate) self_approval: bool,
}

const COLUMNS: &str = "id, name, partner, message_type, protocol, folder, folder_set_by, rules, \
     schedule, schedule_minutes, schedule_time, file_name_pattern, active, revision, sftp_host, \
     sftp_port, sftp_username, sftp_auth, sftp_fingerprint, sftp_password, destination";

fn connection_row(row: &Row<'_>) -> rusqlite::Result<(ConnectionRow, String)> {
    let schedule = match row.get::<_, String>(8)?.as_str() {
        "every_minutes" => Schedule::EveryMinutes {
            minutes: row.get(9)?,
        },
        "daily_at" => Schedule::DailyAt { time: row.get(10)? },
        _ => Schedule::Immediately,
    };
    let rules: String = row.get(7)?;
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
    Ok((
        ConnectionRow {
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
            },
            folder,
            folder_set_by: row.get(6)?,
            sealed_password: row.get(19)?,
            destination: row.get(20)?,
            active: row.get(12)?,
            revision: row.get(13)?,
        },
        rules,
    ))
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

fn with_rules((mut row, rules): (ConnectionRow, String)) -> Result<ConnectionRow, PartnerError> {
    row.settings.rules = serde_json::from_str(&rules)?;
    Ok(row)
}

fn schedule_columns(schedule: &Schedule) -> (&'static str, Option<u16>, Option<&str>) {
    match schedule {
        Schedule::Immediately => ("immediately", None, None),
        Schedule::EveryMinutes { minutes } => ("every_minutes", Some(*minutes), None),
        Schedule::DailyAt { time } => ("daily_at", None, Some(time.as_str())),
    }
}

pub(crate) fn insert_connection(
    conn: &Connection,
    row: &ConnectionRow,
) -> Result<(), PartnerError> {
    let (schedule, minutes, time) = schedule_columns(&row.settings.schedule);
    let (host, port, username, auth, fingerprint) = sftp_columns(&row.settings);
    conn.execute(
        &format!(
            "INSERT INTO ifc_partner_connections ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
             ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"
        ),
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
            row.destination
        ],
    )?;
    Ok(())
}

pub(crate) fn update_connection(
    conn: &Connection,
    row: &ConnectionRow,
) -> Result<(), PartnerError> {
    let (schedule, minutes, time) = schedule_columns(&row.settings.schedule);
    let (host, port, username, auth, fingerprint) = sftp_columns(&row.settings);
    conn.execute(
        "UPDATE ifc_partner_connections SET name = ?2, partner = ?3, folder = ?4, \
         folder_set_by = ?5, rules = ?6, schedule = ?7, schedule_minutes = ?8, \
         schedule_time = ?9, file_name_pattern = ?10, active = ?11, revision = ?12, \
         sftp_host = ?13, sftp_port = ?14, sftp_username = ?15, sftp_auth = ?16, \
         sftp_fingerprint = ?17, sftp_password = ?18, destination = ?19 WHERE id = ?1",
        params![
            row.id,
            row.settings.name,
            row.settings.partner,
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
            row.destination
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
    .map(with_rules)
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
    .map(|row| with_rules(row?))
    .collect()
}

pub(crate) fn name_in_use(
    conn: &Connection,
    name: &str,
    except: Option<PartnerConnectionId>,
) -> Result<bool, PartnerError> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_partner_connections \
         WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2))",
        params![name, except],
        |row| row.get(0),
    )?)
}

pub(crate) fn insert_change(
    conn: &Connection,
    id: PartnerConnectionId,
    change: &ChangeRow,
) -> Result<(), PartnerError> {
    conn.execute(
        "INSERT INTO ifc_partner_connection_changes \
         (connection_id, revision, change, settings, reason, changed_by, changed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            change.revision,
            change.change,
            change.settings,
            change.reason,
            change.changed_by,
            change.changed_at
        ],
    )?;
    Ok(())
}

/// The history, newest first.
pub(crate) fn changes(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<Vec<ChangeRow>, PartnerError> {
    conn.prepare_cached(
        "SELECT revision, change, settings, reason, changed_by, changed_at \
         FROM ifc_partner_connection_changes WHERE connection_id = ?1 ORDER BY revision DESC",
    )?
    .query_map([id], |row| {
        Ok(ChangeRow {
            revision: row.get(0)?,
            change: row.get(1)?,
            settings: row.get(2)?,
            reason: row.get(3)?,
            changed_by: row.get(4)?,
            changed_at: row.get(5)?,
        })
    })?
    .collect::<rusqlite::Result<_>>()
    .map_err(Into::into)
}

/// Who changed the connection last, and when.
pub(crate) fn last_change(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<(String, UtcTimestamp), PartnerError> {
    Ok(conn.query_row(
        "SELECT changed_by, changed_at FROM ifc_partner_connection_changes \
         WHERE connection_id = ?1 ORDER BY revision DESC LIMIT 1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

pub(crate) fn insert_verification(
    conn: &Connection,
    id: PartnerConnectionId,
    folder: &str,
    verification: &VerificationRow,
) -> Result<(), PartnerError> {
    conn.execute(
        "INSERT INTO ifc_partner_verifications \
         (connection_id, folder, method, verified_by, verified_at, self_approval) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            folder,
            verification.method,
            verification.verified_by,
            verification.verified_at,
            verification.self_approval
        ],
    )?;
    Ok(())
}

/// The latest check of `folder` on the connection.
pub(crate) fn verification(
    conn: &Connection,
    id: PartnerConnectionId,
    folder: &str,
) -> Result<Option<VerificationRow>, PartnerError> {
    conn.prepare_cached(
        "SELECT method, verified_by, verified_at, self_approval FROM ifc_partner_verifications \
         WHERE connection_id = ?1 AND folder = ?2 ORDER BY verified_at DESC LIMIT 1",
    )?
    .query_row(params![id, folder], |row| {
        Ok(VerificationRow {
            method: row.get(0)?,
            verified_by: row.get(1)?,
            verified_at: row.get(2)?,
            self_approval: row.get(3)?,
        })
    })
    .optional()
    .map_err(Into::into)
}

/// The settings a history entry keeps: everything a person set, plus the
/// folder, whether a password was set (never the password) and whether the
/// connection was started.
pub(crate) fn snapshot(row: &ConnectionRow) -> Result<String, PartnerError> {
    let mut value = serde_json::to_value(&row.settings)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("folder".to_owned(), row.folder.clone().into());
        object.insert("active".to_owned(), row.active.into());
        if let Some(sealed) = &row.sealed_password {
            // Each new password seals differently, so a changed one shows in History.
            object.insert(
                "password".to_owned(),
                hex::encode(Sha256::digest(sealed)).into(),
            );
        }
    }
    Ok(value.to_string())
}
