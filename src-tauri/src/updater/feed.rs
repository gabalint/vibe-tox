//! What the release repository publishes: the newest official release and the nightly
//! index, read through the updater's client without comparing versions. The index and the
//! change lines are untrusted input; entries outside the repository are dropped.

use lis_core::CommandError;
use lis_engine::version_precedence;
use serde::Deserialize;
use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_updater::UpdaterExt;

use super::errors::{internal, refused, updater_error};
use super::{
    AvailableBuild, CHECK_TIMEOUT, UpdateChannel, UpdateFeedView, Updates, is_newer, is_nightly,
    running_version,
};

/// Most nightly builds and change lines shown; the index is untrusted input.
const MAX_BUILDS: usize = 20;
pub(super) const MAX_CHANGES: usize = 50;
pub(super) const MAX_CHANGE_CHARS: usize = 200;

/// A nightly index entry, as published; checked before use.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IndexEntry {
    pub(super) version: String,
    published_at: Option<String>,
    commit: Option<String>,
    #[serde(default)]
    changes: Vec<String>,
    pub(super) manifest_url: Url,
}

/// The newest official release, from `releases/latest`.
pub(crate) async fn release_feed<R: Runtime>(
    app: &AppHandle<R>,
    updates: &Updates,
) -> Result<UpdateFeedView, CommandError> {
    let running = running_version(app);
    let Some(releases) = &updates.releases else {
        return Ok(feed(running, false, true, Vec::new()));
    };
    let url = release_url(releases, "latest/download/latest.json")?;
    let Some(manifest) = fetch_manifest(app, url).await? else {
        return Ok(feed(running, true, true, Vec::new()));
    };
    let build = AvailableBuild {
        channel: UpdateChannel::Release,
        newer: is_newer(&manifest.version, &running),
        version: manifest.version,
        published_at: manifest.date,
        changes: change_lines(manifest.body.as_deref().unwrap_or_default().lines()),
        commit: None,
    };
    Ok(feed(running, true, true, vec![build]))
}

/// The retained nightly builds, from the `nightly.json` index; offered only to nightly builds.
pub(crate) async fn nightly_feed<R: Runtime>(
    app: &AppHandle<R>,
    updates: &Updates,
) -> Result<UpdateFeedView, CommandError> {
    let running = running_version(app);
    let offered = is_nightly(&running);
    let Some(releases) = updates.releases.as_ref().filter(|_| offered) else {
        return Ok(feed(running, updates.configured(), offered, Vec::new()));
    };
    let builds = nightly_index(app, updates, releases)
        .await?
        .into_iter()
        .map(|entry| AvailableBuild {
            channel: UpdateChannel::Nightly,
            newer: is_newer(&entry.version, &running),
            version: entry.version,
            published_at: entry.published_at,
            changes: change_lines(entry.changes.iter().map(String::as_str)),
            commit: entry.commit,
        })
        .collect();
    Ok(feed(running, true, true, builds))
}

/// A fetched manifest: the parts the screen shows, and the whole document.
struct Manifest {
    version: String,
    date: Option<String>,
    body: Option<String>,
    raw: serde_json::Value,
}

/// Fetches the update manifest or index at `url` without comparing versions; `None` when
/// nothing is published there.
async fn fetch_manifest<R: Runtime>(
    app: &AppHandle<R>,
    url: Url,
) -> Result<Option<Manifest>, CommandError> {
    let checked = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|error| updater_error(&error, "update address not valid"))?
        .timeout(CHECK_TIMEOUT)
        // Every published build is listed; installation compares versions again.
        .version_comparator(|_, _| true)
        .build()
        .map_err(|error| updater_error(&error, "updater not available"))?
        .check()
        .await;
    match checked {
        Ok(Some(update)) => Ok(Some(Manifest {
            date: update
                .raw_json
                .get("pub_date")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            version: update.version,
            body: update.body,
            raw: update.raw_json,
        })),
        Ok(None) | Err(tauri_plugin_updater::Error::ReleaseNotFound) => Ok(None),
        Err(error) => Err(updater_error(&error, "update check failed")),
    }
}

/// The nightly index's entries that are valid and published by this repository.
pub(super) async fn nightly_index<R: Runtime>(
    app: &AppHandle<R>,
    updates: &Updates,
    releases: &Url,
) -> Result<Vec<IndexEntry>, CommandError> {
    let url = release_url(releases, "download/nightly/nightly.json")?;
    let Some(manifest) = fetch_manifest(app, url).await? else {
        return Ok(Vec::new());
    };
    let entries: Vec<IndexEntry> = manifest
        .raw
        .get("builds")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| refused("The nightly build list is not valid."))?
        .unwrap_or_default();
    let total = entries.len();
    let valid: Vec<IndexEntry> = entries
        .into_iter()
        .filter(|entry| version_precedence(&entry.version, &entry.version).is_some())
        .filter(|entry| updates.owns(&entry.manifest_url))
        .take(MAX_BUILDS)
        .collect();
    if valid.len() < total.min(MAX_BUILDS) {
        tracing::warn!(
            dropped = total - valid.len(),
            "nightly index entries refused"
        );
    }
    Ok(valid)
}

/// A release address under `releases`, such as `latest/download/latest.json`.
pub(super) fn release_url(releases: &Url, path: &str) -> Result<Url, CommandError> {
    releases
        .join(path)
        .map_err(|_| internal("update address not valid"))
}

fn feed(
    running: String,
    configured: bool,
    offered: bool,
    builds: Vec<AvailableBuild>,
) -> UpdateFeedView {
    UpdateFeedView {
        running_version: running,
        configured,
        offered,
        builds,
    }
}

/// Non-empty change lines, bounded in number and length.
pub(super) fn change_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    lines
        .map(|line| line.trim().trim_start_matches(['-', '*']).trim())
        .filter(|line| !line.is_empty())
        .take(MAX_CHANGES)
        .map(|line| line.chars().take(MAX_CHANGE_CHARS).collect())
        .collect()
}
