//! Shared audited failures and lockout for every selected authentication method.

use crate::actions;
use crate::error::{Attempt, AuthError, Rejection};
use crate::reauthenticate::ReauthPurpose;
use crate::repo::UserRow;
use crate::throttle;
use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::UserId;
use lis_store::CommandContext;

/// One failed attempt to record against a username.
pub(crate) struct Failure<'a> {
    action: &'static str,
    username: &'a str,
    user_id: Option<UserId>,
    pub(crate) reason_code: &'static str,
    /// The signed action a re-authentication confirms.
    pub(crate) purpose: Option<ReauthPurpose>,
}

impl<'a> Failure<'a> {
    pub(crate) fn for_username(action: &'static str, username: &'a str) -> Self {
        Self {
            action,
            username,
            user_id: None,
            reason_code: "invalid_credentials",
            purpose: None,
        }
    }
    /// A failure of `action` against the account `user`.
    pub(crate) fn of(action: &'static str, user: &'a UserRow) -> Self {
        Self {
            action,
            username: user.username.as_str(),
            user_id: Some(user.id),
            reason_code: "locked_out",
            purpose: None,
        }
    }

    /// True while the username is locked: the attempt is refused unchecked.
    pub(crate) fn is_locked(&self, ctx: &CommandContext<'_>) -> Result<bool, AuthError> {
        throttle::is_locked(ctx.tx(), self.username, ctx.now())
    }
}

/// Counts the failure (unless the username is already locked), audits it and
/// the lockout it may cause, and returns the refusal to commit.
pub(crate) fn refuse<T>(
    ctx: &mut CommandContext<'_>,
    failure: &Failure<'_>,
    rejection: Rejection,
) -> Result<Attempt<T>, AuthError> {
    let locked_now = rejection != Rejection::LockedOut
        && throttle::record_failure(ctx.tx(), failure.username, ctx.now())?;
    let username = AuditDetail::workforce_username(failure.username);
    let event = |action, outcome| {
        let mut event = AuditEvent::new(Stream::Security, action, outcome)
            .detail("username", username.clone())
            .detail("reason_code", AuditDetail::code(failure.reason_code));
        if let Some(purpose) = failure.purpose {
            event = event.detail("purpose", AuditDetail::code(purpose.code()));
        }
        match failure.user_id {
            Some(id) => event.record("user", id),
            None => event,
        }
    };
    ctx.audit(event(failure.action, Outcome::Failed))?;
    if locked_now {
        ctx.audit(event(actions::LOCKOUT, Outcome::Succeeded))?;
        return Ok(Attempt::Rejected(Rejection::LockedOut));
    }
    Ok(Attempt::Rejected(rejection))
}
