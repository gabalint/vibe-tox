//! Update records (REQ-OPS-006, NFR-24; M4 design brief D13, §2.3, §7): approvals with a
//! retained verified backup, withdrawal before installation, acceptance for clinical
//! use, and the version history computed from app runs. There is no updater yet (Q-19):
//! these records document the laboratory's control, and health raises an alert when a
//! version starts without approval.
//!
//! A version is *installed* once a run of it starts after its approval. The first
//! version ever run needs acceptance only. Fresh authentication and the package digest
//! come from the engine.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, Id, UtcTimestamp, required_text};
use lis_store::{CommandContext, StoreError};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::backup::RunId;
use crate::policy::NOTE_MAX_CHARS;
use crate::runs::Stamp;
use crate::runtime_error::RuntimeError;

mod backup;

use backup::ensure_pre_update_backup;
pub use backup::update_approval_eligible;

/// Audit action of an update approval.
pub const APPROVE_UPDATE: &str = "ops.approve_update";
/// Audit action of an approval withdrawal.
pub const WITHDRAW_UPDATE_APPROVAL: &str = "ops.withdraw_update_approval";
/// Audit action of a version acceptance.
pub const ACCEPT_VERSION: &str = "ops.accept_version";

/// Marker for update approval identifiers.
pub enum UpdateApproval {}
/// Identifies one update approval.
pub type UpdateApprovalId = Id<UpdateApproval>;

/// An approval as the engine records it: the screen's input plus the package the
/// engine read from the chosen file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUpdateApproval {
    /// The version to install, as semantic version text.
    pub version: String,
    /// The vendor's risk class, as published.
    pub risk_class_text: String,
    /// The approver confirmed reading the release notes.
    pub release_notes_reviewed: bool,
    /// The retained verified manual or pre-update backup.
    pub pre_update_run_id: RunId,
    /// Approval notes, recorded as the audit reason.
    pub notes: String,
    /// The package's file name (not its path).
    pub package_file_name: String,
    /// The package's SHA-256, lowercase hex, computed by the engine.
    pub package_sha256: String,
}

/// Input of `withdraw_update_approval`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct WithdrawUpdateApprovalInput {
    /// The approval.
    pub approval_id: UpdateApprovalId,
    /// The revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why.
    pub reason: String,
}

/// An approval as the updates screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateApprovalView {
    /// The approval.
    pub approval_id: UpdateApprovalId,
    /// The approved version.
    pub version: String,
    /// Who approved (principal ID text).
    pub approved_by: String,
    /// When.
    pub approved_at: UtcTimestamp,
    /// The package's file name.
    pub package_file_name: String,
    /// The package's SHA-256.
    pub package_sha256: String,
    /// The vendor's risk class, as published.
    pub risk_class_text: String,
    /// The retained verified manual or pre-update backup.
    pub pre_update_run_id: RunId,
    /// Who withdrew it, and when.
    pub withdrawn: Option<Stamp>,
    /// A run of this version started after the approval.
    pub installed: bool,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// The status of an installed version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VersionStatus {
    /// Accepted for clinical use.
    Accepted,
    /// Running, started after its approval (or the first version), not yet accepted.
    AwaitingAcceptance,
    /// Running without an approval, or approved only after it started.
    Unapproved,
    /// No longer running and never accepted.
    Previous,
}

/// One version that has run, as the updates screen shows it (`list_installed_versions`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledVersionView {
    /// The version.
    pub version: String,
    /// Its first start.
    pub first_started_at: UtcTimestamp,
    /// Its newest start.
    pub last_started_at: UtcTimestamp,
    /// It is the version of the newest run.
    pub running: bool,
    /// Its current (not withdrawn) approval.
    pub approval: Option<UpdateApprovalView>,
    /// Who accepted it for clinical use, and when.
    pub acceptance: Option<Stamp>,
    /// Its status.
    pub status: VersionStatus,
}

/// Version facts for health: the running version's approval and acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionFacts {
    /// The running version started without an approval and is not accepted.
    pub started_without_approval: bool,
    /// The running version is not yet accepted.
    pub awaiting_acceptance: bool,
}

