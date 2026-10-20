//! The laboratory's security policy (NFR-19, laboratory question L11): the idle timeout
//! and the sign-in lockout, in versioned append-only rows (M4 design brief D12, §8).
//! Until the laboratory sets values, [`DEFAULT_SECURITY_POLICY`] applies. The throttle
//! reads the lockout values inside the attempt's own transaction.

use std::ops::RangeInclusive;
use std::time::Duration;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::account::parse_reason;
use crate::actions;
use crate::error::{Attempt, AuthError, Problem};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};

/// The idle timeout and lockout values in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityPolicy {
    /// Minutes without input after which a session locks (2–60).
    pub idle_timeout_minutes: u32,
    /// Consecutive failures that lock a username (3–10).
    pub lockout_failures: u32,
    /// How long a lockout lasts, in minutes (5–60).
    pub lockout_minutes: u32,
    /// Permit a newly enrolled key without PIN/biometric verification at sign-in.
    #[serde(default)]
    pub allow_security_key_touch_only: bool,
}

/// The values that apply until the laboratory sets its own (L11): a 15-minute idle
/// timeout; five consecutive failures lock a username for 15 minutes.
pub const DEFAULT_SECURITY_POLICY: SecurityPolicy = SecurityPolicy {
    idle_timeout_minutes: 15,
    lockout_failures: 5,
    lockout_minutes: 15,
    allow_security_key_touch_only: false,
};

const IDLE_TIMEOUT_MINUTES: RangeInclusive<u32> = 2..=60;
const LOCKOUT_FAILURES: RangeInclusive<u32> = 3..=10;
const LOCKOUT_MINUTES: RangeInclusive<u32> = 5..=60;

impl SecurityPolicy {
    /// The idle time after which a session locks.
    #[must_use]
    pub fn idle_timeout(&self) -> Duration {
        minutes(self.idle_timeout_minutes)
    }

    /// How long a lockout lasts.
    pub(crate) fn lockout_duration(&self) -> Duration {
        minutes(self.lockout_minutes)
    }

    /// Every value outside its allowed range, as field errors.
    fn problems(&self) -> Vec<FieldError> {
        [
            (
                "idleTimeoutMinutes",
                self.idle_timeout_minutes,
                IDLE_TIMEOUT_MINUTES,
            ),
            ("lockoutFailures", self.lockout_failures, LOCKOUT_FAILURES),
            ("lockoutMinutes", self.lockout_minutes, LOCKOUT_MINUTES),
        ]
        .into_iter()
        .filter(|(_, value, range)| !range.contains(value))
        .map(|(field, _, range)| out_of_range(field, &range))
        .collect()
    }
}

fn minutes(count: u32) -> Duration {
    Duration::from_secs(u64::from(count) * 60)
}

fn out_of_range(field: &str, range: &RangeInclusive<u32>) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: "out_of_range".to_owned(),
        message: format!("Enter a value from {} to {}.", range.start(), range.end()),
    }
}

/// The security policy as the settings screen shows it (`get_security_policy`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SecurityPolicyView {
    /// The version in force; `None` while the defaults apply.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
    /// The values in force.
    pub values: SecurityPolicy,
    /// The defaults, for comparison.
    pub defaults: SecurityPolicy,
    /// Who set the version in force (principal ID text).
    pub set_by: Option<String>,
    /// When it was set.
    pub set_at: Option<UtcTimestamp>,
}

/// Input of `set_security_policy`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetSecurityPolicyInput {
    /// The version the administrator saw; `None` while the defaults applied.
    #[ts(type = "number | null")]
    pub expected_version: Option<i64>,
    /// Minutes without input after which a session locks.
    pub idle_timeout_minutes: u32,
    /// Consecutive failures that lock a username.
    pub lockout_failures: u32,
    /// How long a lockout lasts, in minutes.
    pub lockout_minutes: u32,
    /// Permit a newly enrolled key without PIN/biometric verification at sign-in.
    #[serde(default)]
    pub allow_security_key_touch_only: bool,
    /// Fresh confirmation, required only when the touch-only setting changes.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub password: Option<Reauthentication>,
    /// Why; required after the first version and whenever touch-only is enabled.
    pub reason: Option<String>,
}

