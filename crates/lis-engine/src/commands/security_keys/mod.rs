//! Native security-key orchestration (ADR 0019, REQ-IDN-004, NFR-19).
//! Trusted Rust retains challenge state, identity and exact action until verification.

mod action;
mod authentication;
mod configuration;
mod registration;
mod types;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use lis_auth::{AuthError, AuthenticationStatus, KeyEnrollment, NewSession, SecurityKeyVerifier};
use lis_core::{CommandError, ErrorCode, SessionId, UtcTimestamp};

use super::sign_in::session_of;
use crate::Engine;
use crate::pipeline::{Call, Failure, accepted};
use crate::registry::{Access, CommandSpec, spec};

pub use action::{SecurityKeyAction, SecurityKeyActionInput};
pub use types::*;

macro_rules! commands {
    ($($constant:ident => ($name:literal, $access:expr)),+ $(,)?) => {
        $(pub(crate) const $constant: CommandSpec = spec($name, concat!("auth.", $name), $access);)+
    };
}
commands! {
    STATUS => ("get_authentication_status", Access::AnySession),
    CHOICES => ("list_key_sign_in_choices", Access::Public),
    SIGNUP => ("security_key_signup", Access::Public),
    ACKNOWLEDGE => ("acknowledge_key_recovery", Access::Public),
    SIGN_IN => ("security_key_sign_in", Access::Public),
    UNLOCK => ("security_key_unlock", Access::AnySession),
    ENROLL => ("security_key_enroll", Access::Session),
    RECOVER => ("security_key_recover", Access::Public),
    ACTION => ("security_key_action", Access::Session),
    REVOKE => ("revoke_security_key", Access::Session),
}

/// One key ceremony or PIN unlock at a time; lock/signout invalidate in-flight results.
#[derive(Default)]
pub(crate) struct KeyCeremonies {
    pub(crate) generation: Arc<AtomicU64>,
    gate: tokio::sync::Mutex<()>,
    pending_setup: Mutex<Option<(KeyEnrollment, UtcTimestamp, u64)>>,
}

impl KeyCeremonies {
    pub(crate) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.pending_setup
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
    pub(super) fn guard(&self, now: UtcTimestamp) -> CeremonyGuard {
        CeremonyGuard {
            generation: self.generation.load(Ordering::SeqCst),
            current: Arc::clone(&self.generation),
            started: now,
            elapsed: Instant::now(),
        }
    }
    pub(super) fn enter(&self) -> Result<tokio::sync::MutexGuard<'_, ()>, Failure> {
        self.gate.try_lock().map_err(|_| {
            Failure::refused(
                ErrorCode::Conflict,
                "Another authentication operation is in progress. Finish or cancel it first.",
            )
        })
    }
}

pub(super) struct CeremonyGuard {
    generation: u64,
    current: Arc<AtomicU64>,
    started: UtcTimestamp,
    elapsed: Instant,
}

impl CeremonyGuard {
    /// Also usable inside the writer: a cancelled settings change must roll back.
    pub(super) fn ensure_current(&self, now: UtcTimestamp) -> Result<(), AuthError> {
        if self.current.load(Ordering::SeqCst) != self.generation
            || now < self.started
            || self.elapsed.elapsed() > Duration::from_secs(120)
            || now.saturating_duration_since(self.started) > Duration::from_secs(120)
        {
            return Err(AuthError::ChallengeExpired);
        }
        Ok(())
    }
}

impl Engine {
    /// Own authentication metadata is also available to the locked session.
    pub async fn get_authentication_status(&self) -> Result<AuthenticationStatus, CommandError> {
        self.run(&STATUS, |call| async move {
            let (user_id, _) = session_of(&call.principal)?;
            self.read(&call, move |conn| {
                lis_auth::get_authentication_status(conn, user_id)
            })
            .await
        })
        .await
    }

    /// Named active-key choices; never exposes raw credentials or public-key encodings.
    pub async fn list_key_sign_in_choices(
        &self,
        input: KeySignInChoicesInput,
    ) -> Result<Vec<lis_auth::SecurityKeyCredentialView>, CommandError> {
        self.run(&CHOICES, |call| async move {
            self.read(&call, move |conn| {
                lis_auth::list_key_sign_in_choices(conn, &input.username)
            })
            .await
        })
        .await
    }

    /// Revokes a personal key. Domain enforces ownership, revisions and fresh confirmation.
    pub async fn revoke_security_key(
        &self,
        input: RevokeSecurityKeyInput,
    ) -> Result<(), CommandError> {
        self.run(&REVOKE, |call| async move {
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::revoke_security_key(
                        ctx,
                        input.credential_id,
                        input.expected_revision,
                        &input.password,
                        &input.reason,
                    )
                })
                .await?;
            accepted(attempt)?;
            self.key_ceremonies.invalidate();
            Ok(())
        })
        .await
    }

    fn key_origin(&self) -> String {
        format!("https://{}.vibe-tox.invalid", self.workstation_id)
    }

    fn key_verifier(&self) -> Result<SecurityKeyVerifier, Failure> {
        Ok(SecurityKeyVerifier::new(
            &format!("{}.vibe-tox.invalid", self.workstation_id),
            &self.key_origin(),
        )?)
    }

    fn new_key_session(&self) -> NewSession {
        NewSession {
            id: SessionId::generate(),
            workstation_id: self.workstation_id,
            webview: self.webview.clone(),
            replaces: self.open_session().map(|session| session.id),
        }
    }

    /// Re-observes policy after hardware interaction, including lock, expiry and DB revocation.
    async fn check_key_guard(&self, call: &Call, guard: &CeremonyGuard) -> Result<(), Failure> {
        if call.spec.access != Access::Public {
            let principal = self
                .session_principal(call.spec.access, call.correlation_id)
                .await?;
            if session_of(&principal)? != session_of(&call.principal)? {
                return Err(AuthError::ChallengeExpired.into());
            }
        }
        guard.ensure_current(self.clock.now())?;
        Ok(())
    }
}

fn native_failure(error: CommandError) -> Failure {
    Failure::Forwarded(error)
}

fn recovery_result(enrollment: &KeyEnrollment) -> KeyRecoveryResult {
    use secrecy::ExposeSecret;
    KeyRecoveryResult {
        recovery_code: enrollment.recovery_code().expose_secret().to_owned(),
    }
}
