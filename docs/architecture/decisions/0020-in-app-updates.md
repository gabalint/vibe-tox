# ADR 0020: In-app updates from the project's GitHub releases

Status: **Accepted by the project owner, 2026-10-27**, as relayed by a maintainer the owner
authorized to carry it out. The owner asked for official releases and nightly builds to be
checked, downloaded and installed from the desktop application. Acceptance covers the three
rule changes under [Decision](#decision): the updater's HTTP client, the Minisign update key
and nightly retention. Nightly installations keep the recorded approval and verified backup
(NFR-24); no exemption was granted. Owner: project owner and technical maintainer; the
laboratory owns its update policy.

## Context

The application recorded update approvals, pre-update backups and acceptance for clinical
use, but did not install updates ([updates guide](../../operations/updates.md)). Testers and
laboratories had to download installers from GitHub and install them by hand. The package
workflow already publishes a rolling `nightly` prerelease from `develop` and a `vX.Y.Z`
release from `main` ([branches and releases](../../development/releases.md)).

These rules govern any updater and stay in force:

- **REQ-OPS-006 and NFR-24.** Nothing installs without a recorded approval by an authorized
  administrator naming the package, and a verified pre-upgrade snapshot. The updater never
  checks or installs on a schedule. An unsigned, downgraded or mismatched package is refused.
- **NFR-21.** Update packages carry Minisign signatures that the Tauri updater verifies. The
  host also verifies a signed, expiring update manifest; this decision does not meet that
  part (see [Consequences](#consequences)).
- **Library policy.** It listed HTTP clients as not needed, because the local application
  made no HTTP calls.
- **Mandate.** Publication of development releases did not authorize signing (Q-19, Q-34).

## Decision

1. **The shell contacts the update host; the engine never does.** Only the desktop shell
   makes update requests, through the HTTPS client of `tauri-plugin-updater`. It uses the
   operating system's certificate store and adds no second HTTP client. Every address it
   fetches, including those in manifests and the nightly index, must be a release download
   of the repository compiled into the build. *(Library-policy change; accepted.)*
2. **Minisign update signatures.** The package workflow signs the Windows NSIS installer,
   the Linux AppImage and `.deb` and the macOS application archive with a project Minisign
   key. It runs only when the `TAURI_SIGNING_PRIVATE_KEY` secret and the
   `TAURI_UPDATER_PUBKEY` repository variable are both set. The public key is compiled into
   the build, and `requireSignedVersion` binds each signature to its version. Without both
   values the workflow publishes as before and the build cannot update itself. This key signs
   update packages only. It is not Windows Authenticode, Apple Developer ID or Linux package
   signing; those remain Q-19 and Q-34. *(Update-signing authorization; accepted.)*
3. **Separate channels.**
   - Official releases are read from `releases/latest/download/latest.json`. GitHub's latest
     release is never a prerelease, so a release build never receives a nightly build.
   - Nightly builds are read from the `nightly.json` index on the rolling `nightly`
     prerelease, and only while the running version is itself a nightly build.
   - Each nightly build is kept as its own prerelease, `nightly-<UTC date>.<run>`. The index
     lists the newest ten, and the workflow deletes older ones.
     *(Release retention change; accepted.)*
4. **Only newer versions install.** The updater installs only a version newer than the
   running one, by Semantic Versioning precedence. The database is migrated forward only, and
   an older build cannot open a database a newer one migrated.
5. **The existing control stays in front of every installation, nightly included.**
   - The administrator downloads the build. The updater verifies the signature and signed
     version, and the shell stages the package.
   - The administrator records the approval of that staged package in the approval dialog,
     with the release notes reviewed, a retained verified manual or pre-update backup from
     the last 24 hours, a note and fresh identity confirmation.
   - Installation requires `manage_configuration` and no work in progress. The engine
     re-hashes the staged package and requires a current approval naming the same SHA-256,
     not yet installed, whose backup is still verified and recent. It audits
     `ops.install_update` in the same transaction.
   - The updater verifies the signature again, installs and restarts the app. On Windows the
     engine shuts down before the installer replaces the app.
   - The new version then awaits acceptance for clinical use as before. A failed installation
     is audited, and the running version is unchanged.
6. **Requests only on demand.** The app checks when **Available updates** opens or on
   **Check again**, never on a schedule or at start-up.

## Alternatives

| Option | Why not chosen |
| --- | --- |
| A button that opens the GitHub release page | Leaves download, verification and installation manual. The project owner asked for installation from the app |
| Nightly installation without approval or backup | Contradicts NFR-24 ("every install needs a recorded lab approval and a verified pre-upgrade snapshot"). Changing that requirement for a development channel needs an explicit owner decision; this record does not assume one |
| A separate HTTP client for the nightly list | Adds a second TLS stack. The updater's client reads the index, which has the shape of a manifest |
| Nightly opt-in for release builds through a setting | A clinical installation could then receive development builds. Testers install one nightly build by hand instead |
| Checking at start-up | NFR-24 forbids scheduled checks, and a start-up check would contact GitHub from every laboratory without being asked |

## Consequences

- **Network access.** A laboratory computer contacts `github.com` only when a person opens
  **Available updates** or chooses **Check again**. Offline operation is unchanged
  ([ADR 0009](0009-single-computer-local-lis.md)).
- **First installation is manual.** Builds published before this change, and builds without
  the update key, cannot update themselves. Install one configured build by hand.
- **Linux.** A `.deb` installation asks for the computer's administrator password through
  `pkexec`. An AppImage is replaced in place and needs write access to its file.
- **macOS.** The archive is signed and listed, but macOS in-app installation is not yet
  exercised. The ad-hoc bundle signature and the Gatekeeper exception are unchanged.
- **Remaining gap (NFR-21, REQ-OPS-006, G2-03).** The manifest and nightly index are neither
  signed nor expiring. Signed versions and the newer-only rule stop a replayed or downgraded
  package. A withheld update (freeze) is not detected. Authenticode, Developer ID,
  notarization, the SBOM and provenance also remain open. The in-app updater does not make a
  development build a supported or clinically validated release.
- **Rollback.** Rollback remains a restore of the pre-update backup plus a reinstall of the
  previous package. The app does not automate it.

## Revisit when

- the project owner changes the three rule changes above, or decides that nightly
  installations for internal testing need no approval or backup;
- Q-19 or Q-34 settle publisher signing, a signed and expiring manifest, or the support
  window;
- the updater plugin, GitHub release limits or the repository change.