struct StoredPolicy {
    version: i64,
    values: SecurityPolicy,
    set_by: String,
    set_at: UtcTimestamp,
}

fn stored_policy(row: &Row<'_>) -> rusqlite::Result<StoredPolicy> {
    Ok(StoredPolicy {
        version: row.get(0)?,
        values: SecurityPolicy {
            idle_timeout_minutes: row.get(1)?,
            lockout_failures: row.get(2)?,
            lockout_minutes: row.get(3)?,
            allow_security_key_touch_only: row.get(6)?,
        },
        set_by: row.get(4)?,
        set_at: row.get(5)?,
    })
}

fn latest(conn: &Connection) -> Result<Option<StoredPolicy>, AuthError> {
    Ok(conn
        .prepare_cached(
            "SELECT version, idle_timeout_minutes, lockout_failures, lockout_minutes, set_by, \
             set_at, allow_security_key_touch_only FROM auth_security_policies ORDER BY version DESC LIMIT 1",
        )?
        .query_row([], stored_policy)
        .optional()?)
}

/// The policy in force: the newest version, or the defaults until one is set.
pub fn current_security_policy(conn: &Connection) -> Result<SecurityPolicy, AuthError> {
    Ok(latest(conn)?.map_or(DEFAULT_SECURITY_POLICY, |stored| stored.values))
}

/// The policy in force with its version and author, for the settings screen.
pub fn get_security_policy(conn: &Connection) -> Result<SecurityPolicyView, AuthError> {
    let stored = latest(conn)?;
    Ok(SecurityPolicyView {
        version: stored.as_ref().map(|stored| stored.version),
        values: stored
            .as_ref()
            .map_or(DEFAULT_SECURITY_POLICY, |stored| stored.values),
        defaults: DEFAULT_SECURITY_POLICY,
        set_by: stored.as_ref().map(|stored| stored.set_by.clone()),
        set_at: stored.map(|stored| stored.set_at),
    })
}

