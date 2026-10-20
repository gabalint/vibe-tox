//! Session authentication and private action proofs.

use lis_auth::{
    KeyAuthenticationPurpose, ReauthPurpose, Reauthentication, SecurityKeyId,
    VerifiedKeyAuthentication,
};
use lis_core::CommandError;

use super::{
    CeremonyGuard, SIGN_IN, SecurityKeyClient, SecurityKeySignInInput, SecurityKeyUnlockInput,
    UNLOCK, native_failure, session_of,
};
use crate::pipeline::{Call, Failure, accepted};
use crate::{Engine, SessionInfo};

impl Engine {
    /// Recheck after the queued write as well as after hardware. If a new session
    /// lost the race with lock/signout, close its database row without activating it.
    pub(in crate::commands) async fn finish_authenticated_session(
        &self,
        call: &Call,
        guard: &CeremonyGuard,
        session_id: lis_core::SessionId,
        user: &lis_auth::SessionUser,
        created: bool,
    ) -> Result<SessionInfo, Failure> {
        let result = self.check_key_guard(call, guard).await.and_then(|()| {
            if self.activate_if_current(session_id, user, guard.generation) {
                Ok(())
            } else {
                Err(lis_auth::AuthError::ChallengeExpired.into())
            }
        });
        if result.is_err() && created {
            self.write(call, move |ctx| {
                lis_auth::end_session(ctx, session_id, lis_auth::EndReason::SignedOut)
            })
            .await?;
        }
        result?;
        Ok(SessionInfo::new(user, false, self.security_policy()))
    }

    /// Completes a native key assertion and starts the individual account's session.
    pub async fn security_key_sign_in(
        &self,
        input: SecurityKeySignInInput,
        client: &impl SecurityKeyClient,
    ) -> Result<SessionInfo, CommandError> {
        self.run(&SIGN_IN, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let verified = self
                .verify_key(
                    &call,
                    &guard,
                    input,
                    KeyAuthenticationPurpose::SignInOrUnlock,
                    client,
                )
                .await?;
            let session = self.new_key_session();
            let session_id = session.id;
            client.finish().map_err(native_failure)?;
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::complete_key_sign_in(ctx, verified, &session)
                })
                .await?;
            let user = accepted(attempt)?;
            self.challenges.clear();
            self.finish_authenticated_session(&call, &guard, session_id, &user, true)
                .await
        })
        .await
    }

    /// Requires a fresh assertion by the locked session's same account.
    pub async fn security_key_unlock(
        &self,
        input: SecurityKeyUnlockInput,
        client: &impl SecurityKeyClient,
    ) -> Result<SessionInfo, CommandError> {
        self.run(&UNLOCK, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let (user_id, session_id) = session_of(&call.principal)?;
            let user = self
                .read(&call, move |conn| lis_auth::session_user(conn, user_id))
                .await?;
            let verified = self
                .verify_key(
                    &call,
                    &guard,
                    SecurityKeySignInInput {
                        username: user.username,
                        credential_id: input.credential_id,
                    },
                    KeyAuthenticationPurpose::SignInOrUnlock,
                    client,
                )
                .await?;
            client.finish().map_err(native_failure)?;
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::complete_key_unlock(ctx, verified)
                })
                .await?;
            let user = accepted(attempt)?;
            self.finish_authenticated_session(&call, &guard, session_id, &user, false)
                .await
        })
        .await
    }

    pub(super) async fn key_action_proof(
        &self,
        call: &Call,
        guard: &CeremonyGuard,
        credential_id: SecurityKeyId,
        purpose: ReauthPurpose,
        client: &impl SecurityKeyClient,
    ) -> Result<Reauthentication, Failure> {
        let (user_id, _) = session_of(&call.principal)?;
        let user = self
            .read(call, move |conn| lis_auth::session_user(conn, user_id))
            .await?;
        let verified = self
            .verify_key(
                call,
                guard,
                SecurityKeySignInInput {
                    username: user.username,
                    credential_id,
                },
                KeyAuthenticationPurpose::SensitiveAction,
                client,
            )
            .await?;
        accepted(
            self.write(call, move |ctx| {
                lis_auth::issue_key_action_proof(ctx, verified, purpose)
            })
            .await?,
        )
    }

    async fn verify_key(
        &self,
        call: &Call,
        guard: &CeremonyGuard,
        input: SecurityKeySignInInput,
        purpose: KeyAuthenticationPurpose,
        client: &impl SecurityKeyClient,
    ) -> Result<VerifiedKeyAuthentication, Failure> {
        let name = input.username.clone();
        let account = accepted(
            self.write(call, move |ctx| {
                lis_auth::prepare_key_authentication_for_purpose(
                    ctx,
                    &name,
                    input.credential_id,
                    purpose,
                )
            })
            .await?,
        )?;
        let verifier = self.key_verifier()?;
        let (options, pending) = verifier.begin_authentication(account)?;
        let response = client
            .authenticate(self.key_origin(), options)
            .await
            .map_err(native_failure)?;
        self.check_key_guard(call, guard).await?;
        match verifier.finish_authentication(pending, response) {
            Ok(verified) => Ok(verified),
            Err(_) => {
                let attempt = self
                    .write(call, move |ctx| {
                        lis_auth::record_key_failure(ctx, &input.username)
                    })
                    .await?;
                accepted(attempt)?;
                Err(lis_auth::AuthError::SecurityKeyVerification.into())
            }
        }
    }
}
