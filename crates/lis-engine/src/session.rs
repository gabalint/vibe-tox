//! The window's session (REQ-IDN-004, NFR-19): none, active or locked. The engine, not
//! the UI, enforces the session policy at the next command:
//!
//! - **Idle timeout** from the laboratory's security policy (`lis_auth`), cached by the
//!   engine and refreshed when the policy changes: the first command after it locks the
//!   session and is refused with `session_locked`.
//! - **Absolute lifetime** of [`ABSOLUTE_SESSION_LIFETIME`], whatever the activity: the
//!   session ends and the command is refused with `unauthenticated`.
//! - **Disabled account**: a session whose user was disabled ends at the next command,
//!   so it keeps no session-only reads.
//!
//! A forced end is audited as `auth.session_expire` with its trigger, then as the
//! session's end.

use std::sync::{MutexGuard, PoisonError};
use std::time::Duration;

use lis_auth::{AuthError, EndReason, LockTrigger, RoleCode, SecurityPolicy, SessionUser};
use lis_core::{CorrelationId, ErrorCode, Principal, SessionId, UserId, UtcTimestamp};
use serde::Serialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Call, Failure};
use crate::registry::{Access, LOCK_SESSION};

/// The idle timeout that applies until the laboratory sets a security policy
/// (`lis_auth::DEFAULT_SECURITY_POLICY`, laboratory question L11).
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A session ends this long after sign-in, whatever the activity (security review
/// 2026-10-13). Fixed: an engineering limit until the laboratory policy covers it.
pub const ABSOLUTE_SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

/// The signed-in session of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) id: SessionId,
    pub(crate) user_id: UserId,
    pub(crate) last_activity: UtcTimestamp,
}

/// Whether someone is signed in, and whether the session is locked.
#[derive(Debug, Default)]
pub(crate) enum SessionState {
    #[default]
    None,
    Active(Session),
    Locked(Session),
}

/// The session as found by a command.
enum Found {
    Unlocked(Session),
    /// Locked; `now` is true when this lookup locked it for idleness.
    Locked(Session, bool),
    /// Past its absolute lifetime; already removed from the window.
    Expired(Session),
}

/// The signed-in person and session, as the UI shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionInfo {
    /// The account.
    pub user_id: UserId,
    /// Sign-in name.
    pub username: String,
    /// The person's name.
    pub display_name: String,
    /// Current roles.
    pub roles: Vec<RoleCode>,
    /// Capability codes the roles grant now.
    pub capabilities: Vec<String>,
    /// The engine's idle timeout.
    pub idle_timeout_seconds: u32,
    /// True while the session waits for the user's password, key, or enabled unlock PIN.
    pub locked: bool,
}

impl SessionInfo {
    /// The view of `user`'s session, with the idle timeout of the security policy in force.
    pub(crate) fn new(user: &SessionUser, locked: bool, policy: SecurityPolicy) -> Self {
        Self {
            user_id: user.user_id,
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            roles: user.roles.clone(),
            capabilities: user
                .capabilities()
                .into_iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            idle_timeout_seconds: u32::try_from(policy.idle_timeout().as_secs())
                .unwrap_or(u32::MAX),
            locked,
        }
    }
}

