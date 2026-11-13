use tauri::Url;

use super::address::{declared_repository, valid_file_name, valid_repository};
use super::feed::{MAX_CHANGE_CHARS, MAX_CHANGES, change_lines};
use super::{Updates, is_newer, is_nightly};

fn updates(repository: &str) -> Updates {
    let releases = Url::parse(&format!("https://github.com/{repository}/releases/")).unwrap();
    Updates::from_releases(Some(releases), false)
}

#[test]
fn only_release_downloads_of_the_repository_are_fetched() {
    let updates = updates("example/tox");
    let owned = [
        "https://github.com/example/tox/releases/download/nightly/nightly.json",
        "https://github.com/example/tox/releases/latest/download/latest.json",
        "https://github.com/example/tox/releases/download/v0.2.0/a.exe",
    ];
    for url in owned {
        assert!(updates.owns(&Url::parse(url).unwrap()), "{url}");
    }
    let foreign = [
        "http://github.com/example/tox/releases/download/nightly/nightly.json",
        "https://github.com/other/tox/releases/download/nightly/nightly.json",
        "https://github.com/example/tox-fork/releases/download/v1/a.exe",
        "https://github.com/example/tox/archive/main.zip",
        "https://github.com:8443/example/tox/releases/download/v1/a.exe",
        "https://github.com/example/tox/releases/download/v1/a.exe?x=1",
        "https://objects.example.com/example/tox/releases/download/v1/a.exe",
    ];
    for url in foreign {
        assert!(!updates.owns(&Url::parse(url).unwrap()), "{url}");
    }
    let unconfigured = Updates::from_releases(None, false);
    assert!(!unconfigured.owns(&Url::parse(owned[0]).unwrap()));
}

#[test]
fn repository_and_file_names_are_checked() {
    assert!(valid_repository("openbga/steady-state-lims"));
    for bad in ["", "openbga", "a/b/c", "../x", "a/.git", "a b/c"] {
        assert!(!valid_repository(bad), "{bad}");
    }
    assert!(valid_file_name(
        "steady-state-lims_0.1.2_windows-x64-setup.exe"
    ));
    for bad in ["", ".hidden", "a/b", "a\\b", "a b.exe"] {
        assert!(!valid_file_name(bad), "{bad}");
    }
}

#[test]
fn only_newer_versions_are_offered_and_nightly_needs_a_nightly_build() {
    assert!(is_newer(
        "0.1.2-nightly.20261028.20",
        "0.1.2-nightly.20261027.19"
    ));
    assert!(is_newer("0.1.2", "0.1.2-nightly.20261028.20"));
    assert!(!is_newer("0.1.1", "0.1.2-nightly.20261028.20"));
    assert!(!is_newer("0.1.1", "0.1.1"));
    assert!(!is_newer("not a version", "0.1.1"));
    assert!(is_nightly("0.1.2-nightly.20261028.20"));
    assert!(!is_nightly("0.1.2"));
}

#[test]
fn change_lines_are_bounded_plain_lines() {
    let long = "x".repeat(500);
    let text = format!("- First change\n\n* Second change\n{long}");
    let lines = change_lines(text.lines());
    assert_eq!(lines[..2], ["First change", "Second change"]);
    assert_eq!(lines[2].chars().count(), MAX_CHANGE_CHARS);
    let many: Vec<String> = (0..80).map(|n| n.to_string()).collect();
    assert_eq!(
        change_lines(many.iter().map(String::as_str)).len(),
        MAX_CHANGES
    );
}

#[test]
fn a_build_without_the_public_key_cannot_update() {
    let config: tauri::Config = serde_json::from_value(serde_json::json!({
        "identifier": "org.example.test"
    }))
    .unwrap();
    assert!(!Updates::new(&config).configured());
}

#[test]
fn a_local_feed_owns_only_its_own_release_downloads() {
    let feed = Updates::from_releases(Some(Url::parse("http://127.0.0.1:8765/").unwrap()), true);
    assert!(feed.owns(&Url::parse("http://127.0.0.1:8765/download/nightly/nightly.json").unwrap()));
    for url in [
        "http://127.0.0.1:8766/download/nightly/nightly.json",
        "http://127.0.0.1:8765/other/a.deb",
        "https://github.com/openbga/steady-state-lims/releases/download/nightly/nightly.json",
    ] {
        assert!(!feed.owns(&Url::parse(url).unwrap()), "{url}");
    }
}

#[test]
fn a_renamed_repository_named_by_the_feed_is_accepted_too() {
    // Built as openbga/tox; the repository is now openbga/steady-state-lims (2026-11-13).
    let updates = updates("openbga/tox");
    let renamed = Url::parse(
        "https://github.com/openbga/steady-state-lims/releases/download/nightly-20261113.31/latest.json",
    )
    .unwrap();
    assert!(!updates.owns(&renamed));
    assert!(!updates.owns_or_declared(&renamed, None));
    assert!(updates.owns_or_declared(&renamed, Some("openbga/steady-state-lims")));
    let own = Url::parse("https://github.com/openbga/tox/releases/download/v1/a.exe").unwrap();
    assert!(updates.owns_or_declared(&own, Some("openbga/steady-state-lims")));
    // Only release downloads of the named repository, and only a valid name.
    for (url, declared) in [
        (
            "https://github.com/openbga/steady-state-lims/archive/main.zip",
            "openbga/steady-state-lims",
        ),
        (
            "https://github.com/openbga/steady-state-lims/releases/download/v1/a.exe?x=1",
            "openbga/steady-state-lims",
        ),
        (
            "https://github.com/other/repo/releases/download/v1/a.exe",
            "openbga/steady-state-lims",
        ),
        (
            "https://github.com/a/b/c/releases/download/v1/a.exe",
            "a/b/c",
        ),
        ("https://github.com/../x/releases/download/v1/a.exe", "../x"),
    ] {
        let url = Url::parse(url).unwrap();
        assert!(
            !updates.owns_or_declared(&url, Some(declared)),
            "{url} as {declared}"
        );
    }
}

#[test]
fn a_local_feed_or_an_unconfigured_build_accepts_no_named_repository() {
    let url = Url::parse("https://github.com/openbga/steady-state-lims/releases/download/v1/a.exe")
        .unwrap();
    let feed = Updates::from_releases(Some(Url::parse("http://127.0.0.1:8765/").unwrap()), true);
    assert!(!feed.owns_or_declared(&url, Some("openbga/steady-state-lims")));
    let unconfigured = Updates::from_releases(None, false);
    assert!(!unconfigured.owns_or_declared(&url, Some("openbga/steady-state-lims")));
}

#[test]
fn the_repository_a_feed_names_is_read_only_when_valid() {
    let named = serde_json::json!({ "repository": "openbga/steady-state-lims", "version": "1" });
    assert_eq!(
        declared_repository(&named),
        Some("openbga/steady-state-lims")
    );
    for document in [
        serde_json::json!({ "version": "1" }),
        serde_json::json!({ "repository": "openbga" }),
        serde_json::json!({ "repository": "a/b/c" }),
        serde_json::json!({ "repository": 7 }),
    ] {
        assert_eq!(declared_repository(&document), None, "{document}");
    }
}
