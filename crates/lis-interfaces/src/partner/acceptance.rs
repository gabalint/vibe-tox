//! Whether a connection's acceptance tests cover it as it is now (ticket #4555 step 5a;
//! REQ-INT-001). A run covers one acceptance key: the approved contract version, the code
//! mapping version and the settings that shape its messages. Approving a contract or a
//! mapping version, or changing those settings, makes a new key, so the tests run again
//! before the connection writes or reads patient data. Its destination and schedule are not
//! part of the key: the destination check covers the destination.

use lis_audit::AuditDetail;
use lis_core::{Capability, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::connections::{event, person};
use super::contract_model::{AcceptanceCase, AcceptanceRun, AcceptanceState, ContractSummary};
use super::error::PartnerError;
use super::mapping_repo;
use super::model::{PartnerConnectionId, PartnerConnectionSettings};
use super::repo::{ConnectionRow, load_connection};
use crate::model::ChangeStamp;

/// SQL that is true when the latest run for connection `c`'s current key passed.
pub(crate) const ACCEPTED: &str = "COALESCE((SELECT a.passed FROM ifc_partner_acceptance_runs a \
     WHERE a.connection_id = c.id AND a.covers = c.acceptance_key \
     ORDER BY a.ran_at DESC, a.rowid DESC LIMIT 1), 0)";

/// The connection's acceptance key, from what is stored now; written with its row.
pub(crate) fn acceptance_key(
    conn: &Connection,
    row: &ConnectionRow,
) -> Result<String, PartnerError> {
    let contract: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM ifc_partner_contracts WHERE connection_id = ?1",
        [row.id],
        |found| found.get(0),
    )?;
    let mappings =
        mapping_repo::current_version(conn, row.id)?.map_or(0, |(version, _, _)| version);
    Ok(format!(
        "contract {contract}; mappings {mappings}; settings {}",
        settings_digest(&row.settings)?
    ))
}

/// The settings that shape the messages: who they are for, how they travel, how files are
/// named, what is accepted and how it is acknowledged.
fn settings_digest(settings: &PartnerConnectionSettings) -> Result<String, PartnerError> {
    let shape = serde_json::json!({
        "partner": settings.partner,
        "messageType": settings.message_type,
        "protocol": settings.protocol,
        "fileNamePattern": digest_pattern(&settings.file_name_pattern),
        "acceptFrom": settings.intake.as_ref().map(|intake| &intake.accept_from),
        "unknownPatient": settings.intake.as_ref().map(|intake| intake.unknown_patient),
        "acknowledgement": settings.mllp.as_ref().map(|mllp| mllp.acknowledgement),
        "listener": settings.mllp_listener.as_ref()
            .map(|listener| (listener.acknowledgement, listener.character_set)),
    });
    let digest = Sha256::digest(serde_json::to_vec(&shape)?);
    Ok(hex::encode(&digest[..8]))
}

/// The pattern as the key reads it. Before migration 0017 `{accession}` stood for the
/// report number, now `{report}`: reading `{report}` as `{accession}`, and the accession
/// as a name no pattern can hold, keeps a migrated connection's key and tells every
/// pattern apart.
fn digest_pattern(pattern: &str) -> String {
    pattern
        .replace("{accession}", "{accession_number}")
        .replace("{report}", "{accession}")
}

