//! Disposal and re-use records for host disks and backup media (REQ-OPS-003): what
//! happened to a medium that held laboratory data, who recorded it and when. Append-only;
//! the note is the audit reason of `ops.record_media_disposal` (security stream). The
//! medium description and identifier are the laboratory's PHI-free text and never go into
//! audit details.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, UtcTimestamp, optional_text, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::policy::NOTE_MAX_CHARS;
use crate::runtime_error::RuntimeError;

/// Audit action of a disposal record.
pub const RECORD_MEDIA_DISPOSAL: &str = "ops.record_media_disposal";

/// Marker for disposal identifiers.
pub enum MediaDisposal {}
/// Identifies one disposal record.
pub type MediaDisposalId = Id<MediaDisposal>;

lis_core::text_enum! {
    /// What was done with the medium.
    #[ts(export)]
    DisposalMethod {
        /// Securely erased and kept.
        Wiped = "wiped",
        /// Physically destroyed.
        Destroyed = "destroyed",
        /// Erased, then put to another use.
        ReusedAfterWipe = "reused_after_wipe",
        /// Returned to its owner or vendor.
        Returned = "returned",
    }
}

/// Input of `record_media_disposal`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordMediaDisposalInput {
    /// The medium, such as "Backup drive B (USB)"; no patient data.
    pub medium: String,
    /// What was done with it.
    pub method: DisposalMethod,
    /// A serial or asset number, if any.
    pub identifier: Option<String>,
    /// How and by whom it was done, or the certificate reference.
    pub note: String,
}

/// One disposal record, as the disposals screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaDisposalView {
    /// The record.
    pub disposal_id: MediaDisposalId,
    /// The medium.
    pub medium: String,
    /// What was done with it.
    pub method: DisposalMethod,
    /// Its serial or asset number.
    pub identifier: Option<String>,
    /// Who recorded it (principal ID text).
    pub recorded_by: String,
    /// When.
    pub recorded_at: UtcTimestamp,
}

/// Records a disposal (`backup_restore`).
pub fn record_media_disposal(
    ctx: &mut CommandContext<'_>,
    input: RecordMediaDisposalInput,
) -> Result<MediaDisposalView, RuntimeError> {
    ctx.require(Capability::BackupRestore)?;
    let medium = required_text("medium", &input.medium, 200);
    let identifier = optional_text("identifier", input.identifier.as_deref(), 100);
    let note = required_text("note", &input.note, NOTE_MAX_CHARS);
    let (medium, identifier, note) = match (medium, identifier, note) {
        (Ok(medium), Ok(identifier), Ok(note)) => (medium, identifier, note),
        (medium, identifier, note) => {
            let errors = [medium.err(), identifier.err(), note.err()]
                .into_iter()
                .flatten()
                .collect();
            return Err(RuntimeError::Invalid(errors));
        }
    };
    let view = MediaDisposalView {
        disposal_id: MediaDisposalId::generate(),
        medium,
        method: input.method,
        identifier,
        recorded_by: ctx.principal().actor().id_text(),
        recorded_at: ctx.now(),
    };
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_media_disposals (id, medium, method, identifier, recorded_by, \
             recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?
        .execute(params![
            view.disposal_id,
            view.medium,
            view.method,
            view.identifier,
            view.recorded_by,
            view.recorded_at
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, RECORD_MEDIA_DISPOSAL, Outcome::Succeeded)
            .record("media_disposal", view.disposal_id)
            .detail("method", AuditDetail::code(view.method.as_str()))
            .detail(
                "identifier_recorded",
                AuditDetail::boolean(view.identifier.is_some()),
            )
            .reason(note),
    )?;
    Ok(view)
}

/// Every disposal record, newest first.
pub fn list_media_disposals(conn: &Connection) -> Result<Vec<MediaDisposalView>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT id, medium, method, identifier, recorded_by, recorded_at \
             FROM ops_media_disposals ORDER BY recorded_at DESC, id DESC",
        )?
        .query_map([], |row| {
            Ok(MediaDisposalView {
                disposal_id: row.get(0)?,
                medium: row.get(1)?,
                method: row.get(2)?,
                identifier: row.get(3)?,
                recorded_by: row.get(4)?,
                recorded_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
