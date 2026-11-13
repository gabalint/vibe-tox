//! Where updates may come from: the repository name CI compiles in, the repository a feed
//! read from it names after a rename, a debug build's local test feed, and the package file
//! names the shell stages.

use tauri::Url;

/// Debug builds only: the loopback HTTP address in `VIBE_TOX_UPDATE_FEED_URL`, laid out like
/// a repository's `releases/` path. Release builds never read it.
pub(super) fn local_feed() -> Option<Url> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let text = std::env::var("VIBE_TOX_UPDATE_FEED_URL").ok()?;
    let mut url = Url::parse(&text).ok()?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if url.scheme() != "http" || !loopback || url.query().is_some() {
        tracing::warn!("VIBE_TOX_UPDATE_FEED_URL ignored: not a loopback http address");
        return None;
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Some(url)
}

pub(super) fn valid_repository(repository: &str) -> bool {
    let mut parts = repository.split('/');
    let part = |part: Option<&str>| {
        part.is_some_and(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
    };
    part(parts.next()) && part(parts.next()) && parts.next().is_none()
}

pub(super) fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Whether `url` is a release download under `releases` (`…/releases/`): a `download/` or
/// `latest/download/` address with no query and no `..` segment.
pub(super) fn release_download(releases: &Url, url: &Url) -> bool {
    url.query().is_none()
        && !url.path().contains("/../")
        && url
            .as_str()
            .strip_prefix(releases.as_str())
            .is_some_and(|rest| {
                rest.starts_with("download/") || rest.starts_with("latest/download/")
            })
}

/// The `owner/name` repository a feed document names as its publisher (`"repository"`), when
/// it is a valid name. The workflow writes it into `latest.json` and `nightly.json`. After
/// the repository is renamed, GitHub redirects this build's compiled address to the new name,
/// so a document read from that address names the repository its builds are now under
/// (ADR 0020, amended 2026-11-13).
pub(super) fn declared_repository(document: &serde_json::Value) -> Option<&str> {
    document
        .get("repository")
        .and_then(serde_json::Value::as_str)
        .filter(|repository| valid_repository(repository))
}

/// The release root `https://github.com/<repository>/releases/` of a valid repository name.
pub(super) fn github_releases(repository: &str) -> Option<Url> {
    if !valid_repository(repository) {
        return None;
    }
    Url::parse(&format!("https://github.com/{repository}/releases/")).ok()
}
