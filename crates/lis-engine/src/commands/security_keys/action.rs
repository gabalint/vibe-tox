//! Frozen, typed sensitive actions. Neither proofs nor caller-supplied paths cross IPC.

use super::{ACTION, SecurityKeyClient, native_failure};
use crate::Engine;
use crate::pipeline::{Call, Failure};
use crate::registry::CommandSpec;
use lis_auth::{ReauthPurpose, Reauthentication, SecurityKeyId};
use lis_core::{CommandError, ErrorCode};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Only these existing sensitive commands accept a fresh key confirmation.
#[derive(Deserialize, TS)]
#[serde(
    tag = "command",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[ts(export)]
pub enum SecurityKeyAction {
    /// Existing `create_user` command, with its immutable input.
    CreateUser(lis_auth::CreateUserInput),
    /// Existing `set_user_roles` command, with its immutable input.
    SetUserRoles(lis_auth::SetUserRolesInput),
    /// Existing `reset_user_credentials` command, with its immutable input.
    ResetUserCredentials(lis_auth::ResetUserCredentialsInput),
    /// Existing `designate_one_person_laboratory` command, with its immutable input.
    DesignateOnePersonLaboratory(lis_auth::DesignateOnePersonLaboratoryInput),
    /// Existing `set_separation_policy` command, with its immutable input.
    SetSeparationPolicy(lis_auth::SetSeparationPolicyInput),
    /// Existing `set_security_policy` command, including key-verification policy.
    SetSecurityPolicy(lis_auth::SetSecurityPolicyInput),
    /// Existing `release_report` command, with its immutable input.
    ReleaseReport(crate::ReleaseReportInput),
    /// Existing `withdraw_report` command, with its immutable input.
    WithdrawReport(crate::WithdrawReportInput),
    /// Existing `export_recovery_kit` command, with its immutable input.
    ExportRecoveryKit(crate::ExportRecoveryKitInput),
    /// Existing `request_exit_export` command, with its immutable input.
    RequestExitExport(crate::RequestExitExportInput),
    /// Existing `close_recovery_reconciliation` command, with its immutable input.
    CloseRecoveryReconciliation(crate::CloseRecoveryInput),
    /// Existing `record_update_approval` command, with its immutable input.
    RecordUpdateApproval(crate::RecordUpdateApprovalInput),
    /// Existing `approve_downloaded_update` command, for the shell's staged package.
    ApproveDownloadedUpdate(crate::RecordUpdateApprovalInput),
    /// Existing `accept_installed_version` command, with its immutable input.
    AcceptInstalledVersion(crate::AcceptInstalledVersionInput),
    /// Existing `approve_reviewed_catalog` command, with its immutable input.
    ApproveReviewedCatalog(crate::ApproveReviewedCatalogInput),
    /// Existing `revoke_security_key` command, with its immutable input.
    RevokeSecurityKey(super::RevokeSecurityKeyInput),
    /// Enables or changes the personal session-unlock PIN.
    SetUnlockPin(lis_auth::SetUnlockPinInput),
    /// Disables the personal session-unlock PIN.
    DisableUnlockPin(lis_auth::DisableUnlockPinInput),
}

/// Select a registered key and freeze one specific action for its confirmation.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SecurityKeyActionInput {
    /// The caller's own active key record.
    pub credential_id: SecurityKeyId,
    /// The exact requested action and input retained throughout hardware interaction.
    pub action: SecurityKeyAction,
}

