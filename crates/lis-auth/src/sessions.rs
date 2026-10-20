//! Session rows, the workstation record, locks and session ends (REQ-IDN-004).
//! Whether a session is locked is engine state; this module records it.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{SessionId, UserId, UtcTimestamp, WorkstationId};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use crate::actions;
use crate::error::AuthError;

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The user signed out.
    SignedOut,
    /// Another sign-in replaced it (fast user switching).
    Switched,
    /// The app closed.
    AppExit,
    /// Found open at startup: the app stopped without ending it.
    Abandoned,
    /// An administrator reset the user's credentials.
    CredentialsReset,
    /// Session policy ended access: absolute lifetime or PIN-attempt exhaustion (NFR-19).
    Expired,
    /// The account was disabled while the session was open.
    AccountDisabled,
}

impl EndReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::SignedOut => "signed_out",
            Self::Switched => "switched",
            Self::AppExit => "app_exit",
            Self::Abandoned => "abandoned",
            Self::CredentialsReset => "credentials_reset",
            Self::Expired => "expired",
            Self::AccountDisabled => "account_disabled",
        }
    }
}

/// What locked a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTrigger {
    /// The user asked.
    User,
    /// The idle timeout passed.
    Idle,
}

/// This computer's workstation ID, registered on first start.
pub fn register_workstation(ctx: &mut CommandContext<'_>) -> Result<WorkstationId, AuthError> {
    let existing: Option<WorkstationId> = ctx
        .tx()
        .prepare_cached("SELECT id FROM auth_workstations ORDER BY registered_at LIMIT 1")?
        .query_row([], |row| row.get(0))
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = WorkstationId::generate();
    ctx.tx()
        .prepare_cached("INSERT INTO auth_workstations (id, registered_at) VALUES (?1, ?2)")?
        .execute(params![id, ctx.now()])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::WORKSTATION_REGISTER,
            Outcome::Succeeded,
        )
        .record("workstation", id),
    )?;
    Ok(id)
}

/// A session about to start.
#[derive(Debug, Clone)]
pub struct NewSession {
    /// Its identifier, chosen by the engine.
    pub id: SessionId,
    /// The computer it runs on.
    pub workstation_id: WorkstationId,
    /// Web engine name and version (ADR 0012).
    pub webview: Option<String>,
    /// The session it replaces, if one was open or locked.
    pub replaces: Option<SessionId>,
}

pub(crate) fn insert_session(
    conn: &Connection,
    session: &NewSession,
    user_id: UserId,
    now: UtcTimestamp,
) -> Result<(), AuthError> {
    conn.prepare_cached(
        "INSERT INTO auth_sessions (id, user_id, workstation_id, webview, started_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![
        session.id,
        user_id,
        session.workstation_id,
        session.webview,
        now
    ])?;
    Ok(())
}

/// Ends session `id` if it is still open and audits it: `auth.sign_out` for
/// [`EndReason::SignedOut`], `auth.session_end` otherwise.
pub fn end_session(
    ctx: &mut CommandContext<'_>,
    id: SessionId,
    reason: EndReason,
) -> Result<(), AuthError> {
    let ended = ctx
        .tx()
        .prepare_cached(
            "UPDATE auth_sessions SET ended_at = ?2, end_reason = ?3 \
             WHERE id = ?1 AND ended_at IS NULL",
        )?
        .execute(params![id, ctx.now(), reason.as_str()])?;
    if ended == 0 {
        return Ok(());
    }
    let action = match reason {
        EndReason::SignedOut => actions::SIGN_OUT,
        EndReason::Switched
        | EndReason::AppExit
        | EndReason::Abandoned
        | EndReason::CredentialsReset
        | EndReason::Expired
        | EndReason::AccountDisabled => actions::SESSION_END,
    };
    ctx.audit(
        AuditEvent::new(Stream::Security, action, Outcome::Succeeded)
            .record("session", id)
            .detail("end_reason", AuditDetail::code(reason.as_str())),
    )?;
    Ok(())
}

/// Startup reconciliation: ends every session still open, as `abandoned`.
pub fn end_abandoned_sessions(ctx: &mut CommandContext<'_>) -> Result<usize, AuthError> {
    let open = ctx
        .tx()
        .prepare_cached("SELECT id FROM auth_sessions WHERE ended_at IS NULL")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<SessionId>>>()?;
    for &id in &open {
        end_session(ctx, id, EndReason::Abandoned)?;
    }
    Ok(open.len())
}

/// Ends every open session of `user_id` after a credential reset.
pub(crate) fn end_sessions_of(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
) -> Result<(), AuthError> {
    end_other_sessions(ctx, user_id, None)
}

pub(crate) fn end_other_sessions(
    ctx: &mut CommandContext<'_>,
    user_id: UserId,
    keep: Option<SessionId>,
) -> Result<(), AuthError> {
    let open = ctx
        .tx()
        .prepare_cached("SELECT id FROM auth_sessions WHERE user_id = ?1 AND ended_at IS NULL")?
        .query_map([user_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<SessionId>>>()?;
    for id in open {
        if Some(id) != keep {
            end_session(ctx, id, EndReason::CredentialsReset)?;
        }
    }
    Ok(())
}

/// Whether a session is still open for its original user. The engine checks
/// this on each invocation so credential revocation takes effect immediately.
pub fn session_is_open(
    conn: &Connection,
    session_id: SessionId,
    user_id: UserId,
) -> Result<bool, AuthError> {
    Ok(conn.prepare_cached(
        "SELECT EXISTS (SELECT 1 FROM auth_sessions WHERE id = ?1 AND user_id = ?2 AND ended_at IS NULL)",
    )?.query_row(params![session_id, user_id], |row| row.get(0))?)
}

/// Audits that session `id` was locked.
pub fn record_lock(
    ctx: &mut CommandContext<'_>,
    id: SessionId,
    trigger: LockTrigger,
) -> Result<(), AuthError> {
    let trigger = match trigger {
        LockTrigger::User => "user",
        LockTrigger::Idle => "idle",
    };
    ctx.audit(
        AuditEvent::new(Stream::Security, actions::LOCK, Outcome::Succeeded)
            .record("session", id)
            .detail("trigger", AuditDetail::code(trigger)),
    )?;
    Ok(())
}
