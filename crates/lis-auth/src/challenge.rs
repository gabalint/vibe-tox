//! The sign-in step between a correct password and a session (NFR-19: there
//! is no single-factor sign-in). A challenge lives in memory only, belongs to
//! one user and account revision, expires after five minutes and is completed at
//! most once. The app has one window, so the slot holds at most one challenge:
//! issuing a new one discards the previous.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use lis_core::{Id, UserId, UtcTimestamp};
use serde::Serialize;
use ts_rs::TS;

use crate::account::Username;
use crate::error::AuthError;
use crate::totp::{TotpEnrollment, TotpSecret};

/// How long a challenge stays valid.
pub const CHALLENGE_TTL: Duration = Duration::from_secs(5 * 60);

/// Marker for [`ChallengeId`].
pub enum Challenge {}
/// Identifier of a sign-in challenge.
pub type ChallengeId = Id<Challenge>;

/// What the person must do next to finish signing in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SignInStep {
    /// The password is temporary: choose a new one.
    PasswordChangeRequired {
        /// The challenge to answer.
        challenge_id: ChallengeId,
    },
    /// No authenticator is enrolled yet: enrol this one and enter its code.
    MfaEnrollmentRequired {
        /// The challenge to answer.
        challenge_id: ChallengeId,
        /// The secret to enrol.
        enrollment: TotpEnrollment,
    },
    /// Enter the current authenticator code.
    MfaRequired {
        /// The challenge to answer.
        challenge_id: ChallengeId,
    },
}

/// A correct password for an active account, and what that account still needs.
#[derive(Debug, Clone)]
pub struct PasswordAccepted {
    pub(crate) user_id: UserId,
    pub(crate) account_revision: i64,
    pub(crate) username: Username,
    pub(crate) must_change_password: bool,
    pub(crate) mfa_enrolled: bool,
}

/// What a live challenge asks for.
#[derive(Debug, Clone)]
pub enum ChallengeStep {
    /// A new password replacing a temporary one.
    PasswordChange,
    /// The first code from this pending secret, which confirms its enrolment.
    Enrollment(PendingSecret),
    /// A code from the enrolled authenticator.
    Mfa,
}

/// A TOTP secret awaiting its first code; `Debug` redacts it.
#[derive(Debug, Clone)]
pub struct PendingSecret(pub(crate) TotpSecret);

/// A challenge that is still valid.
#[derive(Debug, Clone)]
pub struct LiveChallenge {
    /// Its identifier.
    pub id: ChallengeId,
    /// The one user it belongs to.
    pub user_id: UserId,
    /// The account revision whose password was checked; rechecked in the transaction.
    pub(crate) account_revision: i64,
    /// What it asks for.
    pub step: ChallengeStep,
    expires_at: UtcTimestamp,
}

/// The single in-memory challenge slot of the app window.
#[derive(Debug, Default)]
pub struct ChallengeSlot(Mutex<Option<LiveChallenge>>);

impl ChallengeSlot {
    /// Issues the challenge `accepted` needs next and describes it for the UI.
    pub fn issue(
        &self,
        accepted: &PasswordAccepted,
        now: UtcTimestamp,
    ) -> Result<SignInStep, AuthError> {
        let id = ChallengeId::generate();
        let (step, view) = if accepted.must_change_password {
            (
                ChallengeStep::PasswordChange,
                SignInStep::PasswordChangeRequired { challenge_id: id },
            )
        } else if accepted.mfa_enrolled {
            (
                ChallengeStep::Mfa,
                SignInStep::MfaRequired { challenge_id: id },
            )
        } else {
            let secret = TotpSecret::generate()?;
            let enrollment = secret.enrollment(&accepted.username)?;
            (
                ChallengeStep::Enrollment(PendingSecret(secret)),
                SignInStep::MfaEnrollmentRequired {
                    challenge_id: id,
                    enrollment,
                },
            )
        };
        let expires_at = now
            .checked_add(CHALLENGE_TTL)
            .ok_or(AuthError::TimeOutOfRange)?;
        *self.lock() = Some(LiveChallenge {
            id,
            user_id: accepted.user_id,
            account_revision: accepted.account_revision,
            step,
            expires_at,
        });
        Ok(view)
    }

