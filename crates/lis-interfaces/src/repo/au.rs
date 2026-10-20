//! Immutable AU wire-profile evidence, separate from current connection settings.

use rusqlite::{Connection, OptionalExtension, params};

use crate::{InterfaceError, ReceiptId};

pub(crate) fn insert_au_profile(
    conn: &Connection,
    receipt: ReceiptId,
    profile: &lis_au::AuConfig,
) -> Result<(), InterfaceError> {
    let json =
        serde_json::to_string(profile).map_err(|_| InterfaceError::InvalidAuConfiguration)?;
    conn.prepare_cached(
        "INSERT INTO ifc_receipt_au_profiles (receipt_id, profile_json, profile_version) VALUES (?, ?, 1)",
    )?.execute(params![receipt, json])?;
    Ok(())
}

pub(crate) fn au_profile(
    conn: &Connection,
    receipt: ReceiptId,
) -> Result<Option<lis_au::AuConfig>, InterfaceError> {
    let json: Option<String> = conn.prepare_cached(
        "SELECT profile_json FROM ifc_receipt_au_profiles WHERE receipt_id = ? AND profile_version = 1",
    )?.query_row([receipt], |row| row.get(0)).optional()?;
    json.map(|json| {
        let profile: lis_au::AuConfig =
            serde_json::from_str(&json).map_err(|_| InterfaceError::InvalidAuConfiguration)?;
        profile
            .validate()
            .map_err(|_| InterfaceError::InvalidAuConfiguration)?;
        Ok(profile)
    })
    .transpose()
}
