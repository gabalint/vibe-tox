//! Optional personal PIN for the same locked, still-valid session (REQ-IDN-004).

use lis_auth::{
    Attempt, DisableUnlockPinInput, Rejection, SetUnlockPinInput, UnlockPinStatus,
    UnlockWithPinInput,
};
use lis_core::{CommandError, ErrorCode};

use super::sign_in::session_of;
use crate::pipeline::{Failure, accepted};
use crate::registry::{DISABLE_UNLOCK_PIN, GET_UNLOCK_PIN_STATUS, SET_UNLOCK_PIN, UNLOCK_WITH_PIN};
use crate::session::SessionState;
use crate::{Engine, SessionInfo};

impl Engine {
    /// Own PIN availability and attempts remaining; never exposes the verifier.
    pub async fn get_unlock_pin_status(&self) -> Result<UnlockPinStatus, CommandError> {
        self.run(&GET_UNLOCK_PIN_STATUS, |call| async move {
            let (user_id, session_id) = session_of(&call.principal)?;
            self.read(&call, move |conn| {
                lis_auth::get_unlock_pin_status(conn, user_id, session_id)
            })
            .await
        })
        .await
    }

    /// Enables or replaces the caller's PIN after normal fresh authentication.
    pub async fn set_unlock_pin(&self, input: SetUnlockPinInput) -> Result<(), CommandError> {
        self.run(&SET_UNLOCK_PIN, |call| async move {
            let guard = self.authentication_configuration_guard(&call)?;
            let key = self.data_key().await?;
            let attempt = self
                .write(&call, move |ctx| {
                    guard.ensure_current()?;
                    let result = lis_auth::set_unlock_pin(ctx, &key, input)?;
                    guard.ensure_current()?;
                    Ok::<_, lis_auth::AuthError>(result)
                })
                .await?;
            accepted(attempt)?;
            self.key_ceremonies.invalidate();
            Ok(())
        })
        .await
    }

    /// Removes the caller's PIN after normal fresh authentication.
    pub async fn disable_unlock_pin(
        &self,
        input: DisableUnlockPinInput,
    ) -> Result<(), CommandError> {
        self.run(&DISABLE_UNLOCK_PIN, |call| async move {
            let guard = self.authentication_configuration_guard(&call)?;
            let attempt = self
                .write(&call, move |ctx| {
                    guard.ensure_current()?;
                    let result = lis_auth::disable_unlock_pin(ctx, input)?;
                    guard.ensure_current()?;
                    Ok::<_, lis_auth::AuthError>(result)
                })
                .await?;
            accepted(attempt)?;
            self.key_ceremonies.invalidate();
            Ok(())
        })
        .await
    }

    /// Resumes only the window's same locked session. No new session or action proof.
    pub async fn unlock_with_pin(
        &self,
        input: UnlockWithPinInput,
    ) -> Result<SessionInfo, CommandError> {
        self.run(&UNLOCK_WITH_PIN, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let (user_id, session_id) = session_of(&call.principal)?;
            if !matches!(*self.session_state(), SessionState::Locked(session)
                if session.id == session_id && session.user_id == user_id)
            {
                return Err(Failure::refused(
                    ErrorCode::Conflict,
                    "The PIN can only unlock your current locked session.",
                ));
            }
            let key = self.data_key().await?;
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::unlock_with_pin(ctx, &key, &input.pin)
                })
                .await?;
            if matches!(
                attempt,
                Attempt::Rejected(Rejection::UnlockPinAttemptsExhausted)
            ) {
                // The domain transaction ended the session. Clear the window immediately.
                self.observe_session(call.correlation_id).await?;
            }
            let user = accepted(attempt)?;
            // Reuses the generation/expiry check and atomic activation used by native keys.
            self.finish_authenticated_session(&call, &guard, session_id, &user, false)
                .await
        })
        .await
    }
}
