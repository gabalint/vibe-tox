//! In-app installation of a downloaded update (REQ-OPS-006, NFR-24; ADR 0020): the desktop
//! shell downloads and verifies the signed package, and the engine lets it install only the
//! exact package an approval names while that approval's pre-update backup is still verified
//! and recent. The start and any failure of the installation are audited.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::{CommandContext, StoreError};

use super::backup::ensure_pre_update_backup;
use super::{INSTALL_UPDATE, UpdateApprovalId, current_approval};
use crate::runtime_error::RuntimeError;

/// Why an installation that the engine allowed did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFailure {
    /// The installer could not be started or reported an error.
    Installer,
    /// The app could not restart into the new version.
    Restart,
}

impl InstallFailure {
    fn code(self) -> &'static str {
        match self {
            Self::Installer => "installer_failed",
            Self::Restart => "restart_failed",
        }
    }
}

/// Allows the installation of `version` from the package with `package_sha256`
/// (`manage_configuration`): its current approval must name that package, must not be
/// installed yet, and its pre-update backup must still be retained, verified and recent.
/// Audited as `ops.install_update`; returns the approval.
pub fn begin_update_install(
    ctx: &mut CommandContext<'_>,
    version: &str,
    package_sha256: &str,
) -> Result<UpdateApprovalId, RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    let approval = current_approval(ctx.tx(), version)?.ok_or(RuntimeError::InstallNotApproved)?;
    if approval.installed {
        return Err(RuntimeError::ApprovalInstalled);
    }
    if approval.package_sha256 != package_sha256 {
        return Err(RuntimeError::PackageNotApproved);
    }
    ensure_pre_update_backup(ctx.tx(), approval.pre_update_run_id, ctx.now())?;
    let digest = AuditDetail::sha256(package_sha256).map_err(StoreError::from)?;
    ctx.audit(
        AuditEvent::new(Stream::Security, INSTALL_UPDATE, Outcome::Succeeded)
            .record("update_approval", approval.approval_id)
            .detail("package_sha256", digest),
    )?;
    Ok(approval.approval_id)
}

/// Records that the installation allowed under `approval_id` failed
/// (`manage_configuration`). Audited as a failed `ops.install_update`.
pub fn record_update_install_failure(
    ctx: &mut CommandContext<'_>,
    approval_id: UpdateApprovalId,
    failure: InstallFailure,
) -> Result<(), RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    ctx.audit(
        AuditEvent::new(Stream::Security, INSTALL_UPDATE, Outcome::Failed)
            .record("update_approval", approval_id)
            .detail("failure", AuditDetail::code(failure.code())),
    )?;
    Ok(())
}
