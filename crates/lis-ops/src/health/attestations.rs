//! Attestations (M4 design brief §2.4, §6.1): a person with `manage_configuration`
//! attests what the app cannot read (disk encryption, UPS, time synchronization, a web
//! engine smoke check, operating-system support), with a note, for at most
//! [`ATTESTATION_MAX_DAYS`] days. Audited as `ops.attest_check` on the security stream.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, Id, UtcTimestamp, required_text};
use lis_store::{CommandContext, StoreError};
use rusqlite::{Connection, params};
use serde::Deserialize;
use ts_rs::TS;

use super::checks::{AttestationCode, CheckCode};
use super::evaluate::days;
use super::inputs::Attestation;
use crate::policy::{ATTESTATION_MAX_DAYS, NOTE_MAX_CHARS};
use crate::runtime_error::RuntimeError;

/// Audit action of an attestation.
pub const ATTEST_CHECK: &str = "ops.attest_check";

/// Marker for attestation identifiers.
enum AttestationRow {}
type AttestationId = Id<AttestationRow>;

/// Input of `attest_health_check`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AttestCheckInput {
    /// The attestable check.
    pub check: CheckCode,
    /// What the person checked, and how.
    pub note: String,
}

/// Records an attestation valid for [`ATTESTATION_MAX_DAYS`] days from now.
pub fn attest_check(
    ctx: &mut CommandContext<'_>,
    input: AttestCheckInput,
) -> Result<Attestation, RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    let note = required_text("note", &input.note, NOTE_MAX_CHARS);
    let code = input.check.attestation().ok_or_else(|| FieldError {
        field: "check".to_owned(),
        code: "not_attestable".to_owned(),
        message: "This check cannot be attested.".to_owned(),
    });
    let (code, note) = match (code, note) {
        (Ok(code), Ok(note)) => (code, note),
        (code, note) => {
            let errors = [code.err(), note.err()].into_iter().flatten().collect();
            return Err(RuntimeError::Invalid(errors));
        }
    };
    let attestation = Attestation {
        code,
        by: ctx.principal().actor().id_text(),
        by_display: None,
        at: ctx.now(),
        valid_until: ctx
            .now()
            .checked_add(days(ATTESTATION_MAX_DAYS))
            .ok_or(StoreError::TimeOutOfRange)?,
    };
    let id = AttestationId::generate();
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_attestations (id, check_code, attested_by, attested_at, valid_until) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?
        .execute(params![
            id,
            code,
            attestation.by,
            attestation.at,
            attestation.valid_until
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Security, ATTEST_CHECK, Outcome::Succeeded)
            .record("attestation", id)
            .detail("check", AuditDetail::code(input.check.as_str()))
            .detail("attestation", AuditDetail::code(code.as_str()))
            .detail(
                "valid_days",
                AuditDetail::unsigned(ATTESTATION_MAX_DAYS.into()),
            )
            .reason(note),
    )?;
    Ok(attestation)
}

/// The newest attestation of each code that is valid at `now`.
pub fn valid_attestations(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<Vec<Attestation>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT check_code, attested_by, attested_at, valid_until FROM ops_attestations a \
             WHERE attested_at <= ?1 AND valid_until > ?1 AND NOT EXISTS ( \
               SELECT 1 FROM ops_attestations b WHERE b.check_code = a.check_code \
               AND b.attested_at <= ?1 AND b.valid_until > ?1 \
               AND (b.attested_at > a.attested_at OR (b.attested_at = a.attested_at AND b.id > a.id))) \
             ORDER BY check_code",
        )?
        .query_map([now], |row| {
            Ok(Attestation {
                code: row.get::<_, AttestationCode>(0)?,
                by: row.get(1)?,
                by_display: None,
                at: row.get(2)?,
                valid_until: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