impl Engine {
    pub(crate) fn session_state(&self) -> MutexGuard<'_, SessionState> {
        // A poisoned lock still holds a coherent state.
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The cached security policy.
    pub(crate) fn security_policy(&self) -> SecurityPolicy {
        *self.policy.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Replaces the cached policy after a new version is recorded.
    pub(crate) fn refresh_security_policy(&self, policy: SecurityPolicy) {
        *self.policy.lock().unwrap_or_else(PoisonError::into_inner) = policy;
    }

    /// When `session` started: the activity time of its first lookup, which is its sign-in
    /// time because only lookups move it.
    pub(crate) fn session_started(&self, session: Session) -> UtcTimestamp {
        let mut started = self
            .session_started
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match *started {
            Some((id, at)) if id == session.id => at,
            _ => {
                *started = Some((session.id, session.last_activity));
                session.last_activity
            }
        }
    }

    /// The session, after ending it past its lifetime or locking it if it has been idle
    /// too long. An unlocked session counts this lookup as activity.
    fn find_session(&self) -> Option<Found> {
        let now = self.clock.now();
        let idle_timeout = self.security_policy().idle_timeout();
        let mut state = self.session_state();
        let session = match *state {
            SessionState::None => return None,
            SessionState::Active(session) | SessionState::Locked(session) => session,
        };
        if now.saturating_duration_since(self.session_started(session)) >= ABSOLUTE_SESSION_LIFETIME
        {
            *state = SessionState::None;
            self.key_ceremonies.invalidate();
            self.layout_samples.clear();
            return Some(Found::Expired(session));
        }
        Some(match *state {
            SessionState::Active(session)
                if now.saturating_duration_since(session.last_activity) >= idle_timeout =>
            {
                *state = SessionState::Locked(session);
                self.key_ceremonies.invalidate();
                self.layout_samples.clear();
                Found::Locked(session, true)
            }
            SessionState::Active(mut session) => {
                session.last_activity = now;
                *state = SessionState::Active(session);
                Found::Unlocked(session)
            }
            _ => Found::Locked(session, false),
        })
    }

    /// Locks an unlocked session and returns it; `None` if there was none to lock.
    pub(crate) fn lock_if_active(&self) -> Option<Session> {
        let mut state = self.session_state();
        // A repeated lock request must also cancel any pending unlock.
        self.key_ceremonies.invalidate();
        let SessionState::Active(session) = *state else {
            return None;
        };
        *state = SessionState::Locked(session);
        self.layout_samples.clear();
        Some(session)
    }

    /// Removes only the captured session; never clears a newer user's session.
    pub(crate) fn forget_session(&self, session_id: SessionId) {
        let mut state = self.session_state();
        if matches!(*state, SessionState::Active(open) | SessionState::Locked(open)
            if open.id == session_id)
        {
            *state = SessionState::None;
            self.key_ceremonies.invalidate();
            self.layout_samples.clear();
        }
    }

    /// The session (locked or not) for public queries, after the policy checks.
    pub(crate) async fn observe_session(
        &self,
        correlation_id: CorrelationId,
    ) -> Result<Option<(Session, bool)>, Failure> {
        Ok(self
            .observe(correlation_id)
            .await?
            .map(|(session, locked, _)| (session, locked)))
    }

    /// The session, whether it is locked, and its user's current account; `None` when no
    /// one is signed in or the session was ended now (lifetime or disabled account).
    async fn observe(
        &self,
        correlation_id: CorrelationId,
    ) -> Result<Option<(Session, bool, SessionUser)>, Failure> {
        let (session, locked) = match self.find_session() {
            None => return Ok(None),
            Some(Found::Expired(session)) => {
                self.end_forced(session, EndReason::Expired, correlation_id)
                    .await?;
                return Ok(None);
            }
            Some(Found::Unlocked(session)) => (session, false),
            Some(Found::Locked(session, idle_now)) => {
                if idle_now {
                    self.audit_lock(session, LockTrigger::Idle, correlation_id)
                        .await?;
                }
                (session, true)
            }
        };
        let user_id = session.user_id;
        let session_id = session.id;
        let is_open = self
            .db
            .read(move |conn| lis_auth::session_is_open(conn, session_id, user_id))
            .await?;
        if !is_open {
            self.forget_session(session.id);
            return Ok(None);
        }
        let user: SessionUser = self
            .db
            .read(move |conn| lis_auth::session_user(conn, user_id))
            .await?;
        if user.status == lis_auth::UserStatus::Disabled {
            self.end_forced(session, EndReason::AccountDisabled, correlation_id)
                .await?;
            return Ok(None);
        }
        Ok(Some((session, locked, user)))
    }

    /// The signed-in user as a principal with their current capabilities.
    pub(crate) async fn session_principal(
        &self,
        access: Access,
        correlation_id: CorrelationId,
    ) -> Result<Principal, Failure> {
        let Some((session, locked, user)) = self.observe(correlation_id).await? else {
            return Err(Failure::refused(
                ErrorCode::Unauthenticated,
                "Sign in to continue.",
            ));
        };
        if locked && access != Access::AnySession {
            return Err(Failure::refused(
                ErrorCode::SessionLocked,
                "Your session is locked. Authenticate to continue.",
            ));
        }
        Ok(self.user_principal(session, user.capabilities()))
    }

    pub(crate) fn user_principal(
        &self,
        session: Session,
        capabilities: impl IntoIterator<Item = lis_core::Capability>,
    ) -> Principal {
        Principal::user(
            session.user_id,
            session.id,
            self.workstation_id,
            capabilities,
        )
    }

    /// Audits that `session` was locked, as its user.
    pub(crate) async fn audit_lock(
        &self,
        session: Session,
        trigger: LockTrigger,
        correlation_id: CorrelationId,
    ) -> Result<(), Failure> {
        let call = Call {
            spec: &LOCK_SESSION,
            correlation_id,
            principal: self.user_principal(session, []),
        };
        self.write(&call, move |ctx| {
            lis_auth::record_lock(ctx, session.id, trigger)
        })
        .await
    }

    /// Ends `session` for `reason`, a policy end, removes it from the window, and audits
    /// it (`auth.session_end` with the reason).
    async fn end_forced(
        &self,
        session: Session,
        reason: EndReason,
        correlation_id: CorrelationId,
    ) -> Result<(), Failure> {
        self.forget_session(session.id);
        tracing::info!(session_id = %session.id, ?reason, "session ended by policy");
        let ended: Result<(), AuthError> = self
            .db
            .write(
                self.user_principal(session, []),
                correlation_id,
                move |ctx| lis_auth::end_session(ctx, session.id, reason),
            )
            .await;
        Ok(ended?)
    }
}
