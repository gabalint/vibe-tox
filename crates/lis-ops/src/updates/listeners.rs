//! The analyzer listeners around an in-app update (REQ-OPS-006, REQ-OPS-013): they stop
//! before the installer runs and start again in the new version, or at once when the
//! installation fails. Each step is audited with the update; no instrument's enabled
//! setting changes.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;

use crate::runtime_error::RuntimeError;

/// Audit action of stopping the analyzer listeners before an update is installed.
pub const STOP_LISTENERS_FOR_UPDATE: &str = "ops.stop_listeners_for_update";
/// Audit action of starting them again after an update.
pub const RESTART_LISTENERS_AFTER_UPDATE: &str = "ops.restart_listeners_after_update";

/// Why the listeners started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenerRestart {
    /// The new version started.
    NewVersion,
    /// The installation did not happen; the running version is unchanged.
    InstallNotCompleted,
}

impl ListenerRestart {
    fn code(self) -> &'static str {
        match self {
            Self::NewVersion => "new_version",
            Self::InstallNotCompleted => "install_not_completed",
        }
    }
}

/// Records that `stopped` analyzer listeners stopped for an update
/// (`manage_configuration`). Audited as `ops.stop_listeners_for_update`.
pub fn record_listeners_stopped(
    ctx: &mut CommandContext<'_>,
    stopped: usize,
) -> Result<(), RuntimeError> {
    ctx.require(Capability::ManageConfiguration)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Security,
            STOP_LISTENERS_FOR_UPDATE,
            Outcome::Succeeded,
        )
        .detail("listeners", AuditDetail::count(stopped)),
    )?;
    Ok(())
}

/// Records that the enabled listeners started again: `listening` opened and `failed`
/// did not. Audited as `ops.restart_listeners_after_update`, failed when any did not
/// open. The engine runs it as the signed-in user after a failed installation, and as a
/// system principal when the new version starts.
pub fn record_listeners_restarted(
    ctx: &mut CommandContext<'_>,
    restart: ListenerRestart,
    listening: usize,
    failed: usize,
) -> Result<(), RuntimeError> {
    let outcome = if failed == 0 {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    };
    ctx.audit(
        AuditEvent::new(Stream::Security, RESTART_LISTENERS_AFTER_UPDATE, outcome)
            .detail("cause", AuditDetail::code(restart.code()))
            .detail("listening", AuditDetail::count(listening))
            .detail("failed", AuditDetail::count(failed)),
    )?;
    Ok(())
}
