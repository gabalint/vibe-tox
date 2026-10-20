//! Fresh authentication for signed actions (REQ-IDN-004, NFR-19): the
//! signed-in user confirms with their selected method inside the action's own
//! transaction, with the sign-in throttle and lockout.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_store::CommandContext;
use secrecy::SecretString;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::error::{Attempt, AuthError, Rejection};
use crate::repo;
use crate::sign_in::{Failure, password_problem, refuse, session_actor};
use crate::throttle;

/// Confirmation supplied to an existing sensitive command. Password strings
/// remain wire-compatible; key proofs are created only by the engine's frozen
/// action gateway and are never returned to the webview.
#[derive(Debug, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Reauthentication {
    /// A freshly entered password for a password/TOTP account.
    Password(#[ts(type = "string")] SecretString),
    /// Single-use proof of a verified key assertion for this action.
    SecurityKey {
        /// Opaque secret; do not log or expose it to the UI.
        #[serde(rename = "keyProof")]
        #[ts(type = "string")]
        key_proof: SecretString,
    },
}

impl From<SecretString> for Reauthentication {
    fn from(value: SecretString) -> Self {
        Self::Password(value)
    }
}

/// The signed action a re-authentication confirms, recorded as the audit
/// detail `purpose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReauthPurpose {
    /// Releasing a report revision.
    ReportRelease,
    /// Withdrawing a report.
    ReportWithdrawal,
    /// Exporting the recovery kit, which holds the installation key.
    RecoveryKitExport,
    /// Exporting every record of the laboratory (exit export).
    ExitExport,
    /// Closing the recovery reconciliation after a restore.
    RecoveryClose,
    /// Approving an application update.
    UpdateApproval,
    /// Accepting an installed version for clinical use.
    VersionAcceptance,
    /// Creating a user account.
    AccountCreation,
    /// Granting or revoking roles.
    RoleChange,
    /// Resetting another user's password or authenticator.
    CredentialReset,
    /// Approving every draft of a reviewed Catalog Preview (All Data Reviewed).
    CatalogReview,
    /// Changing the separation-of-duties policy.
    SeparationPolicy,
    /// Setting up a laboratory that one person runs.
    OnePersonLaboratory,
    /// Adding a personal key or moving a password account to keys.
    SecurityKeyEnrollment,
    /// Revoking a registered key.
    SecurityKeyRevocation,
    /// Replacing the saved offline recovery code.
    SecurityKeyRecovery,
    /// Enabling, replacing or disabling a personal session-unlock PIN.
    UnlockPinChange,
    /// Changing whether the laboratory permits touch-only key sign-in.
    SecurityKeyPolicy,
}

impl ReauthPurpose {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::ReportRelease => "report_release",
            Self::ReportWithdrawal => "report_withdrawal",
            Self::RecoveryKitExport => "recovery_kit_export",
            Self::ExitExport => "exit_export",
            Self::RecoveryClose => "recovery_close",
            Self::UpdateApproval => "update_approval",
            Self::VersionAcceptance => "version_acceptance",
            Self::AccountCreation => "account_creation",
            Self::RoleChange => "role_change",
            Self::CredentialReset => "credential_reset",
            Self::CatalogReview => "catalog_review",
            Self::SeparationPolicy => "separation_policy",
            Self::OnePersonLaboratory => "one_person_laboratory",
            Self::SecurityKeyEnrollment => "security_key_enrollment",
            Self::SecurityKeyRevocation => "security_key_revocation",
            Self::SecurityKeyRecovery => "security_key_recovery",
            Self::UnlockPinChange => "unlock_pin_change",
            Self::SecurityKeyPolicy => "security_key_policy",
        }
    }
}

/// Re-checks the signed-in user's selected method inside the caller's transaction
/// for each signed action (REQ-IDN-004 fresh authentication, NFR-19). Password
/// confirmations check the password; key confirmations consume a single-use,
/// session- and purpose-bound receipt issued by the trusted engine gateway. It
/// shares the sign-in throttle and lockout with [`crate::unlock`]. A wrong password is
/// [`Rejection::WrongPassword`] (`validation` on `password`); a disabled
/// account is [`Rejection::InvalidCredentials`], which ends the session in
/// the UI. A rejection commits its count and `auth.reauthenticate` failure
/// event, so the caller must commit and perform nothing else; success
/// writes `auth.reauthenticate` with the purpose, under the caller's
/// correlation ID.
pub fn reauthenticate(
    ctx: &mut CommandContext<'_>,
    confirmation: &Reauthentication,
    purpose: ReauthPurpose,
) -> Result<Attempt<()>, AuthError> {
    let (user_id, session_id) = session_actor(ctx)?;
    let user = repo::load(ctx.tx(), user_id)?;
    if let Reauthentication::SecurityKey { key_proof } = confirmation {
        return crate::key_proofs::consume_key_action_proof(ctx, &user, key_proof, purpose);
    }
    let Reauthentication::Password(password) = confirmation else {
        return Err(AuthError::SecurityKeyVerification);
    };
    let mut failure = Failure::of(actions::REAUTHENTICATE, &user);
    failure.purpose = Some(purpose);
    if failure.is_locked(ctx)? {
        return refuse(ctx, &failure, Rejection::LockedOut);
    }
    if let Some(reason_code) = password_problem(&user, password)? {
        failure.reason_code = reason_code;
        let rejection = if reason_code == "disabled" {
            Rejection::InvalidCredentials
        } else {
            Rejection::WrongPassword
        };
        return refuse(ctx, &failure, rejection);
    }
    throttle::clear(ctx.tx(), user.username.as_str())?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            actions::REAUTHENTICATE,
            Outcome::Succeeded,
        )
        .record("user", user.id)
        .detail("session_id", AuditDetail::id(session_id))
        .detail("purpose", AuditDetail::code(purpose.code())),
    )?;
    Ok(Attempt::Accepted(()))
}
