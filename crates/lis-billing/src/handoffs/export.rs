//! Exporting a stored handoff file: what the export reads, and its security-stream record.
//! A billing handoff export is not a report delivery.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension};

use crate::error::BillingError;
use crate::handoff_list::handoff_accession_label;
use crate::mapping_support::acting_user;
use crate::model::BillingHandoffId;

/// The stored file a handoff export reads, and its file name stem, which holds
/// no demographics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffExportSource {
    /// The digest of the stored bytes.
    pub artifact_sha256: String,
    /// `bh-<handoff ID>_<accession>` (#4742): `bh` labels a billing handoff;
    /// the accession part is [`handoff_accession_label`], left out when the
    /// handoff names no accession.
    pub file_stem: String,
}

/// What `export_billing_handoff` reads.
pub fn handoff_export_source(
    conn: &Connection,
    handoff_id: BillingHandoffId,
) -> Result<HandoffExportSource, BillingError> {
    let artifact_sha256 = conn
        .query_row(
            "SELECT artifact_sha256 FROM bil_handoffs WHERE id = ?1",
            [handoff_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(BillingError::NotFound("billing handoff"))?;
    let file_stem = match handoff_accession_label(conn, handoff_id)? {
        Some(accession) => format!("bh-{handoff_id}_{accession}"),
        None => format!("bh-{handoff_id}"),
    };
    Ok(HandoffExportSource {
        artifact_sha256,
        file_stem,
    })
}

/// Records an export of a handoff file the engine wrote, on the security
/// stream. A billing handoff export is not a report delivery.
pub fn record_handoff_export(
    ctx: &mut CommandContext<'_>,
    handoff_id: BillingHandoffId,
    source: &HandoffExportSource,
) -> Result<(), BillingError> {
    ctx.require(Capability::Billing)?;
    acting_user(ctx.principal())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            "billing.export_handoff",
            Outcome::Succeeded,
        )
        .record("billing_handoff", handoff_id)
        .detail(
            "artifact_sha256",
            AuditDetail::sha256(&source.artifact_sha256)?,
        ),
    )?;
    Ok(())
}