    /// The challenge `id` if it is still valid at `now`. An expired challenge is discarded.
    pub fn live(&self, id: ChallengeId, now: UtcTimestamp) -> Result<LiveChallenge, AuthError> {
        let mut slot = self.lock();
        if slot.as_ref().is_some_and(|live| live.expires_at <= now) {
            *slot = None;
        }
        slot.as_ref()
            .filter(|live| live.id == id)
            .cloned()
            .ok_or(AuthError::ChallengeExpired)
    }

    /// Ends challenge `id` after it was answered; a later answer is refused.
    pub fn complete(&self, id: ChallengeId) {
        let mut slot = self.lock();
        if slot.as_ref().is_some_and(|live| live.id == id) {
            *slot = None;
        }
    }

    /// Discards any challenge (sign-out, lockout).
    pub fn clear(&self) {
        *self.lock() = None;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<LiveChallenge>> {
        // A poisoned slot still holds a coherent value.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> UtcTimestamp {
        UtcTimestamp::parse("2026-01-02T03:04:05.000000000Z")
            .unwrap()
            .checked_add(Duration::from_secs(seconds))
            .unwrap()
    }

    fn accepted(must_change_password: bool, mfa_enrolled: bool) -> PasswordAccepted {
        PasswordAccepted {
            user_id: UserId::generate(),
            account_revision: 1,
            username: Username::parse("jsmith").unwrap(),
            must_change_password,
            mfa_enrolled,
        }
    }

    fn id_of(step: &SignInStep) -> ChallengeId {
        match step {
            SignInStep::PasswordChangeRequired { challenge_id }
            | SignInStep::MfaEnrollmentRequired { challenge_id, .. }
            | SignInStep::MfaRequired { challenge_id } => *challenge_id,
        }
    }

    #[test]
    fn step_follows_the_account_state() {
        let slot = ChallengeSlot::default();
        let change = slot.issue(&accepted(true, false), at(0)).unwrap();
        assert!(matches!(change, SignInStep::PasswordChangeRequired { .. }));
        let enrol = slot.issue(&accepted(false, false), at(0)).unwrap();
        assert!(matches!(enrol, SignInStep::MfaEnrollmentRequired { .. }));
        let mfa = slot.issue(&accepted(false, true), at(0)).unwrap();
        assert!(matches!(mfa, SignInStep::MfaRequired { .. }));
    }

    #[test]
    fn challenge_expires_after_five_minutes() {
        let slot = ChallengeSlot::default();
        let id = id_of(&slot.issue(&accepted(false, true), at(0)).unwrap());
        assert!(slot.live(id, at(299)).is_ok());
        assert!(matches!(
            slot.live(id, at(300)),
            Err(AuthError::ChallengeExpired)
        ));
        assert!(
            slot.live(id, at(0)).is_err(),
            "an expired challenge is gone"
        );
    }

    #[test]
    fn challenge_is_single_use_and_replaced_by_the_next() {
        let slot = ChallengeSlot::default();
        let first = id_of(&slot.issue(&accepted(false, true), at(0)).unwrap());
        let second = id_of(&slot.issue(&accepted(false, true), at(0)).unwrap());
        assert!(slot.live(first, at(1)).is_err());
        slot.complete(second);
        assert!(slot.live(second, at(1)).is_err());
    }

    #[test]
    fn step_json_matches_the_ipc_contract() {
        let id = ChallengeId::generate();
        let json = serde_json::to_value(SignInStep::MfaRequired { challenge_id: id }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "mfaRequired", "challengeId": id.to_string()})
        );
    }
}
