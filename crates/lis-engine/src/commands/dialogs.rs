//! Preflight of the commands whose path comes from a native dialog (M8; M4 D10). The
//! desktop shell asks [`Engine::preflight_dialog`] before it opens the dialog, so a
//! locked session, a missing capability or a restore onto an installation in use is
//! refused before anyone browses the file system. The preflight changes nothing; a
//! missing capability is audited as a denial of the command, as when the command runs.
//! The command still rechecks everything when it receives the chosen path.

use lis_core::CommandError;
use lis_ops::restore::RestoreSessionId;

use super::anchors::VERIFY_AUDIT_ANCHORS;
use super::backup::{
    ADD_BACKUP_TARGET, CHECK_RESTORE_ANCHORS, CHOOSE_RESTORE_SOURCE, EXPORT_RECOVERY_KIT,
    REQUEST_EXIT_EXPORT, RestoreStep, preflight_restore_step,
};
use super::file_templates::OPEN_LAYOUT_SAMPLE;
use super::operations::RECORD_UPDATE_APPROVAL;
use super::result_files::{CHOOSE_RESULT_FOLDER, LOAD_RESULT_FILE};
use crate::engine::Engine;
use crate::registry::CommandSpec;

/// A command that opens a native dialog in the desktop shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCommand {
    /// `add_backup_target`: a folder.
    AddBackupTarget,
    /// `export_recovery_kit`: a file to save.
    ExportRecoveryKit,
    /// `request_exit_export`: a folder.
    RequestExitExport,
    /// `verify_audit_anchors` with a chosen folder.
    VerifyAuditAnchors,
    /// `record_update_approval`: the update package.
    RecordUpdateApproval,
    /// `choose_restore_source` of a restore session: a folder.
    ChooseRestoreSource(RestoreSessionId),
    /// `check_restore_anchors` of a restore session: a folder.
    CheckRestoreAnchors(RestoreSessionId),
    /// `load_result_file`: a `.csv` or `.txt` result file.
    LoadResultFile,
    /// `choose_result_folder`: a folder to watch.
    ChooseResultFolder,
    /// `open_layout_sample`: a sample result file for the template page.
    OpenLayoutSample,
}

impl DialogCommand {
    fn spec(self) -> &'static CommandSpec {
        match self {
            Self::AddBackupTarget => &ADD_BACKUP_TARGET,
            Self::ExportRecoveryKit => &EXPORT_RECOVERY_KIT,
            Self::RequestExitExport => &REQUEST_EXIT_EXPORT,
            Self::VerifyAuditAnchors => &VERIFY_AUDIT_ANCHORS,
            Self::RecordUpdateApproval => &RECORD_UPDATE_APPROVAL,
            Self::ChooseRestoreSource(_) => &CHOOSE_RESTORE_SOURCE,
            Self::CheckRestoreAnchors(_) => &CHECK_RESTORE_ANCHORS,
            Self::LoadResultFile => &LOAD_RESULT_FILE,
            Self::ChooseResultFolder => &CHOOSE_RESULT_FOLDER,
            Self::OpenLayoutSample => &OPEN_LAYOUT_SAMPLE,
        }
    }
}

impl Engine {
    /// Refuses `command` now if it would be refused once its path is chosen: the session
    /// and capability of its registry entry and, for a restore step, the no-users gate
    /// and a live restore session (staged, for the anchor check). Changes nothing.
    pub async fn preflight_dialog(&self, command: DialogCommand) -> Result<(), CommandError> {
        self.run(command.spec(), |call| async move {
            self.authorize(&call).await?;
            match command {
                DialogCommand::ChooseRestoreSource(session) => {
                    preflight_restore_step(self, &call, session, RestoreStep::ChooseSource).await
                }
                DialogCommand::CheckRestoreAnchors(session) => {
                    preflight_restore_step(self, &call, session, RestoreStep::CheckAnchors).await
                }
                DialogCommand::AddBackupTarget
                | DialogCommand::ExportRecoveryKit
                | DialogCommand::RequestExitExport
                | DialogCommand::VerifyAuditAnchors
                | DialogCommand::RecordUpdateApproval
                | DialogCommand::LoadResultFile
                | DialogCommand::ChooseResultFolder => Ok(()),
                DialogCommand::OpenLayoutSample => self.require_template_access(&call).await,
            }
        })
        .await
    }
}
