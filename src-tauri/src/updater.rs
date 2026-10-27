//! In-app updates (ADR 0020; REQ-OPS-006, NFR-21, NFR-24): the shell, never the engine,
//! contacts the project's GitHub releases, through the Tauri updater's HTTPS client only.
//!
//! - A build can update itself only when CI compiled in the release repository and the
//!   updater's Minisign public key ([`Updates::configured`]); local builds cannot.
//! - Official releases come from `releases/latest`, which never names a prerelease. Nightly
//!   builds come from the `nightly.json` index and are offered only while a nightly build
//!   runs, so a release installation never sees them.
//! - Every URL the shell fetches, including those the index and manifests name, must be a
//!   release download of that repository.
//! - Debug builds only: `VIBE_TOX_UPDATE_FEED_URL`, a loopback HTTP address, stands in for the
//!   repository so the screen can be tried locally without publishing ([`local_feed`]).
//!   Packages are still signature-checked, and installation is refused.
//! - Only a newer version installs. The updater checks the package's Minisign signature and
//!   the version it was signed for (`requireSignedVersion`) before the package is staged, and
//!   again before installation; the engine allows installation only under an approval naming
//!   the staged package's SHA-256.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use lis_core::{CommandError, CorrelationId, ErrorCode};
use lis_engine::{Engine, version_precedence};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, Url};
use tauri_plugin_updater::{Update, UpdaterExt};
use ts_rs::TS;

mod address;
mod errors;
mod feed;

use address::{local_feed, valid_file_name, valid_repository};

use errors::{internal, not_staged, refused, unavailable, updater_error};
pub(crate) use feed::{nightly_feed, release_feed};
use feed::{nightly_index, release_url};

/// The `owner/name` GitHub repository whose releases this build updates from, set by CI.
const REPOSITORY: Option<&str> = option_env!("VIBE_TOX_UPDATE_REPOSITORY");

/// How long a manifest or index request may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a package download may take.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Where an update comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub(crate) enum UpdateChannel {
    /// Official releases, published from `main`.
    Release,
    /// Nightly builds of `develop`, for internal testing.
    Nightly,
}

/// One published build, as the updates screen shows it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct AvailableBuild {
    pub(crate) channel: UpdateChannel,
    pub(crate) version: String,
    /// RFC 3339 publication time, when the publisher gave one.
    #[ts(optional)]
    pub(crate) published_at: Option<String>,
    /// The change summary lines, as published.
    pub(crate) changes: Vec<String>,
    /// The source commit, for nightly builds.
    #[ts(optional)]
    pub(crate) commit: Option<String>,
    /// Newer than the running version, so it can be installed.
    pub(crate) newer: bool,
}

/// What the updates screen knows about one channel.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct UpdateFeedView {
    /// The running version.
    pub(crate) running_version: String,
    /// This build can download and install updates.
    pub(crate) configured: bool,
    /// The channel is offered to this build: always for releases, only to nightly builds
    /// for nightly builds.
    pub(crate) offered: bool,
    /// Published builds, newest first; empty when none is published or not offered.
    pub(crate) builds: Vec<AvailableBuild>,
}

/// Input of `download_update`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub(crate) struct DownloadUpdateInput {
    pub(crate) channel: UpdateChannel,
    /// The version chosen on the screen; the download fails if the channel now names another.
    pub(crate) version: String,
}

/// A downloaded, signature-verified package waiting for approval and installation.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct DownloadedUpdateView {
    pub(crate) version: String,
    pub(crate) package_file_name: String,
}

/// The staged package.
struct Staged {
    view: DownloadedUpdateView,
    update: Update,
    path: PathBuf,
}

/// The shell's update state: where this build's releases are, and the staged package.
pub(crate) struct Updates {
    /// `https://github.com/<repository>/releases/`, or a local feed in a debug build; `None`
    /// when this build cannot update.
    releases: Option<Url>,
    /// The releases are a local feed: nothing is installed from it.
    local: bool,
    staged: Mutex<Option<Staged>>,
}

