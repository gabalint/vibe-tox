//! Update records (REQ-OPS-006, NFR-24; D13, §7) and media disposal records (REQ-OPS-003).

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::commands::people::{display_names, name_of};
use lis_auth::Reauthentication;
use lis_auth::{Attempt, ReauthPurpose};
use lis_core::CommandError;
use lis_ops::RuntimeError;
use lis_ops::backup::RunId;
use lis_ops::disposals::{MediaDisposalView, RecordMediaDisposalInput};
use lis_ops::updates::{
    AcceptVersionInput, InstalledVersionView, NewUpdateApproval, UpdateApprovalView,
    WithdrawUpdateApprovalInput,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::{
    ACCEPT_INSTALLED_VERSION, LIST_INSTALLED_VERSIONS, LIST_MEDIA_DISPOSALS, LIST_UPDATE_APPROVALS,
    OperationsError, RECORD_MEDIA_DISPOSAL, RECORD_UPDATE_APPROVAL, WITHDRAW_UPDATE_APPROVAL,
    invalid,
};
use crate::engine::Engine;
use crate::pipeline::{Failure, accepted};

/// Bytes read at a time while hashing a package.
const READ_CHUNK: usize = 1 << 20;

/// Input of `record_update_approval`; the package comes from a file dialog.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordUpdateApprovalInput {
    /// The version to install.
    pub version: String,
    /// The vendor's risk class, as published.
    pub risk_class_text: String,
    /// The approver read the release notes; must be true.
    pub release_notes_reviewed: bool,
    /// A retained verified manual or pre-update backup, completed within 24 hours.
    pub pre_update_run_id: RunId,
    /// Approval notes, recorded as the audit reason.
    pub notes: String,
    /// Fresh confirmation by the approver.
    pub password: Reauthentication,
}

/// Input of `accept_installed_version`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AcceptInstalledVersionInput {
    /// The running version.
    pub version: String,
    /// What was checked before clinical use.
    pub note: String,
    /// Fresh confirmation by the approver.
    pub password: Reauthentication,
}

impl Engine {
    /// `list_installed_versions` (session): every version that has run, newest first.
    pub async fn list_installed_versions(&self) -> Result<Vec<InstalledVersionView>, CommandError> {
        self.run(&LIST_INSTALLED_VERSIONS, |call| async move {
            self.read(&call, named_installed_versions).await
        })
        .await
    }

    /// `list_update_approvals` (`manage_configuration`): every approval, newest first.
    pub async fn list_update_approvals(&self) -> Result<Vec<UpdateApprovalView>, CommandError> {
        self.run(&LIST_UPDATE_APPROVALS, |call| async move {
            self.read(&call, named_update_approvals).await
        })
        .await
    }

    /// `record_update_approval` (`manage_configuration`, fresh authentication): `package` is
    /// the file the dialog returned, `None` when it was cancelled. The engine reads the
    /// package outside any transaction and records only its name and SHA-256.
    pub async fn record_update_approval(
        &self,
        input: RecordUpdateApprovalInput,
        package: Option<PathBuf>,
    ) -> Result<Option<UpdateApprovalView>, CommandError> {
        self.run(&RECORD_UPDATE_APPROVAL, |call| async move {
            self.authorize(&call).await?;
            let Some(package) = package else {
                return Ok(None);
            };
            let (package_file_name, package_sha256) = package_digest(package).await?;
            let approval = NewUpdateApproval {
                version: input.version,
                risk_class_text: input.risk_class_text,
                release_notes_reviewed: input.release_notes_reviewed,
                pre_update_run_id: input.pre_update_run_id,
                notes: input.notes,
                package_file_name,
                package_sha256,
            };
            let password = input.password;
            let attempt = self
                .write(&call, move |ctx| -> Result<_, OperationsError> {
                    match lis_auth::reauthenticate(ctx, &password, ReauthPurpose::UpdateApproval)? {
                        Attempt::Rejected(rejection) => Ok(Attempt::Rejected(rejection)),
                        Attempt::Accepted(()) => Ok(Attempt::Accepted(
                            lis_ops::updates::record_update_approval(ctx, &approval)?,
                        )),
                    }
                })
                .await?;
            accepted(attempt).map(Some)
        })
        .await
    }

    /// `withdraw_update_approval` (`manage_configuration`): before the version is installed.
    pub async fn withdraw_update_approval(
        &self,
        input: WithdrawUpdateApprovalInput,
    ) -> Result<UpdateApprovalView, CommandError> {
        self.run(&WITHDRAW_UPDATE_APPROVAL, |call| async move {
            self.write(&call, move |ctx| {
                lis_ops::updates::withdraw_update_approval(ctx, input)
            })
            .await
        })
        .await
    }

