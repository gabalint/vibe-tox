//! SQL for connections, their history and destination checks.

use lis_core::UtcTimestamp;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

pub(crate) use super::connection_columns::{
    connections, insert_connection, load_connection, update_connection,
};
use super::error::PartnerError;
use super::model::{ConnectionChange, MovedTo, PartnerConnectionId, PartnerConnectionSettings};

/// A connection as stored.
#[derive(Debug, Clone)]
pub(crate) struct ConnectionRow {
    pub(crate) id: PartnerConnectionId,
    pub(crate) settings: PartnerConnectionSettings,
    /// The local folder, or the remote folder (also in `settings.sftp`): where files are
    /// written, or for incoming orders where they are read.
    pub(crate) folder: String,
    /// For incoming orders: where read files move to.
    pub(crate) moved_to: Option<MovedTo>,
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
        if let Some(moved_to) = &row.moved_to {
            object.insert(
                "processedFolder".to_owned(),
                moved_to.processed.clone().into(),
            );
            object.insert("failedFolder".to_owned(), moved_to.failed.clone().into());
        }
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
