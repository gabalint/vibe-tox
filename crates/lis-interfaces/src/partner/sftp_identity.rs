//! The LIS's own SSH key for signing in to SFTP servers: one per installation, made once.
//! Its public key goes to the partner's IT; its private key is kept sealed with the
//! installation key, by the engine, and never leaves it.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use super::connections::person;
use super::error::PartnerError;

/// The public key in OpenSSH form, such as `ssh-ed25519 AAAA… openbga-tox`, once made.
pub fn sftp_public_key(conn: &Connection) -> Result<Option<String>, PartnerError> {
    conn.query_row("SELECT public_key FROM ifc_sftp_identity", [], |row| {
        row.get(0)
    })
    .optional()
    .map_err(Into::into)
}

/// The sealed private key, for the engine to open when it signs in.
pub fn sealed_sftp_key(conn: &Connection) -> Result<Option<Vec<u8>>, PartnerError> {
    conn.query_row("SELECT private_key FROM ifc_sftp_identity", [], |row| {
        row.get(0)
    })
    .optional()
    .map_err(Into::into)
}

/// Keeps the key the engine made and sealed. Refused once a key exists: partners have
/// installed its public key.
pub fn create_sftp_identity(
    ctx: &mut CommandContext<'_>,
    public_key: &str,
    sealed_private_key: &[u8],
) -> Result<(), PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    if sftp_public_key(ctx.tx())?.is_some() {
        return Err(PartnerError::WrongState(
            "The LIS already has its SFTP key.",
        ));
    }
    ctx.tx().execute(
        "INSERT INTO ifc_sftp_identity (id, public_key, private_key, created_by, created_at) \
         VALUES (1, ?1, ?2, ?3, ?4)",
        params![public_key, sealed_private_key, actor, ctx.now()],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "interfaces.create_sftp_key",
            Outcome::Succeeded,
        )
        .record("sftp_identity", 1),
    )?;
    Ok(())
}
