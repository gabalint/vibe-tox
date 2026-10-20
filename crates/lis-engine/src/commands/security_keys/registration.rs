//! Atomic signup, migration and replacement-key recovery.

use std::sync::PoisonError;
use std::sync::atomic::Ordering;
use std::time::Duration;

use lis_auth::{FirstKeyAdministratorInput, ReauthPurpose};
use lis_core::{CommandError, ErrorCode};
use secrecy::ExposeSecret;
use subtle::ConstantTimeEq;

use super::*;
use crate::pipeline::{Failure, accepted};
use crate::{Engine, SessionInfo};

impl Engine {
    /// Direct personal-key signup: account and recovery material commit together.
    pub async fn security_key_signup(
        &self,
        input: SecurityKeySignupInput,
        client: &impl SecurityKeyClient,
    ) -> Result<KeyRecoveryResult, CommandError> {
        self.run(&SIGNUP, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            if input.load_demo_data.unwrap_or(false)
                && self.installation_kind != lis_ops::InstallationKind::Demo
            {
                return Err(Failure::refused(
                    ErrorCode::Conflict,
                    "Demo data can be prepared only in the separate demo laboratory.",
                ));
            }
            let details = FirstKeyAdministratorInput {
                username: input.username,
                display_name: input.display_name,
                one_person: input.one_person,
                load_demo_data: input.load_demo_data,
            };
            let account = self
                .read(&call, move |conn| {
                    lis_auth::prepare_first_key_registration(conn, details)
                })
                .await?;
            let verified = self
                .verify_key_registration(&call, &guard, account, client)
                .await?;
            let enrollment = self
                .write(&call, move |ctx| {
                    lis_auth::complete_first_key_registration(ctx, verified, &input.key_label)
                })
                .await?;
            let result = recovery_result(&enrollment);
            *self
                .key_ceremonies
                .pending_setup
                .lock()
                .unwrap_or_else(PoisonError::into_inner) =
                Some((enrollment, self.clock.now(), guard.generation));
            Ok(result)
        })
        .await
    }

    /// First setup alone may activate after the user acknowledges saving the recovery code.
    pub async fn acknowledge_key_recovery(
        &self,
        input: KeyRecoveryAcknowledgement,
    ) -> Result<SessionInfo, CommandError> {
        self.run(&ACKNOWLEDGE, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let (enrollment, started, generation) = self
                .key_ceremonies
                .pending_setup
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .ok_or(lis_auth::AuthError::ChallengeExpired)?;
            if self.clock.now().saturating_duration_since(started) > Duration::from_secs(15 * 60)
                || self.clock.now() < started
                || self.key_ceremonies.generation.load(Ordering::SeqCst) != generation
                || !bool::from(
                    enrollment
                        .recovery_code()
                        .expose_secret()
                        .as_bytes()
                        .ct_eq(input.recovery_code.expose_secret().as_bytes()),
                )
            {
                return Err(lis_auth::AuthError::ChallengeExpired.into());
            }
            let session = self.new_key_session();
            let session_id = session.id;
            let guard = CeremonyGuard {
                generation,
                current: std::sync::Arc::clone(&self.key_ceremonies.generation),
                started: self.clock.now(),
                elapsed: std::time::Instant::now(),
            };
            let user = self
                .write(&call, move |ctx| {
                    lis_auth::complete_key_enrollment_session(ctx, enrollment, &session)
                })
                .await?;
            self.finish_authenticated_session(&call, &guard, session_id, &user, true)
                .await
        })
        .await
    }

    /// Verify the current method first; remove password/TOTP only after the new key succeeds.
    pub async fn security_key_enroll(
        &self,
        input: SecurityKeyEnrollInput,
        client: &impl SecurityKeyClient,
    ) -> Result<KeyRecoveryResult, CommandError> {
        self.run(&ENROLL, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let (user_id, _) = session_of(&call.principal)?;
            let method = self
                .read(&call, move |conn| {
                    lis_auth::authentication_method(conn, user_id)
                })
                .await?;
            let confirmation = if method == lis_auth::AuthenticationMethod::SecurityKey {
                let credential_id = input
                    .credential_id
                    .ok_or(lis_auth::AuthError::SecurityKeyUnavailable)?;
                self.key_action_proof(
                    &call,
                    &guard,
                    credential_id,
                    ReauthPurpose::SecurityKeyEnrollment,
                    client,
                )
                .await?
            } else {
                input.password
            };
            let account = accepted(
                self.write(&call, move |ctx| {
                    lis_auth::prepare_own_key_registration(ctx, &confirmation)
                })
                .await?,
            )?;
            let verified = self
                .verify_key_registration(&call, &guard, account, client)
                .await?;
            let enrollment = self
                .write(&call, move |ctx| {
                    lis_auth::complete_own_key_registration(ctx, verified, &input.key_label)
                })
                .await?;
            Ok(recovery_result(&enrollment))
        })
        .await
    }

    /// A valid one-use code authorizes replacement enrollment, never a signed-in session.
    pub async fn security_key_recover(
        &self,
        input: SecurityKeyRecoveryInput,
        client: &impl SecurityKeyClient,
    ) -> Result<KeyRecoveryResult, CommandError> {
        self.run(&RECOVER, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let guard = self.key_ceremonies.guard(self.clock.now());
            let account = accepted(
                self.write(&call, move |ctx| {
                    lis_auth::prepare_key_recovery(ctx, &input.username, &input.recovery_code)
                })
                .await?,
            )?;
            let verified = self
                .verify_key_registration(&call, &guard, account, client)
                .await?;
            let enrollment = self
                .write(&call, move |ctx| {
                    lis_auth::complete_key_recovery(ctx, verified, &input.key_label)
                })
                .await?;
            self.key_ceremonies.invalidate();
            Ok(recovery_result(&enrollment))
        })
        .await
    }
    /// The shared native registration boundary; all account authority remains in its
    /// opaque prepared account and is rechecked by the final domain transaction.
    async fn verify_key_registration(
        &self,
        call: &crate::pipeline::Call,
        guard: &CeremonyGuard,
        account: lis_auth::KeyRegistrationAccount,
        client: &impl SecurityKeyClient,
    ) -> Result<lis_auth::VerifiedKeyRegistration, Failure> {
        let verifier = self.key_verifier()?;
        let (options, pending) = verifier.begin_registration(account)?;
        let response = client
            .register(self.key_origin(), options)
            .await
            .map_err(native_failure)?;
        self.check_key_guard(call, guard).await?;
        let verified = verifier.finish_registration(pending, response)?;
        client.finish().map_err(native_failure)?;
        Ok(verified)
    }
}
