//! The laboratory's separation-of-duties policy (ADR 0015; Q-08, Q-37). Three duties have
//! two steps that normally need two people; the laboratory may let one person do both.
//! CLIA lets one qualified person hold every role (42 CFR 493.1445(a)) and leaves review
//! before reporting to the director (493.1445(e)(15)), so this is laboratory policy, not a
//! fixed rule. Versions are append-only; until the laboratory sets one, every duty needs a
//! second person.
//!
//! [`SeparationPolicy::authorize_self`] is the one home of the self-approval rule: the
//! policy in force must allow it, and the person must confirm it for that action. The
//! module owning the action calls it inside its transaction, refuses as it returns, and
//! marks the audit event with [`lis_audit::AuditEvent::self_approval`].

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::account::parse_reason;
use crate::actions;
use crate::error::{Attempt, AuthError};
use crate::reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};

/// The input field that confirms a self-approval, named in [`SelfApprovalRefused::NotConfirmed`].
pub const SELF_APPROVAL_FIELD: &str = "selfApproval";

/// A duty whose second step normally needs a second person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duty {
    /// Approving configuration one wrote: a catalog version, the drafts of a Catalog
    /// Preview, a result source registration, a billing class mapping or the report layout.
    ConfigurationApproval,
    /// Releasing a report whose reported results one reviewed technically.
    ReportRelease,
    /// Verifying a delivery destination one created.
    DestinationVerification,
}

impl Duty {
    /// The field error on [`SELF_APPROVAL_FIELD`] that asks the person to confirm a
    /// self-approval of this duty ([`SelfApprovalRefused::NotConfirmed`]).
    #[must_use]
    pub fn confirmation_field_error(self) -> FieldError {
        FieldError {
            field: SELF_APPROVAL_FIELD.to_owned(),
            code: "self_approval_unconfirmed".to_owned(),
            message: not_confirmed_message(self).to_owned(),
        }
    }
}

/// Whether one person may do both steps of each [`Duty`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SeparationPolicy {
    /// The author of configuration may approve it.
    pub self_approve_configuration: bool,
    /// The technical reviewer of reported results may release the report.
    pub self_release_reports: bool,
    /// The creator of a delivery destination may verify it.
    pub self_verify_destinations: bool,
}

/// The policy until the laboratory sets one: every duty needs a second person.
pub const DEFAULT_SEPARATION_POLICY: SeparationPolicy = SeparationPolicy {
    self_approve_configuration: false,
    self_release_reports: false,
    self_verify_destinations: false,
};

/// The policy of a laboratory that one person runs: every duty may be done alone.
pub(crate) const ONE_PERSON_SEPARATION_POLICY: SeparationPolicy = SeparationPolicy {
    self_approve_configuration: true,
    self_release_reports: true,
    self_verify_destinations: true,
};

impl SeparationPolicy {
    /// Whether the policy lets one person do both steps of `duty`.
    #[must_use]
    pub fn allows(self, duty: Duty) -> bool {
        match duty {
            Duty::ConfigurationApproval => self.self_approve_configuration,
            Duty::ReportRelease => self.self_release_reports,
            Duty::DestinationVerification => self.self_verify_destinations,
        }
    }

    /// Authorizes the second step of `duty` by the person who did the first: the policy
    /// must allow it, and `confirmed` must carry the person's confirmation for this action.
    pub fn authorize_self(self, duty: Duty, confirmed: bool) -> Result<(), SelfApprovalRefused> {
        if !self.allows(duty) {
            Err(SelfApprovalRefused::NotAllowed(duty))
        } else if !confirmed {
            Err(SelfApprovalRefused::NotConfirmed(duty))
        } else {
            Ok(())
        }
    }
}

/// Why the person who did a duty's first step may not do its second. The owning
/// module maps it to its own error: [`Self::NotAllowed`] to its existing refusal, and
/// [`Self::NotConfirmed`] to `validation` with [`Duty::confirmation_field_error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SelfApprovalRefused {
    /// The laboratory's policy requires a second person for this duty.
    #[error("{}", not_allowed_message(*.0))]
    NotAllowed(Duty),
    /// The policy allows it, but the person did not confirm it for this action.
    #[error("Check the highlighted fields.")]
    NotConfirmed(Duty),
}