impl SecurityKeyAction {
    fn purpose(&self) -> ReauthPurpose {
        match self {
            Self::CreateUser(_) => ReauthPurpose::AccountCreation,
            Self::SetUserRoles(_) => ReauthPurpose::RoleChange,
            Self::ResetUserCredentials(_) => ReauthPurpose::CredentialReset,
            Self::DesignateOnePersonLaboratory(_) => ReauthPurpose::OnePersonLaboratory,
            Self::SetSeparationPolicy(_) => ReauthPurpose::SeparationPolicy,
            Self::SetSecurityPolicy(_) => ReauthPurpose::SecurityKeyPolicy,
            Self::ReleaseReport(_) => ReauthPurpose::ReportRelease,
            Self::WithdrawReport(_) => ReauthPurpose::ReportWithdrawal,
            Self::ExportRecoveryKit(_) => ReauthPurpose::RecoveryKitExport,
            Self::RequestExitExport(_) => ReauthPurpose::ExitExport,
            Self::CloseRecoveryReconciliation(_) => ReauthPurpose::RecoveryClose,
            Self::RecordUpdateApproval(_) | Self::ApproveDownloadedUpdate(_) => {
                ReauthPurpose::UpdateApproval
            }
            Self::AcceptInstalledVersion(_) => ReauthPurpose::VersionAcceptance,
            Self::ApproveReviewedCatalog(_) => ReauthPurpose::CatalogReview,
            Self::RevokeSecurityKey(_) => ReauthPurpose::SecurityKeyRevocation,
            Self::SetUnlockPin(_) | Self::DisableUnlockPin(_) => ReauthPurpose::UnlockPinChange,
        }
    }
    fn spec(&self) -> Result<&'static CommandSpec, Failure> {
        let name = match self {
            Self::CreateUser(_) => "create_user",
            Self::SetUserRoles(_) => "set_user_roles",
            Self::ResetUserCredentials(_) => "reset_user_credentials",
            Self::DesignateOnePersonLaboratory(_) => "designate_one_person_laboratory",
            Self::SetSeparationPolicy(_) => "set_separation_policy",
            Self::SetSecurityPolicy(_) => "set_security_policy",
            Self::ReleaseReport(_) => "release_report",
            Self::WithdrawReport(_) => "withdraw_report",
            Self::ExportRecoveryKit(_) => "export_recovery_kit",
            Self::RequestExitExport(_) => "request_exit_export",
            Self::CloseRecoveryReconciliation(_) => "close_recovery_reconciliation",
            Self::RecordUpdateApproval(_) => "record_update_approval",
            Self::ApproveDownloadedUpdate(_) => "approve_downloaded_update",
            Self::AcceptInstalledVersion(_) => "accept_installed_version",
            Self::ApproveReviewedCatalog(_) => "approve_reviewed_catalog",
            Self::RevokeSecurityKey(_) => "revoke_security_key",
            Self::SetUnlockPin(_) => "set_unlock_pin",
            Self::DisableUnlockPin(_) => "disable_unlock_pin",
        };
        crate::registry::COMMANDS
            .iter()
            .find(|spec| spec.name == name)
            .ok_or_else(|| Failure::Internal("Unregistered security-key action".to_owned()))
    }
    fn confirm(&mut self, proof: Reauthentication) {
        match self {
            Self::CreateUser(input) => input.password = proof,
            Self::SetUserRoles(input) => input.password = proof,
            Self::ResetUserCredentials(input) => input.password = proof,
            Self::DesignateOnePersonLaboratory(input) => input.password = proof,
            Self::SetSeparationPolicy(input) => input.password = proof,
            Self::SetSecurityPolicy(input) => input.password = Some(proof),
            Self::ReleaseReport(input) => input.password = proof,
            Self::WithdrawReport(input) => input.password = proof,
            Self::ExportRecoveryKit(input) => input.password = proof,
            Self::RequestExitExport(input) => input.password = proof,
            Self::CloseRecoveryReconciliation(input) => input.password = proof,
            Self::RecordUpdateApproval(input) | Self::ApproveDownloadedUpdate(input) => {
                input.password = proof;
            }
            Self::AcceptInstalledVersion(input) => input.password = proof,
            Self::ApproveReviewedCatalog(input) => input.password = proof,
            Self::RevokeSecurityKey(input) => input.password = proof,
            Self::SetUnlockPin(input) => input.password = proof,
            Self::DisableUnlockPin(input) => input.password = proof,
        }
    }
}