/// Records a new policy version (`manage_users`; NFR-19). The expected version must
/// be the one in force (`None` while the defaults apply), every value must be in its
/// range, and every version after the first needs a reason. Audited as
/// `auth.set_security_policy` on the security stream.
pub fn set_security_policy(
    ctx: &mut CommandContext<'_>,
    input: SetSecurityPolicyInput,
) -> Result<Attempt<SecurityPolicyView>, AuthError> {
    ctx.require(Capability::ManageUsers)?;
    let values = SecurityPolicy {
        idle_timeout_minutes: input.idle_timeout_minutes,
        lockout_failures: input.lockout_failures,
        lockout_minutes: input.lockout_minutes,
        allow_security_key_touch_only: input.allow_security_key_touch_only,
    };
    let mut problems = values.problems();
    let stored = latest(ctx.tx())?;
    let previous = stored.as_ref().map(|stored| stored.version);
    let previous_values = stored.map_or(DEFAULT_SECURITY_POLICY, |stored| stored.values);
    let key_policy_changed =
        previous_values.allow_security_key_touch_only != values.allow_security_key_touch_only;
    let reason = change_reason(
        previous.is_some() || values.allow_security_key_touch_only,
        input.reason.as_deref(),
    )
    .map_err(|problem| problems.push(problem.at("reason")))
    .ok()
    .flatten();
    if !problems.is_empty() {
        return Err(AuthError::Invalid(problems));
    }
    if input.expected_version != previous {
        return Err(AuthError::PolicyVersionConflict);
    }
    // Bind a frozen key-action receipt even for a numeric-only policy update.
    if (key_policy_changed || matches!(input.password, Some(Reauthentication::SecurityKey { .. })))
        && let Attempt::Rejected(reason) = confirm_key_policy_change(ctx, input.password.as_ref())?
    {
        return Ok(Attempt::Rejected(reason));
    }
    if key_policy_changed {
        let (_, session_id) = crate::sign_in::session_actor(ctx)?;
        crate::key_policy::invalidate_policy_sessions(ctx, session_id)?;
    }
    let version = previous.unwrap_or(0) + 1;
    let stored_reason = reason.as_deref();
    ctx.tx()
        .prepare_cached(
            "INSERT INTO auth_security_policies (version, idle_timeout_minutes, \
             lockout_failures, lockout_minutes, reason, set_by, set_at, allow_security_key_touch_only) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?
        .execute(params![
            version,
            values.idle_timeout_minutes,
            values.lockout_failures,
            values.lockout_minutes,
            stored_reason,
            ctx.principal().actor().id_text(),
            ctx.now(),
            values.allow_security_key_touch_only,
        ])?;
    ctx.audit(policy_event(previous, version, &values, reason))?;
    Ok(Attempt::Accepted(get_security_policy(ctx.tx())?))
}

fn confirm_key_policy_change(
    ctx: &mut CommandContext<'_>,
    confirmation: Option<&Reauthentication>,
) -> Result<Attempt<()>, AuthError> {
    let (user_id, session_id) = crate::sign_in::session_actor(ctx)?;
    if !crate::sessions::session_is_open(ctx.tx(), session_id, user_id)? {
        return Err(AuthError::SessionUnavailable);
    }
    let empty = Reauthentication::from(secrecy::SecretString::from(""));
    if let Attempt::Rejected(reason) = reauthenticate(
        ctx,
        confirmation.unwrap_or(&empty),
        ReauthPurpose::SecurityKeyPolicy,
    )? {
        return Ok(Attempt::Rejected(reason));
    }
    Ok(Attempt::Accepted(()))
}

/// A reason is required for subsequent versions and first-version relaxation.
fn change_reason(required: bool, reason: Option<&str>) -> Result<Option<String>, Problem> {
    match reason.filter(|text| !text.trim().is_empty()) {
        Some(text) => parse_reason(text).map(Some),
        None if required => parse_reason("").map(Some),
        None => Ok(None),
    }
}

fn policy_event(
    previous: Option<i64>,
    version: i64,
    values: &SecurityPolicy,
    reason: Option<String>,
) -> AuditEvent {
    let mut event = AuditEvent::new(
        Stream::Security,
        actions::SET_SECURITY_POLICY,
        Outcome::Succeeded,
    )
    .record("security_policy", version)
    .new_revision(version)
    .detail(
        "idle_timeout_minutes",
        AuditDetail::unsigned(values.idle_timeout_minutes.into()),
    )
    .detail(
        "lockout_failures",
        AuditDetail::unsigned(values.lockout_failures.into()),
    )
    .detail(
        "lockout_minutes",
        AuditDetail::unsigned(values.lockout_minutes.into()),
    );
    event = event.detail(
        "allow_security_key_touch_only",
        AuditDetail::boolean(values.allow_security_key_touch_only),
    );
    if let Some(previous) = previous {
        event = event.old_revision(previous);
    }
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_within_their_ranges() {
        assert!(DEFAULT_SECURITY_POLICY.problems().is_empty());
        assert_eq!(
            DEFAULT_SECURITY_POLICY.idle_timeout(),
            Duration::from_secs(900)
        );
        assert_eq!(
            DEFAULT_SECURITY_POLICY.lockout_duration(),
            Duration::from_secs(900)
        );
    }

    #[test]
    fn range_boundaries_are_inclusive() {
        let at = |idle, failures, lock| SecurityPolicy {
            idle_timeout_minutes: idle,
            lockout_failures: failures,
            lockout_minutes: lock,
            allow_security_key_touch_only: false,
        };
        assert!(at(2, 3, 5).problems().is_empty());
        assert!(at(60, 10, 60).problems().is_empty());
        let fields: Vec<String> = at(1, 11, 4)
            .problems()
            .into_iter()
            .map(|problem| problem.field)
            .collect();
        assert_eq!(
            fields,
            ["idleTimeoutMinutes", "lockoutFailures", "lockoutMinutes"]
        );
        assert_eq!(at(61, 2, 61).problems().len(), 3);
    }
}