impl Updates {
    /// The update state for a build with `config`.
    pub(crate) fn new(config: &tauri::Config) -> Self {
        let pubkey = config
            .plugins
            .0
            .get("updater")
            .and_then(|updater| updater.get("pubkey"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|key| !key.trim().is_empty());
        if !pubkey {
            return Self::from_releases(None, false);
        }
        if let Some(feed) = local_feed() {
            tracing::warn!(%feed, "updates read from a local feed; installation is disabled");
            return Self::from_releases(Some(feed), true);
        }
        let releases = REPOSITORY
            .filter(|repository| valid_repository(repository))
            .and_then(|repository| {
                Url::parse(&format!("https://github.com/{repository}/releases/")).ok()
            });
        Self::from_releases(releases, false)
    }

    fn from_releases(releases: Option<Url>, local: bool) -> Self {
        Self {
            releases,
            local,
            staged: Mutex::new(None),
        }
    }

    /// Whether this build can update itself; the updater plugin is registered only then.
    pub(crate) fn configured(&self) -> bool {
        self.releases.is_some()
    }

    /// Whether `url` is a release download of this build's releases.
    fn owns(&self, url: &Url) -> bool {
        let Some(releases) = &self.releases else {
            return false;
        };
        url.query().is_none()
            && !url.path().contains("/../")
            && url
                .as_str()
                .strip_prefix(releases.as_str())
                .is_some_and(|rest| {
                    rest.starts_with("download/") || rest.starts_with("latest/download/")
                })
    }

    fn staged(&self) -> std::sync::MutexGuard<'_, Option<Staged>> {
        self.staged.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Downloads the package of `input.version` from its channel and verifies its signature,
/// then stages it in the app's cache folder in place of any earlier staged package.
pub(crate) async fn download<R: Runtime>(
    app: &AppHandle<R>,
    updates: &Updates,
    input: DownloadUpdateInput,
) -> Result<DownloadedUpdateView, CommandError> {
    let manifest_url = manifest_url(app, updates, &input).await?;
    // The updater's own comparison: only a newer version is an update.
    let update = app
        .updater_builder()
        .endpoints(vec![manifest_url])
        .map_err(|error| updater_error(&error, "update address not valid"))?
        .timeout(DOWNLOAD_TIMEOUT)
        .on_before_exit(shut_down_engine(app.clone()))
        .build()
        .map_err(|error| updater_error(&error, "updater not available"))?
        .check()
        .await
        .map_err(|error| updater_error(&error, "update check failed"))?
        .ok_or_else(|| refused("This version is not newer than the running version."))?;
    if update.version != input.version {
        return Err(refused(
            "The published version changed. Refresh and try again.",
        ));
    }
    if !updates.owns(&update.download_url) {
        tracing::warn!("update package address outside the release repository refused");
        return Err(refused(
            "The update package is not published by this project.",
        ));
    }
    let package_file_name = update
        .download_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| valid_file_name(name))
        .map(str::to_owned)
        .ok_or_else(|| refused("The update package name is not valid."))?;
    tracing::info!(version = %update.version, "downloading update");
    // `download` verifies the Minisign signature and the signed version.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|error| updater_error(&error, "update download failed"))?;
    let path = stage(app, &package_file_name, bytes).await?;
    let view = DownloadedUpdateView {
        version: update.version.clone(),
        package_file_name,
    };
    *updates.staged() = Some(Staged {
        view: view.clone(),
        update,
        path,
    });
    tracing::info!(version = %view.version, "update package verified and staged");
    Ok(view)
}

/// The manifest of `input.version` in its channel: the newest release, or the nightly index
/// entry of that version, offered only while a nightly build runs.
async fn manifest_url<R: Runtime>(
    app: &AppHandle<R>,
    updates: &Updates,
    input: &DownloadUpdateInput,
) -> Result<Url, CommandError> {
    let Some(releases) = &updates.releases else {
        return Err(unavailable());
    };
    match input.channel {
        UpdateChannel::Release => release_url(releases, "latest/download/latest.json"),
        UpdateChannel::Nightly if is_nightly(&running_version(app)) => {
            nightly_index(app, updates, releases)
                .await?
                .into_iter()
                .find(|entry| entry.version == input.version)
                .map(|entry| entry.manifest_url)
                .ok_or_else(|| refused("This nightly build is no longer published."))
        }
        UpdateChannel::Nightly => Err(refused(
            "Nightly builds are offered only to nightly builds.",
        )),
    }
}

/// Writes the verified package to the app's cache folder in place of any earlier one.
async fn stage<R: Runtime>(
    app: &AppHandle<R>,
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<PathBuf, CommandError> {
    let folder = app
        .path()
        .app_cache_dir()
        .map_err(|_| internal("cache folder not resolved"))?
        .join("updates");
    let path = folder.join(file_name);
    let written = tokio::task::spawn_blocking({
        let path = path.clone();
        move || -> std::io::Result<()> {
            // One staged package at a time.
            if folder.exists() {
                std::fs::remove_dir_all(&folder)?;
            }
            std::fs::create_dir_all(&folder)?;
            std::fs::write(&path, bytes)
        }
    })
    .await
    .map_err(|_| internal("staging task stopped"))?;
    written.map_err(|error| {
        tracing::warn!(error = ?error.kind(), "update package not staged");
        internal("update package not staged")
    })?;
    Ok(path)
}

/// The staged package's path when it is `version`.
pub(crate) fn staged_path(updates: &Updates, version: &str) -> Result<PathBuf, CommandError> {
    updates
        .staged()
        .as_ref()
        .filter(|staged| staged.view.version == version)
        .map(|staged| staged.path.clone())
        .ok_or_else(not_staged)
}

/// Installs the staged package of `version` after the engine allowed it, then restarts into
/// it. On Windows the installer replaces the app and starts it again; the engine is shut
/// down before the app exits. Elsewhere the app restarts through the ordinary exit path.
pub(crate) async fn install<R: Runtime>(
    app: &AppHandle<R>,
    engine: &Engine,
    updates: &Updates,
    version: &str,
) -> Result<(), CommandError> {
    if updates.local {
        return Err(refused(
            "Installation is disabled while updates come from a local test feed.",
        ));
    }
    let (update, path) = {
        let staged = updates.staged();
        let staged = staged
            .as_ref()
            .filter(|staged| staged.view.version == version)
            .ok_or_else(not_staged)?;
        (staged.update.clone(), staged.path.clone())
    };
    let warnings = engine.quit_warnings().await;
    if !warnings.is_empty() {
        return Err(refused(&format!(
            "Wait until these finish, then install: {}.",
            warnings.0.join("; ")
        )));
    }
    let approval = engine
        .begin_update_install(version.to_owned(), path.clone())
        .await?;
    tracing::info!(%version, "installing update");
    let installed = tokio::task::spawn_blocking(move || {
        // `install` checks the signature again before it runs the installer.
        std::fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| update.install(bytes).map_err(|error| error.to_string()))
    })
    .await
    .unwrap_or_else(|_| Err("install task stopped".to_owned()));
    if let Err(error) = installed {
        tracing::error!(%error, %version, "update not installed");
        engine
            .record_update_install_failure(approval, lis_engine::InstallFailure::Installer)
            .await?;
        return Err(CommandError::new(
            ErrorCode::Internal,
            "The update could not be installed. The running version is unchanged.",
            CorrelationId::generate(),
        ));
    }
    tracing::info!(%version, "update installed; restarting");
    *updates.staged() = None;
    crate::confirm_exit(app);
    app.request_restart();
    Ok(())
}

/// Shuts the engine down before the Windows installer replaces the app, which exits the
/// process without the ordinary exit path.
fn shut_down_engine<R: Runtime>(app: AppHandle<R>) -> impl Fn() + Send + Sync + 'static {
    move || {
        if let Some(engine) = app.try_state::<Engine>() {
            tauri::async_runtime::block_on(engine.shutdown());
        }
        tracing::info!("app stopped for update installation");
    }
}

fn running_version<R: Runtime>(app: &AppHandle<R>) -> String {
    app.package_info().version.to_string()
}

fn is_nightly(version: &str) -> bool {
    version.contains("-nightly.")
}

fn is_newer(candidate: &str, running: &str) -> bool {
    version_precedence(candidate, running) == Some(std::cmp::Ordering::Greater)
}

#[cfg(test)]
mod tests;
