//! Operations settings, the security policy and the separation-of-duties policy:
//! versioned laboratory decisions (D12, §8; ADR 0015).

use lis_auth::{
    SecurityPolicyView, SeparationPolicyView, SetSecurityPolicyInput, SetSeparationPolicyInput,
};
use lis_core::CommandError;
use lis_ops::settings::{OperationsSettingsView, SetOperationsSettingsInput};

use super::{
    GET_OPERATIONS_SETTINGS, GET_SECURITY_POLICY, GET_SEPARATION_POLICY, SET_OPERATIONS_SETTINGS,
    SET_SECURITY_POLICY, SET_SEPARATION_POLICY,
};
use crate::engine::Engine;
use crate::pipeline::accepted;

impl Engine {
    /// `get_operations_settings` (`manage_configuration`): the values in force, their
    /// version and the defaults.
    pub async fn get_operations_settings(&self) -> Result<OperationsSettingsView, CommandError> {
        self.run(&GET_OPERATIONS_SETTINGS, |call| async move {
            self.read(&call, lis_ops::settings::get_operations_settings)
                .await
        })
        .await
    }

    /// `set_operations_settings` (`manage_configuration`): a new version, with a reason
    /// after the first (`ops.set_settings`). The loops read it at their next step.
    pub async fn set_operations_settings(
        &self,
        input: SetOperationsSettingsInput,
    ) -> Result<OperationsSettingsView, CommandError> {
        self.run(&SET_OPERATIONS_SETTINGS, |call| async move {
            self.write(&call, move |ctx| {
                lis_ops::settings::set_operations_settings(ctx, input)
            })
            .await
        })
        .await
    }

    /// `get_security_policy` (`manage_users`): the idle timeout and lockout in force.
    pub async fn get_security_policy(&self) -> Result<SecurityPolicyView, CommandError> {
        self.run(&GET_SECURITY_POLICY, |call| async move {
            self.read(&call, lis_auth::get_security_policy).await
        })
        .await
    }

    /// `set_security_policy` (`manage_users`; NFR-19): a new version, with a reason after
    /// the first (`auth.set_security_policy`). The engine's idle timeout follows at once.
    pub async fn set_security_policy(
        &self,
        input: SetSecurityPolicyInput,
    ) -> Result<SecurityPolicyView, CommandError> {
        self.run(&SET_SECURITY_POLICY, |call| async move {
            let guard = self.authentication_configuration_guard(&call)?;
            let previous = self.security_policy();
            let attempt = self
                .write(&call, move |ctx| {
                    guard.ensure_current()?;
                    let result = lis_auth::set_security_policy(ctx, input)?;
                    guard.ensure_current()?;
                    Ok::<_, lis_auth::AuthError>(result)
                })
                .await?;
            let view = accepted(attempt)?;
            self.refresh_security_policy(view.values);
            if previous.allow_security_key_touch_only != view.values.allow_security_key_touch_only {
                self.key_ceremonies.invalidate();
            }
            Ok(view)
        })
        .await
    }

    /// `get_separation_policy` (configuration approval or quality; ADR 0018): which duties one person may
    /// do alone.
    pub async fn get_separation_policy(&self) -> Result<SeparationPolicyView, CommandError> {
        self.run(&GET_SEPARATION_POLICY, |call| async move {
            self.read(&call, lis_auth::get_separation_policy).await
        })
        .await
    }

    /// `set_separation_policy` (`approve_configuration`, the actor's password again, a reason;
    /// ADR 0015): a new version (`auth.set_separation_policy`). The owning modules read it
    /// inside each approval's transaction.
    pub async fn set_separation_policy(
        &self,
        input: SetSeparationPolicyInput,
    ) -> Result<SeparationPolicyView, CommandError> {
        self.run(&SET_SEPARATION_POLICY, |call| async move {
            let attempt = self
                .write(&call, move |ctx| {
                    lis_auth::set_separation_policy(ctx, input)
                })
                .await?;
            accepted(attempt)
        })
        .await
    }
}