fn not_allowed_message(duty: Duty) -> &'static str {
    match duty {
        Duty::ConfigurationApproval => {
            "You wrote this configuration, and your laboratory's separation-of-duties policy \
             requires someone else to approve it."
        }
        Duty::ReportRelease => {
            "You reviewed results on this report, and your laboratory's separation-of-duties \
             policy requires someone else to release it."
        }
        Duty::DestinationVerification => {
            "You created this destination, and your laboratory's separation-of-duties policy \
             requires someone else to verify it."
        }
    }
}

fn not_confirmed_message(duty: Duty) -> &'static str {
    match duty {
        Duty::ConfigurationApproval => {
            "You wrote this configuration. Your laboratory allows you to approve it yourself: \
             confirm that no second person reviewed it."
        }
        Duty::ReportRelease => {
            "You reviewed results on this report. Your laboratory allows you to release it \
             yourself: confirm that no second person reviewed it."
        }
        Duty::DestinationVerification => {
            "You created this destination. Your laboratory allows you to verify it yourself: \
             confirm that no second person checked it."
        }
    }
}

/// The policy as the settings screen shows it (`get_separation_policy`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeparationPolicyView {
    /// The version in force; `None` while the default applies.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
    /// The values in force.
    pub values: SeparationPolicy,
    /// Why the version in force was set.
    pub reason: Option<String>,
    /// Who set it (principal ID text).
    pub set_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub set_by_display: Option<String>,
    /// When it was set.
    pub set_at: Option<UtcTimestamp>,
}

/// Input of `set_separation_policy`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetSeparationPolicyInput {
    /// The version the administrator saw; `None` while the default applied.
    #[ts(type = "number | null")]
    pub expected_version: Option<i64>,
    /// The new values.
    pub values: SeparationPolicy,
    /// Why.
    pub reason: String,
    /// Fresh confirmation of the acting user's selected method (REQ-IDN-004).
    pub password: Reauthentication,
}

struct StoredPolicy {
    version: i64,
    values: SeparationPolicy,
    reason: String,
    set_by: String,
    set_at: UtcTimestamp,
}

fn stored_policy(row: &Row<'_>) -> rusqlite::Result<StoredPolicy> {
    Ok(StoredPolicy {
        version: row.get(0)?,
        values: SeparationPolicy {
            self_approve_configuration: row.get(1)?,
            self_release_reports: row.get(2)?,
            self_verify_destinations: row.get(3)?,
        },
        reason: row.get(4)?,
        set_by: row.get(5)?,
        set_at: row.get(6)?,
    })
}

fn latest(conn: &Connection) -> rusqlite::Result<Option<StoredPolicy>> {
    conn.prepare_cached(
        "SELECT version, self_approve_configuration, self_release_reports, \
         self_verify_destinations, reason, set_by, set_at FROM auth_separation_policies \
         ORDER BY version DESC LIMIT 1",
    )?
    .query_row([], stored_policy)
    .optional()
}

/// The policy in force, read inside the caller's transaction: the newest version, or
/// [`DEFAULT_SEPARATION_POLICY`] until one is set.
pub fn current_separation_policy(conn: &Connection) -> rusqlite::Result<SeparationPolicy> {
    Ok(latest(conn)?.map_or(DEFAULT_SEPARATION_POLICY, |stored| stored.values))
}

/// The policy in force with its version, reason and author, for the settings screen.
pub fn get_separation_policy(conn: &Connection) -> Result<SeparationPolicyView, AuthError> {
    Ok(match latest(conn)? {
        Some(stored) => SeparationPolicyView {
            version: Some(stored.version),
            values: stored.values,
            reason: Some(stored.reason),
            set_by: Some(stored.set_by),
            set_by_display: None,
            set_at: Some(stored.set_at),
        },
        None => SeparationPolicyView {
            version: None,
            values: DEFAULT_SEPARATION_POLICY,
            reason: None,
            set_by: None,
            set_by_display: None,
            set_at: None,
        },
    })
}

