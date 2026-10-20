//! The restore wizard's in-memory sessions, staging and staged-restore view (REQ-OPS-002).
//! A session holds the installation key, the chosen source and the staged copy; it lives
//! only in process memory and is taken out while a step runs, so a failed step puts it
//! back unless it expired.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use lis_core::UtcTimestamp;
use lis_ops::InstallationKind;
use lis_ops::backup::BackupError;
use lis_ops::restore::{RestoreSession, RestoreSessionId, RestoreSource, StagedRestore};

use super::{RestoreChainView, StagedRestoreView};
use crate::commands::backup::FindingView;
use crate::engine::{Engine, MODULES, artifact_columns};
use crate::pipeline::Failure;

pub(super) struct RestoreProgress {
    pub(super) data_dir: PathBuf,
    pub(super) session: RestoreSession,
    pub(super) source: Option<RestoreSource>,
    pub(super) staged: Option<StagedRestore>,
}

fn sessions() -> &'static Mutex<HashMap<RestoreSessionId, RestoreProgress>> {
    static SESSIONS: OnceLock<Mutex<HashMap<RestoreSessionId, RestoreProgress>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn session_guard()
-> std::sync::MutexGuard<'static, HashMap<RestoreSessionId, RestoreProgress>> {
    sessions().lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) async fn ensure_new_installation(
    engine: &Engine,
    call: &crate::pipeline::Call,
) -> Result<(), Failure> {
    let new_installation = engine
        .read(call, |conn| {
            lis_auth::setup_status(conn).map(|status| status.needs_administrator)
        })
        .await?;
    if new_installation {
        Ok(())
    } else {
        Err(BackupError::InstallationInUse.into())
    }
}

/// A restore-wizard step that opens a folder dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestoreStep {
    /// `choose_restore_source`.
    ChooseSource,
    /// `check_restore_anchors`: needs a staged set.
    CheckAnchors,
}

/// The dialog preflight of a restore step (M8): an installation without users and a live
/// session, staged for the anchor check. Nothing is taken out of the session.
pub(crate) async fn preflight_restore_step(
    engine: &Engine,
    call: &crate::pipeline::Call,
    session: RestoreSessionId,
    step: RestoreStep,
) -> Result<(), Failure> {
    ensure_new_installation(engine, call).await?;
    ensure_live(
        session,
        engine.clock.now(),
        step == RestoreStep::CheckAnchors,
    )?;
    Ok(())
}

pub(super) fn take_progress(id: RestoreSessionId) -> Result<RestoreProgress, BackupError> {
    session_guard()
        .remove(&id)
        .ok_or(BackupError::RestoreSessionExpired)
}

/// The session `id` is live and, when `staged` is asked, has a staged set; nothing is taken.
fn ensure_live(id: RestoreSessionId, now: UtcTimestamp, staged: bool) -> Result<(), BackupError> {
    let sessions = session_guard();
    let progress = sessions
        .get(&id)
        .ok_or(BackupError::RestoreSessionExpired)?;
    progress.session.backup_key(now)?;
    if staged && progress.staged.is_none() {
        return Err(BackupError::RestoreNotVerified);
    }
    Ok(())
}

pub(super) fn put_progress(progress: RestoreProgress) {
    session_guard().insert(progress.session.id(), progress);
}

pub(super) fn put_unless_expired(progress: RestoreProgress, error: &BackupError) {
    if !matches!(error, BackupError::RestoreSessionExpired) {
        put_progress(progress);
    }
}

pub(super) async fn stage_progress(
    mut progress: RestoreProgress,
    set_id: String,
    anchor_roots: Vec<PathBuf>,
    now: UtcTimestamp,
    clock: Arc<dyn lis_core::Clock>,
    ops: Arc<crate::ops_worker::Operations>,
) -> Result<(RestoreProgress, StagedRestore, lis_ops::restore::SetSummary), Failure> {
    let Some(source) = progress.source.as_ref() else {
        put_progress(progress);
        return Err(BackupError::NoBackupsInFolder.into());
    };
    let Some(summary) = source.sets.iter().find(|set| set.set_id == set_id).cloned() else {
        put_progress(progress);
        return Err(BackupError::NotFound("backup set").into());
    };
    let source_root = source.root.clone();
    let scratch = lis_ops::restore::restore_staging_dir(&progress.data_dir);
    let columns = artifact_columns();
    let result = tokio::task::spawn_blocking(move || {
        let staged = progress.session.backup_key(now).and_then(|key| {
            lis_ops::restore::stage(&lis_ops::restore::StageInput {
                source_root: &source_root,
                set_id: Some(&set_id),
                key,
                scratch_dir: &scratch,
                modules: MODULES,
                artifact_columns: &columns,
                anchor_roots: &anchor_roots,
                keep_artifacts: true,
                clock,
                cancel: &ops.cancel,
            })
        });
        (progress, staged)
    })
    .await
    .map_err(|_| Failure::Internal("restore staging task stopped".to_owned()))?;
    progress = result.0;
    match result.1 {
        Ok(staged) => Ok((progress, staged, summary)),
        Err(error) => {
            put_unless_expired(progress, &error);
            Err(error.into())
        }
    }
}

/// Proposed REQ-OPS-016: refuses a staged set of the other installation kind, so a demo
/// backup never becomes a laboratory's data, nor the reverse; the session is kept.
pub(super) fn ensure_kind(
    progress: RestoreProgress,
    staged: &StagedRestore,
    installation: InstallationKind,
) -> Result<RestoreProgress, Failure> {
    match staged.ensure_kind(installation) {
        Ok(()) => Ok(progress),
        Err(error) => {
            put_progress(progress);
            Err(error.into())
        }
    }
}

pub(super) fn staged_view(
    staged: &StagedRestore,
    summary: &lis_ops::restore::SetSummary,
) -> StagedRestoreView {
    StagedRestoreView {
        set_id: staged.set_id.clone(),
        set_created_at: summary.created_at,
        app_version: summary.app_version.clone(),
        database_bytes: summary.database_bytes,
        artifact_count: summary.artifact_count,
        integrity_ok: staged.integrity_ok,
        chains: staged
            .chains
            .iter()
            .map(|chain| RestoreChainView {
                stream: chain.stream.as_str().to_owned(),
                events: chain.events,
                intact: chain.intact,
                head_matches_manifest: chain.head_matches_manifest,
            })
            .collect(),
        anchors: staged.anchors,
        lost_interval_until: staged.lost_interval_until,
        findings: staged
            .findings
            .list()
            .iter()
            .map(|finding| FindingView {
                code: finding.code.as_str().to_owned(),
                count: finding.count,
            })
            .collect(),
        can_confirm: staged.passed(),
    }
}
