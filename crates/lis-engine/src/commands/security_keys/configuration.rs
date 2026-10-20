//! Session-bound guards shared by personal PIN and laboratory authentication settings.

use lis_core::{Clock, UtcTimestamp};
use std::sync::Arc;

use super::CeremonyGuard;
use crate::commands::sign_in::session_of;
use crate::pipeline::{Call, Failure};
use crate::session::SessionState;
use crate::{ABSOLUTE_SESSION_LIFETIME, Engine};

pub(in crate::commands) struct ConfigurationGuard {
    authentication: CeremonyGuard,
    clock: Arc<dyn Clock>,
    session_started: UtcTimestamp,
}

impl ConfigurationGuard {
    pub(in crate::commands) fn ensure_current(&self) -> Result<(), lis_auth::AuthError> {
        let now = self.clock.now();
        self.authentication.ensure_current(now)?;
        if now.saturating_duration_since(self.session_started) >= ABSOLUTE_SESSION_LIFETIME {
            return Err(lis_auth::AuthError::SessionUnavailable);
        }
        Ok(())
    }
}

impl Engine {
    pub(in crate::commands) fn authentication_configuration_guard(
        &self,
        call: &Call,
    ) -> Result<ConfigurationGuard, Failure> {
        let guard = self.key_ceremonies.guard(self.clock.now());
        let (user_id, session_id) = session_of(&call.principal)?;
        let state = self.session_state();
        let SessionState::Active(session) = *state else {
            return Err(lis_auth::AuthError::ChallengeExpired.into());
        };
        if session.id != session_id || session.user_id != user_id {
            return Err(lis_auth::AuthError::ChallengeExpired.into());
        }
        Ok(ConfigurationGuard {
            authentication: guard,
            clock: Arc::clone(&self.clock),
            session_started: self.session_started(session),
        })
    }
}