/// Records a new policy version (`approve_configuration`, fresh password, reason; ADR 0018).
/// The expected version must be the one in force (`None` while the default applies).
/// Audited as `auth.set_separation_policy` on the security stream.
pub fn set_separation_policy(
    ctx: &mut CommandContext<'_>,
    input: SetSeparationPolicyInput,
) -> Result<Attempt<SeparationPolicyView>, AuthError> {
    ctx.require(Capability::ApproveConfiguration)?;
    let reason = parse_reason(&input.reason).map_err(|problem| problem.invalid("reason"))?;
    let previous = latest(ctx.tx())?.map(|stored| stored.version);
    if input.expected_version != previous {
        return Err(AuthError::SeparationPolicyVersionConflict);
    }
    if let Attempt::Rejected(rejection) =
        reauthenticate(ctx, &input.password, ReauthPurpose::SeparationPolicy)?
    {
        return Ok(Attempt::Rejected(rejection));
    }
    record_separation_policy(ctx, input.values, reason)?;
    get_separation_policy(ctx.tx()).map(Attempt::Accepted)
}

/// Appends `values` as the next version with `reason`, and audits it. The caller has
/// authorized the change.
pub(crate) fn record_separation_policy(
    ctx: &CommandContext<'_>,
    values: SeparationPolicy,
    reason: String,
) -> Result<(), AuthError> {
    let previous = latest(ctx.tx())?.map(|stored| stored.version);
    let version = previous.unwrap_or(0) + 1;
    ctx.tx()
        .prepare_cached(
            "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
             self_release_reports, self_verify_destinations, reason, set_by, set_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![
            version,
            values.self_approve_configuration,
            values.self_release_reports,
            values.self_verify_destinations,
            reason,
            ctx.principal().actor().id_text(),
            ctx.now(),
        ])?;
    let mut event = AuditEvent::new(
        Stream::Security,
        actions::SET_SEPARATION_POLICY,
        Outcome::Succeeded,
    )
    .record("separation_policy", version)
    .new_revision(version)
    .reason(reason)
    .detail(
        "self_approve_configuration",
        AuditDetail::boolean(values.self_approve_configuration),
    )
    .detail(
        "self_release_reports",
        AuditDetail::boolean(values.self_release_reports),
    )
    .detail(
        "self_verify_destinations",
        AuditDetail::boolean(values.self_verify_destinations),
    );
    if let Some(previous) = previous {
        event = event.old_revision(previous);
    }
    ctx.audit(event)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUTIES: [Duty; 3] = [
        Duty::ConfigurationApproval,
        Duty::ReportRelease,
        Duty::DestinationVerification,
    ];

    #[test]
    fn the_default_needs_a_second_person_for_every_duty() {
        for duty in DUTIES {
            assert_eq!(
                DEFAULT_SEPARATION_POLICY.authorize_self(duty, true),
                Err(SelfApprovalRefused::NotAllowed(duty))
            );
        }
    }

    #[test]
    fn an_allowed_self_approval_still_needs_its_confirmation() {
        for duty in DUTIES {
            let refused = ONE_PERSON_SEPARATION_POLICY.authorize_self(duty, false);
            assert_eq!(refused, Err(SelfApprovalRefused::NotConfirmed(duty)));
            assert_eq!(duty.confirmation_field_error().field, SELF_APPROVAL_FIELD);
            assert_eq!(
                ONE_PERSON_SEPARATION_POLICY.authorize_self(duty, true),
                Ok(())
            );
        }
    }

    #[test]
    fn each_switch_governs_only_its_own_duty() {
        let policy = SeparationPolicy {
            self_release_reports: true,
            ..DEFAULT_SEPARATION_POLICY
        };
        assert!(policy.allows(Duty::ReportRelease));
        assert!(!policy.allows(Duty::ConfigurationApproval));
        assert!(!policy.allows(Duty::DestinationVerification));
    }
}
