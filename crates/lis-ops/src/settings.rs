//! Operations settings (REQ-OPS-007, REQ-OPS-008; M4 design brief D12, §8): the backup
//! interval, retention and health thresholds, in versioned append-only rows. Until the
//! laboratory sets values, [`DEFAULT_SETTINGS`] applies (laboratory questions L2–L6, L8).
//! Every version after the first carries a reason; ranges live in [`crate::policy`].

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::policy::{DEFAULT_SETTINGS, NOTE_MAX_CHARS, OperationsSettings};
use crate::runtime_error::RuntimeError;

/// Audit action of a new settings version.
pub const SET_SETTINGS: &str = "ops.set_settings";

/// The settings as the settings screen shows them (`get_operations_settings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OperationsSettingsView {
    /// The version in force; `None` while the defaults apply.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
    /// The values in force.
    pub values: OperationsSettings,
    /// The defaults, for comparison.
    pub defaults: OperationsSettings,
    /// Who set the version in force (principal ID text).
    pub set_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub set_by_display: Option<String>,
    /// When it was set.
    pub set_at: Option<UtcTimestamp>,
}

/// Input of `set_operations_settings`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetOperationsSettingsInput {
    /// The version the user saw; `None` while the defaults applied.
    #[ts(type = "number | null")]
    pub expected_version: Option<i64>,
    /// The complete new values.
    pub values: OperationsSettings,
    /// Why; required once a version exists.
    pub reason: Option<String>,
}

struct StoredSettings {
    version: i64,
    values: OperationsSettings,
    set_by: String,
    set_at: UtcTimestamp,
}

const COLUMNS: &str = "backup_interval_minutes, keep_recent, keep_daily, keep_weekly, \
    backup_warn_minutes, backup_alert_minutes, offhost_warn_hours, offline_warn_days, \
    restore_test_warn_days, kit_confirm_warn_days, anchor_warn_minutes, anchor_incident_hours, \
    disk_free_warn_gib, os_support_warn_days, clock_jump_seconds";

fn stored_settings(row: &Row<'_>) -> rusqlite::Result<StoredSettings> {
    Ok(StoredSettings {
        values: OperationsSettings {
            backup_interval_minutes: row.get(0)?,
            keep_recent: row.get(1)?,
            keep_daily: row.get(2)?,
            keep_weekly: row.get(3)?,
            backup_warn_minutes: row.get(4)?,
            backup_alert_minutes: row.get(5)?,
            offhost_warn_hours: row.get(6)?,
            offline_warn_days: row.get(7)?,
            restore_test_warn_days: row.get(8)?,
            kit_confirm_warn_days: row.get(9)?,
            anchor_warn_minutes: row.get(10)?,
            anchor_incident_hours: row.get(11)?,
            disk_free_warn_gib: row.get(12)?,
            os_support_warn_days: row.get(13)?,
            clock_jump_seconds: row.get(14)?,
        },
        version: row.get(15)?,
        set_by: row.get(16)?,
        set_at: row.get(17)?,
    })
}

fn latest(conn: &Connection) -> Result<Option<StoredSettings>, RuntimeError> {
    let sql = format!(
        "SELECT {COLUMNS}, version, set_by, set_at FROM ops_settings_versions \
         ORDER BY version DESC LIMIT 1"
    );
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([], stored_settings)
        .optional()?)
}

/// The settings in force: the newest version, or the defaults until one is set.
pub fn current(conn: &Connection) -> Result<OperationsSettings, RuntimeError> {
    Ok(latest(conn)?.map_or(DEFAULT_SETTINGS, |stored| stored.values))
}

/// The settings in force with their version and author, for the settings screen.
pub fn get_operations_settings(conn: &Connection) -> Result<OperationsSettingsView, RuntimeError> {
    let stored = latest(conn)?;
    Ok(OperationsSettingsView {
        version: stored.as_ref().map(|stored| stored.version),
        values: stored
            .as_ref()
            .map_or(DEFAULT_SETTINGS, |stored| stored.values),
        defaults: DEFAULT_SETTINGS,
        set_by: stored.as_ref().map(|stored| stored.set_by.clone()),
        set_by_display: None,
        set_at: stored.map(|stored| stored.set_at),
    })
}

/// Records a new settings version (`manage_configuration`). The expected version must be
/// the one in force (`None` while the defaults apply), every value must be in its range,
/// and every version after the first needs a reason. Audited as `ops.set_settings` on the
/// security stream, with the names of the changed settings.
pub fn set_operations_settings(
    ctx: &mut CommandContext<'_>,
    input: SetOperationsSettingsInput,
) -> Result<OperationsSettingsView, RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    let previous = latest(ctx.tx())?;
    let previous_version = previous.as_ref().map(|stored| stored.version);
    let mut problems = input.values.problems();
    let reason = change_reason(previous_version, input.reason.as_deref())
        .map_err(|problem| problems.push(problem))
        .ok()
        .flatten();
    if !problems.is_empty() {
        return Err(RuntimeError::Invalid(problems));
    }
    if input.expected_version != previous_version {
        return Err(RuntimeError::SettingsVersionConflict);
    }
    let version = previous_version.unwrap_or(0) + 1;
    insert(
        ctx,
        version,
        &input.values,
        previous_version.and(reason.as_deref()),
    )?;
    let before = previous.map_or(DEFAULT_SETTINGS, |stored| stored.values);
    let mut event = AuditEvent::new(Stream::Security, SET_SETTINGS, Outcome::Succeeded)
        .record("operations_settings", version)
        .new_revision(version)
        .detail(
            "changed",
            AuditDetail::codes(changed(&before, &input.values)),
        );
    if let Some(previous_version) = previous_version {
        event = event.old_revision(previous_version);
    }
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    get_operations_settings(ctx.tx())
}

/// The reason for a change: required once a version exists, optional for the first.
fn change_reason(
    previous: Option<i64>,
    reason: Option<&str>,
) -> Result<Option<String>, lis_core::FieldError> {
    match reason.filter(|text| !text.trim().is_empty()) {
        Some(text) => required_text("reason", text, NOTE_MAX_CHARS).map(Some),
        None if previous.is_some() => required_text("reason", "", NOTE_MAX_CHARS).map(Some),
        None => Ok(None),
    }
}

fn insert(
    ctx: &CommandContext<'_>,
    version: i64,
    values: &OperationsSettings,
    reason: Option<&str>,
) -> Result<(), RuntimeError> {
    let sql = format!(
        "INSERT INTO ops_settings_versions ({COLUMNS}, version, reason, set_by, set_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
         ?18, ?19)"
    );
    ctx.tx().prepare_cached(&sql)?.execute(params![
        values.backup_interval_minutes,
        values.keep_recent,
        values.keep_daily,
        values.keep_weekly,
        values.backup_warn_minutes,
        values.backup_alert_minutes,
        values.offhost_warn_hours,
        values.offline_warn_days,
        values.restore_test_warn_days,
        values.kit_confirm_warn_days,
        values.anchor_warn_minutes,
        values.anchor_incident_hours,
        values.disk_free_warn_gib,
        values.os_support_warn_days,
        values.clock_jump_seconds,
        version,
        reason,
        ctx.principal().actor().id_text(),
        ctx.now(),
    ])?;
    Ok(())
}

/// The snake_case names of the settings whose values differ.
fn changed(before: &OperationsSettings, after: &OperationsSettings) -> Vec<&'static str> {
    before
        .fields()
        .into_iter()
        .zip(after.fields())
        .filter(|(old, new)| old.value != new.value)
        .map(|(old, _)| old.code)
        .collect()
}
