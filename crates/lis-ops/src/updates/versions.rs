//! The version history computed from app runs, approvals and acceptances (§2.3): a
//! version is approved, then installed when a run of it starts after the approval, then
//! accepted. Problem states: started without approval, and installed but not accepted.
//! Acceptance needs a run started after the approval (NFR-24): an approval recorded after
//! an unapproved start does not regularize that start, so its alert stays until the
//! version has been restarted under the approval and accepted.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use ts_rs::TS;

use super::{ACCEPT_VERSION, InstalledVersionView, VersionFacts, VersionStatus, current_approval};
use crate::policy::NOTE_MAX_CHARS;
use crate::runs::Stamp;
use crate::runtime_error::RuntimeError;

/// Input of `accept_installed_version` (fresh authentication by the engine).
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AcceptVersionInput {
    /// The running version.
    pub version: String,
    /// What was checked before clinical use, recorded as the audit reason.
    pub note: String,
}

/// One version's runs.
struct VersionRuns {
    version: String,
    first_started_at: UtcTimestamp,
    last_started_at: UtcTimestamp,
}

fn version_runs(conn: &Connection) -> Result<Vec<VersionRuns>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT app_version, min(started_at), max(started_at) FROM ops_app_runs \
             GROUP BY app_version ORDER BY min(started_at) DESC",
        )?
        .query_map([], |row| {
            Ok(VersionRuns {
                version: row.get(0)?,
                first_started_at: row.get(1)?,
                last_started_at: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

/// The version of the newest run, and the version of the first run ever.
fn running_and_first(conn: &Connection) -> Result<(Option<String>, Option<String>), RuntimeError> {
    let pick = |order: &str| -> Result<Option<String>, RuntimeError> {
        let sql = format!(
            "SELECT app_version FROM ops_app_runs ORDER BY started_at {order}, id {order} LIMIT 1"
        );
        Ok(conn
            .prepare_cached(&sql)?
            .query_row([], |row| row.get(0))
            .optional()?)
    };
    Ok((pick("DESC")?, pick("ASC")?))
}

fn acceptance(conn: &Connection, version: &str) -> Result<Option<Stamp>, RuntimeError> {
    Ok(conn
        .prepare_cached(
            "SELECT accepted_by, accepted_at FROM ops_version_acceptances WHERE version = ?1",
        )?
        .query_row([version], |row| {
            Ok(Stamp {
                by: row.get(0)?,
                by_display: None,
                at: row.get(1)?,
            })
        })
        .optional()?)
}

/// True when no current approval predates the version's first start (NFR-24). The
/// first version ever run needs no approval.
fn started_without_approval(
    conn: &Connection,
    runs: &VersionRuns,
    first: Option<&str>,
) -> Result<bool, RuntimeError> {
    if first == Some(runs.version.as_str()) {
        return Ok(false);
    }
    let approved_before: bool = conn
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM ops_update_approvals WHERE version = ?1 \
             AND withdrawn_at IS NULL AND approved_at <= ?2)",
        )?
        .query_row(params![runs.version, runs.first_started_at], |row| {
            row.get(0)
        })?;
    Ok(!approved_before)
}

fn version_view(
    conn: &Connection,
    runs: VersionRuns,
    running: Option<&str>,
    first: Option<&str>,
) -> Result<InstalledVersionView, RuntimeError> {
    let is_running = running == Some(runs.version.as_str());
    let approval = current_approval(conn, &runs.version)?;
    let acceptance = acceptance(conn, &runs.version)?;
    let status = if acceptance.is_some() {
        VersionStatus::Accepted
    } else if !is_running {
        VersionStatus::Previous
    } else if approval.as_ref().is_some_and(|approval| approval.installed)
        || first == Some(runs.version.as_str())
    {
        VersionStatus::AwaitingAcceptance
    } else {
        VersionStatus::Unapproved
    };
    Ok(InstalledVersionView {
        version: runs.version,
        first_started_at: runs.first_started_at,
        last_started_at: runs.last_started_at,
        running: is_running,
        approval,
        acceptance,
        status,
    })
}

/// Every version that has run, newest first, with its approval, acceptance and status.
pub fn list_installed_versions(
    conn: &Connection,
) -> Result<Vec<InstalledVersionView>, RuntimeError> {
    let (running, first) = running_and_first(conn)?;
    version_runs(conn)?
        .into_iter()
        .map(|runs| version_view(conn, runs, running.as_deref(), first.as_deref()))
        .collect()
}

/// The running version's facts for health: an alert while it started without approval
/// and is not accepted; a warning while it is not accepted (L9: nothing is blocked).
pub fn version_facts(conn: &Connection) -> Result<VersionFacts, RuntimeError> {
    let (running, first) = running_and_first(conn)?;
    let Some(runs) = version_runs(conn)?
        .into_iter()
        .find(|runs| Some(runs.version.as_str()) == running.as_deref())
    else {
        return Ok(VersionFacts {
            started_without_approval: false,
            awaiting_acceptance: false,
        });
    };
    let accepted = acceptance(conn, &runs.version)?.is_some();
    Ok(VersionFacts {
        started_without_approval: !accepted
            && started_without_approval(conn, &runs, first.as_deref())?,
        awaiting_acceptance: !accepted,
    })
}

/// Accepts the running version for clinical use (`approve_configuration`; fresh
/// authentication by the engine). It must be the first version ever run, or have an
/// approval and a run started after it (NFR-24), and not yet be accepted. Audited as `ops.accept_version` with the note as reason.
pub fn accept_installed_version(
    ctx: &mut CommandContext<'_>,
    input: AcceptVersionInput,
) -> Result<InstalledVersionView, RuntimeError> {
    ctx.require(Capability::ApproveConfiguration)?;
    let note = required_text("note", &input.note, NOTE_MAX_CHARS)?;
    let (running, first) = running_and_first(ctx.tx())?;
    if running.as_deref() != Some(input.version.as_str()) {
        return Err(RuntimeError::VersionNotRunning);
    }
    if acceptance(ctx.tx(), &input.version)?.is_some() {
        return Err(RuntimeError::VersionAlreadyAccepted);
    }
    let approval = current_approval(ctx.tx(), &input.version)?;
    let is_first = first.as_deref() == Some(input.version.as_str());
    if !is_first {
        match &approval {
            None => return Err(RuntimeError::VersionNotApproved),
            Some(approval) if !approval.installed => {
                return Err(RuntimeError::VersionNotStartedSinceApproval);
            }
            Some(_) => {}
        }
    }
    let approval_id = approval.as_ref().map(|approval| approval.approval_id);
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_version_acceptances (version, approval_id, accepted_by, accepted_at) \
             VALUES (?1, ?2, ?3, ?4)",
        )?
        .execute(params![
            input.version,
            approval_id,
            ctx.principal().actor().id_text(),
            ctx.now()
        ])?;
    let mut event = AuditEvent::new(Stream::Security, ACCEPT_VERSION, Outcome::Succeeded)
        .record("app_version", &input.version)
        .detail("first_version", AuditDetail::boolean(is_first))
        .reason(note);
    if let Some(approval_id) = approval_id {
        event = event.detail("approval_id", AuditDetail::id(approval_id));
    }
    ctx.audit(event)?;
    list_installed_versions(ctx.tx())?
        .into_iter()
        .find(|view| view.version == input.version)
        .ok_or(RuntimeError::NotFound("version"))
}