/// True for `MAJOR.MINOR.PATCH`, optionally with `-pre.release` and `+build`
/// identifiers, as Semantic Versioning 2.0.0 defines them.
fn is_semver(text: &str) -> bool {
    let (rest, build) = text
        .split_once('+')
        .map_or((text, None), |(r, b)| (r, Some(b)));
    let (core, pre) = rest
        .split_once('-')
        .map_or((rest, None), |(c, p)| (c, Some(p)));
    let numeric = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let identifier = |part: &str| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let core_parts: Vec<&str> = core.split('.').collect();
    core_parts.len() == 3
        && core_parts.iter().all(|part| numeric(part))
        && pre.is_none_or(|pre| {
            pre.split('.').all(|part| {
                identifier(part) && (!part.bytes().all(|b| b.is_ascii_digit()) || numeric(part))
            })
        })
        && build.is_none_or(|build| build.split('.').all(identifier))
}

fn field_error(field: &str, code: &str, message: &str) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// Validated approval text fields: version, risk class and notes.
fn validate(approval: &NewUpdateApproval) -> Result<(String, String, String), RuntimeError> {
    let mut errors = Vec::new();
    if approval.version.len() > 64 || !is_semver(&approval.version) {
        errors.push(field_error(
            "version",
            "invalid",
            "Enter a version such as 1.4.0.",
        ));
    }
    if !approval.release_notes_reviewed {
        errors.push(field_error(
            "releaseNotesReviewed",
            "required",
            "Confirm that you reviewed the release notes.",
        ));
    }
    let digest_ok = approval.package_sha256.len() == 64
        && approval
            .package_sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !digest_ok || required_text("packageFileName", &approval.package_file_name, 255).is_err() {
        errors.push(field_error(
            "package",
            "invalid",
            "Choose the update package again.",
        ));
    }
    let risk = required_text("riskClassText", &approval.risk_class_text, 200)
        .map_err(|error| errors.push(error))
        .ok();
    let notes = required_text("notes", &approval.notes, NOTE_MAX_CHARS)
        .map_err(|error| errors.push(error))
        .ok();
    match (risk, notes) {
        (Some(risk), Some(notes)) if errors.is_empty() => {
            Ok((approval.version.clone(), risk, notes))
        }
        _ => Err(RuntimeError::Invalid(errors)),
    }
}

/// Records an update approval (`manage_configuration`; fresh authentication by the
/// engine). The version must be semantic version text without a current approval, and
/// the manual or pre-update backup retained, verified and recent. Audited as `ops.approve_update`.
pub fn record_update_approval(
    ctx: &mut CommandContext<'_>,
    approval: &NewUpdateApproval,
) -> Result<UpdateApprovalView, RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    let (version, risk, notes) = validate(approval)?;
    if current_approval(ctx.tx(), &version)?.is_some() {
        return Err(RuntimeError::VersionAlreadyApproved);
    }
    ensure_pre_update_backup(ctx.tx(), approval.pre_update_run_id, ctx.now())?;
    let id = UpdateApprovalId::generate();
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_update_approvals (id, version, package_file_name, package_sha256, \
             risk_class_text, pre_update_run_id, approved_by, approved_at, revision) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
        )?
        .execute(params![
            id,
            version,
            approval.package_file_name.trim(),
            approval.package_sha256,
            risk,
            approval.pre_update_run_id,
            ctx.principal().actor().id_text(),
            ctx.now(),
        ])?;
    let digest = AuditDetail::sha256(&approval.package_sha256).map_err(StoreError::from)?;
    ctx.audit(
        AuditEvent::new(Stream::Security, APPROVE_UPDATE, Outcome::Succeeded)
            .record("update_approval", id)
            .new_revision(1)
            .detail(
                "pre_update_run_id",
                AuditDetail::id(approval.pre_update_run_id),
            )
            .detail("package_sha256", digest)
            .detail("release_notes_reviewed", AuditDetail::boolean(true))
            .reason(notes),
    )?;
    load_approval(ctx.tx(), id)
}