/// The connection's stored acceptance key.
pub fn current_acceptance_key(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<String, PartnerError> {
    conn.query_row(
        "SELECT acceptance_key FROM ifc_partner_connections WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(PartnerError::NotFound("connection"))
}

/// Whether the latest run for the connection's current key passed.
pub(crate) fn accepted(conn: &Connection, id: PartnerConnectionId) -> Result<bool, PartnerError> {
    Ok(conn.query_row(
        &format!("SELECT {ACCEPTED} FROM ifc_partner_connections c WHERE c.id = ?1"),
        [id],
        |row| row.get(0),
    )?)
}

/// The latest run and the state it gives the connection as it is now.
pub(crate) fn acceptance_of(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<(AcceptanceState, Option<AcceptanceRun>), PartnerError> {
    let key = current_acceptance_key(conn, id)?;
    let latest = conn
        .query_row(
            "SELECT covers, passed, results, ran_by, ran_at FROM ifc_partner_acceptance_runs \
             WHERE connection_id = ?1 ORDER BY ran_at DESC, rowid DESC LIMIT 1",
            [id],
            |row| {
                let covers: String = row.get(0)?;
                let results: String = row.get(2)?;
                let ran = ChangeStamp {
                    by: row.get(3)?,
                    by_display: None,
                    at: row.get(4)?,
                };
                Ok((covers, row.get::<_, bool>(1)?, results, ran))
            },
        )
        .optional()?;
    let Some((covers, passed, results, ran)) = latest else {
        return Ok((AcceptanceState::NotRun, None));
    };
    let state = match (covers == key, passed) {
        (false, _) => AcceptanceState::OutOfDate,
        (true, true) => AcceptanceState::Passed,
        (true, false) => AcceptanceState::Failed,
    };
    let run = AcceptanceRun {
        ran,
        passed,
        cases: serde_json::from_str(&results)?,
    };
    Ok((state, Some(run)))
}

/// What the Connections list and Overview show.
pub(crate) fn contract_summary(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<ContractSummary, PartnerError> {
    let version: Option<i64> = conn.query_row(
        "SELECT MAX(version) FROM ifc_partner_contracts WHERE connection_id = ?1",
        [id],
        |row| row.get(0),
    )?;
    let (acceptance, last_run) = acceptance_of(conn, id)?;
    Ok(ContractSummary {
        version,
        acceptance,
        tested_at: last_run.map(|run| run.ran.at),
    })
}

/// Records a run of the acceptance tests the engine made for the connection as `covers`
/// names it. Refused before a contract is approved, and when the connection changed while
/// the tests ran.
pub fn record_acceptance_run(
    ctx: &mut CommandContext<'_>,
    connection_id: PartnerConnectionId,
    covers: &str,
    cases: Vec<AcceptanceCase>,
) -> Result<bool, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let row =
        load_connection(ctx.tx(), connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    if contract_summary(ctx.tx(), row.id)?.version.is_none() {
        return Err(PartnerError::WrongState(
            "Approve the interface contract before running its acceptance tests.",
        ));
    }
    if current_acceptance_key(ctx.tx(), row.id)? != covers {
        return Err(PartnerError::WrongState(
            "The connection changed while its tests ran. Run them again.",
        ));
    }
    let passed = !cases.is_empty() && cases.iter().all(|case| case.passed);
    let failed = cases.iter().filter(|case| !case.passed).count();
    insert_run(ctx.tx(), &row, covers, passed, &cases, (&actor, ctx.now()))?;
    ctx.audit(
        event("interfaces.run_acceptance_tests", &row)
            .detail("passed", AuditDetail::boolean(passed))
            .detail("cases", AuditDetail::count(cases.len()))
            .detail("failed", AuditDetail::count(failed)),
    )?;
    Ok(passed)
}

fn insert_run(
    conn: &Connection,
    row: &ConnectionRow,
    covers: &str,
    passed: bool,
    cases: &[AcceptanceCase],
    (ran_by, ran_at): (&str, UtcTimestamp),
) -> Result<(), PartnerError> {
    conn.execute(
        "INSERT INTO ifc_partner_acceptance_runs \
         (id, connection_id, covers, passed, results, ran_by, ran_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            lis_core::Id::<RunMarker>::generate(),
            row.id,
            covers,
            passed,
            serde_json::to_string(cases)?,
            ran_by,
            ran_at
        ],
    )?;
    Ok(())
}

/// Marker for an acceptance run's ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMarker {}

#[cfg(test)]
mod tests {
    use super::digest_pattern;

    #[test]
    fn a_migrated_pattern_keeps_its_key_and_a_new_accession_does_not_share_it() {
        assert_eq!(
            digest_pattern("{report}_r{revision}.pdf"),
            "{accession}_r{revision}.pdf"
        );
        assert_ne!(
            digest_pattern("{accession}-{report}_r{revision}.pdf"),
            digest_pattern("{report}-{report}_r{revision}.pdf")
        );
    }
}