    /// `accept_installed_version` (`approve_configuration`, fresh authentication): the
    /// running version is accepted for clinical use.
    pub async fn accept_installed_version(
        &self,
        input: AcceptInstalledVersionInput,
    ) -> Result<InstalledVersionView, CommandError> {
        self.run(&ACCEPT_INSTALLED_VERSION, |call| async move {
            // Before fresh authentication, so a refusal counts no password attempt.
            self.authorize(&call).await?;
            let AcceptInstalledVersionInput {
                version,
                note,
                password,
            } = input;
            let attempt = self
                .write(&call, move |ctx| -> Result<_, OperationsError> {
                    match lis_auth::reauthenticate(
                        ctx,
                        &password,
                        ReauthPurpose::VersionAcceptance,
                    )? {
                        Attempt::Rejected(rejection) => Ok(Attempt::Rejected(rejection)),
                        Attempt::Accepted(()) => Ok(Attempt::Accepted(
                            lis_ops::updates::accept_installed_version(
                                ctx,
                                AcceptVersionInput { version, note },
                            )?,
                        )),
                    }
                })
                .await?;
            accepted(attempt)
        })
        .await
    }

    /// `record_media_disposal` (`backup_restore`).
    pub async fn record_media_disposal(
        &self,
        input: RecordMediaDisposalInput,
    ) -> Result<MediaDisposalView, CommandError> {
        self.run(&RECORD_MEDIA_DISPOSAL, |call| async move {
            self.write(&call, move |ctx| -> Result<_, RuntimeError> {
                lis_ops::disposals::record_media_disposal(ctx, input)
            })
            .await
        })
        .await
    }

    /// `list_media_disposals` (`backup_restore`): newest first.
    pub async fn list_media_disposals(&self) -> Result<Vec<MediaDisposalView>, CommandError> {
        self.run(&LIST_MEDIA_DISPOSALS, |call| async move {
            self.read(&call, named_media_disposals).await
        })
        .await
    }
}

/// The package's file name and SHA-256 (lowercase hex), streamed on a blocking thread.
async fn package_digest(package: PathBuf) -> Result<(String, String), Failure> {
    let read = tokio::task::spawn_blocking(move || -> io::Result<(String, String)> {
        let name = package
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok((name, sha256_of(&package)?))
    })
    .await
    .map_err(|_| Failure::Internal("package digest task stopped".to_owned()))?;
    read.map_err(|error| {
        tracing::warn!(error_kind = ?error.kind(), "update package not readable");
        invalid(
            "package",
            "unreadable",
            "The chosen package could not be read. Choose the package file again.",
        )
    })
}

fn sha256_of(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; READ_CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// Installed versions, with who accepted and approved each named on every read.
fn named_installed_versions(
    conn: &rusqlite::Connection,
) -> Result<Vec<InstalledVersionView>, RuntimeError> {
    let mut versions = lis_ops::updates::list_installed_versions(conn)?;
    let ids = versions.iter().flat_map(|version| {
        let accepted = version.acceptance.as_ref().map(|stamp| stamp.by.as_str());
        let approved = version.approval.as_ref().map(|a| a.approved_by.as_str());
        accepted.into_iter().chain(approved)
    });
    let names = display_names(conn, ids)?;
    for version in &mut versions {
        if let Some(stamp) = version.acceptance.as_mut() {
            stamp.by_display = name_of(&names, &stamp.by);
        }
        if let Some(approval) = version.approval.as_mut() {
            approval.approved_by_display = name_of(&names, &approval.approved_by);
        }
    }
    Ok(versions)
}

/// Update approvals, with who approved and withdrew each named on every read.
fn named_update_approvals(
    conn: &rusqlite::Connection,
) -> Result<Vec<UpdateApprovalView>, RuntimeError> {
    let mut approvals = lis_ops::updates::list_update_approvals(conn)?;
    let ids = approvals.iter().flat_map(|approval| {
        std::iter::once(approval.approved_by.as_str())
            .chain(approval.withdrawn.as_ref().map(|stamp| stamp.by.as_str()))
    });
    let names = display_names(conn, ids)?;
    for approval in &mut approvals {
        approval.approved_by_display = name_of(&names, &approval.approved_by);
        if let Some(stamp) = approval.withdrawn.as_mut() {
            stamp.by_display = name_of(&names, &stamp.by);
        }
    }
    Ok(approvals)
}

/// Media disposals, with who recorded each named on every read.
fn named_media_disposals(
    conn: &rusqlite::Connection,
) -> Result<Vec<MediaDisposalView>, RuntimeError> {
    let mut disposals = lis_ops::disposals::list_media_disposals(conn)?;
    let ids = disposals.iter().map(|row| row.recorded_by.as_str());
    let names = display_names(conn, ids)?;
    for row in &mut disposals {
        row.recorded_by_display = name_of(&names, &row.recorded_by);
    }
    Ok(disposals)
}