impl Engine {
    /// Confirms and executes the same typed action. The shell supplies only a trusted
    /// native-dialog destination or staged package, never resubmitted by the webview.
    pub async fn security_key_action(
        &self,
        mut input: SecurityKeyActionInput,
        destination: Option<PathBuf>,
        client: &impl SecurityKeyClient,
    ) -> Result<Value, CommandError> {
        self.run(&ACTION, |call| async move {
            let _gate = self.key_ceremonies.enter()?;
            let Some(proof) = self
                .confirm_key_action(&call, &input, destination.as_deref(), client)
                .await?
            else {
                return Ok(Value::Null);
            };
            input.action.confirm(proof);
            match input.action {
                SecurityKeyAction::CreateUser(input) => encode(self.create_user(input).await),
                SecurityKeyAction::SetUserRoles(input) => encode(self.set_user_roles(input).await),
                SecurityKeyAction::ResetUserCredentials(input) => {
                    encode(self.reset_user_credentials(input).await)
                }
                SecurityKeyAction::DesignateOnePersonLaboratory(input) => {
                    encode(self.designate_one_person_laboratory(input).await)
                }
                SecurityKeyAction::SetSeparationPolicy(input) => {
                    encode(self.set_separation_policy(input).await)
                }
                SecurityKeyAction::SetSecurityPolicy(input) => {
                    encode(self.set_security_policy(input).await)
                }
                SecurityKeyAction::ReleaseReport(input) => encode(self.release_report(input).await),
                SecurityKeyAction::WithdrawReport(input) => {
                    encode(self.withdraw_report(input).await)
                }
                SecurityKeyAction::ExportRecoveryKit(input) => {
                    encode(self.export_recovery_kit(input, destination).await)
                }
                SecurityKeyAction::RequestExitExport(input) => {
                    encode(self.request_exit_export(input, destination).await)
                }
                SecurityKeyAction::CloseRecoveryReconciliation(input) => {
                    encode(self.close_recovery_reconciliation(input).await)
                }
                SecurityKeyAction::RecordUpdateApproval(input) => {
                    encode(self.record_update_approval(input, destination).await)
                }
                SecurityKeyAction::ApproveDownloadedUpdate(input) => {
                    let package = destination.ok_or(Failure::Internal(
                        "Downloaded update package was not supplied".to_owned(),
                    ))?;
                    encode(self.approve_downloaded_update(input, package).await)
                }
                SecurityKeyAction::AcceptInstalledVersion(input) => {
                    encode(self.accept_installed_version(input).await)
                }
                SecurityKeyAction::ApproveReviewedCatalog(input) => {
                    encode(self.approve_reviewed_catalog(input).await)
                }
                SecurityKeyAction::RevokeSecurityKey(input) => {
                    encode(self.revoke_security_key(input).await)
                }
                SecurityKeyAction::SetUnlockPin(input) => encode(self.set_unlock_pin(input).await),
                SecurityKeyAction::DisableUnlockPin(input) => {
                    encode(self.disable_unlock_pin(input).await)
                }
            }
        })
        .await
    }

    async fn confirm_key_action(
        &self,
        call: &Call,
        input: &SecurityKeyActionInput,
        destination: Option<&Path>,
        client: &impl SecurityKeyClient,
    ) -> Result<Option<Reauthentication>, Failure> {
        let guard = self.key_ceremonies.guard(self.clock.now());
        let action_call = Call {
            spec: input.action.spec()?,
            correlation_id: call.correlation_id,
            principal: call.principal.clone(),
        };
        self.authorize(&action_call).await?;
        if destination.is_none()
            && matches!(input.action, SecurityKeyAction::ApproveDownloadedUpdate(_))
        {
            return Err(Failure::refused(
                ErrorCode::Validation,
                "Download this version again before approving it.",
            ));
        }
        if destination.is_none()
            && matches!(
                input.action,
                SecurityKeyAction::ExportRecoveryKit(_)
                    | SecurityKeyAction::RequestExitExport(_)
                    | SecurityKeyAction::RecordUpdateApproval(_)
            )
        {
            return Ok(None);
        }
        let proof = self
            .key_action_proof(
                call,
                &guard,
                input.credential_id,
                input.action.purpose(),
                client,
            )
            .await?;
        self.check_key_guard(call, &guard).await?;
        client.finish().map_err(native_failure)?;
        Ok(Some(proof))
    }
}

fn encode<T: serde::Serialize>(result: Result<T, CommandError>) -> Result<Value, Failure> {
    serde_json::to_value(result.map_err(native_failure)?)
        .map_err(|_| Failure::Internal("Could not encode security-key action result".to_owned()))
}