/// Withdraws an approval before its version is installed (`manage_configuration`).
/// Audited as `ops.withdraw_update_approval` with the reason.
pub fn withdraw_update_approval(
    ctx: &mut CommandContext<'_>,
    input: WithdrawUpdateApprovalInput,
) -> Result<UpdateApprovalView, RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    let reason = required_text("reason", &input.reason, NOTE_MAX_CHARS)?;
    let approval = load_approval(ctx.tx(), input.approval_id)?;
    if approval.withdrawn.is_some() {
        return Err(RuntimeError::ApprovalWithdrawn);
    }
    if approval.revision != input.expected_revision {
        return Err(RuntimeError::RevisionConflict("update approval"));
    }
    if approval.installed {
        return Err(RuntimeError::ApprovalInstalled);
    }
    ctx.tx()
        .prepare_cached(
            "UPDATE ops_update_approvals SET withdrawn_by = ?2, withdrawn_at = ?3, \
             withdraw_reason = ?4, revision = revision + 1 WHERE id = ?1",
        )?
        .execute(params![
            input.approval_id,
            ctx.principal().actor().id_text(),
            ctx.now(),
            reason
        ])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            WITHDRAW_UPDATE_APPROVAL,
            Outcome::Succeeded,
        )
        .record("update_approval", input.approval_id)
        .old_revision(approval.revision)
        .new_revision(approval.revision + 1)
        .reason(reason),
    )?;
    load_approval(ctx.tx(), input.approval_id)
}

const APPROVAL_COLUMNS: &str = "a.id, a.version, a.approved_by, a.approved_at, \
    a.package_file_name, a.package_sha256, a.risk_class_text, a.pre_update_run_id, \
    a.withdrawn_by, a.withdrawn_at, a.revision, EXISTS (SELECT 1 FROM ops_app_runs r \
    WHERE r.app_version = a.version AND r.started_at >= a.approved_at)";

fn approval_view(row: &Row<'_>) -> rusqlite::Result<UpdateApprovalView> {
    let withdrawn_by: Option<String> = row.get(8)?;
    let withdrawn_at: Option<UtcTimestamp> = row.get(9)?;
    Ok(UpdateApprovalView {
        approval_id: row.get(0)?,
        version: row.get(1)?,
        approved_by: row.get(2)?,
        approved_at: row.get(3)?,
        package_file_name: row.get(4)?,
        package_sha256: row.get(5)?,
        risk_class_text: row.get(6)?,
        pre_update_run_id: row.get(7)?,
        withdrawn: withdrawn_by
            .zip(withdrawn_at)
            .map(|(by, at)| Stamp { by, at }),
        revision: row.get(10)?,
        installed: row.get(11)?,
    })
}

fn load_approval(
    conn: &Connection,
    id: UpdateApprovalId,
) -> Result<UpdateApprovalView, RuntimeError> {
    let sql = format!("SELECT {APPROVAL_COLUMNS} FROM ops_update_approvals a WHERE a.id = ?1");
    conn.prepare_cached(&sql)?
        .query_row([id], approval_view)
        .optional()?
        .ok_or(RuntimeError::NotFound("update approval"))
}

/// The approval of `version` that is not withdrawn, if any.
fn current_approval(
    conn: &Connection,
    version: &str,
) -> Result<Option<UpdateApprovalView>, RuntimeError> {
    let sql = format!(
        "SELECT {APPROVAL_COLUMNS} FROM ops_update_approvals a \
         WHERE a.version = ?1 AND a.withdrawn_at IS NULL"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([version], approval_view)
        .optional()?)
}

/// Every approval, newest first, including withdrawn ones; pending approvals carry the
/// revision the screen needs to offer withdrawal.
pub fn list_update_approvals(conn: &Connection) -> Result<Vec<UpdateApprovalView>, RuntimeError> {
    let sql = format!(
        "SELECT {APPROVAL_COLUMNS} FROM ops_update_approvals a ORDER BY a.approved_at DESC, a.id"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_map([], approval_view)?
        .collect::<rusqlite::Result<_>>()?)
}

mod versions;

pub use versions::{
    AcceptVersionInput, accept_installed_version, list_installed_versions, version_facts,
};
