//! Exporting a stored handoff file: what the export reads, and its security-stream record.
//! A billing handoff export is not a report delivery.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension};

use crate::error::BillingError;
use crate::mapping_support::acting_user;
use crate::model::BillingHandoffId;

/// The stored file a handoff export reads, and its PHI-free file name stem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffExportSource {
    /// The digest of the stored bytes.
    pub artifact_sha256: String,
    /// `billing-handoff-<handoff ID>`: labelled as a billing handoff.
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
    Ok(HandoffExportSource {
        artifact_sha256,
        file_stem: format!("billing-handoff-{handoff_id}"),
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
