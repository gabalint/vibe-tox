# Branches and releases

Status: development builds without OS code signing, published by project-owner
instruction on 2026-10-20. They are for evaluation with synthetic data. They are not
validated for clinical use and are not supported releases. OS code signing, publisher
identity, support windows, security-response commitments and the binary's third-party
notice inventory remain open ([Q-19, Q-34](../planning/open-questions.md);
[G2-01, G2-03, G2-07](../quality/open-items-g1-g2.md)). In-app updates with
Minisign-signed update packages are accepted in
[ADR 0020](../architecture/decisions/0020-in-app-updates.md); see
[In-app updates](#in-app-updates).

## Branches

| Branch | Purpose | Receives |
| --- | --- | --- |
| `develop` | Default branch and integration line. Its newest commit becomes the `nightly` prerelease. | Day-to-day pushes and pull requests |
| `main` | Release line. Every new version on `main` becomes a `vX.Y.Z` release. | `develop` when a version is ready, and hotfixes |

CI runs on pushes to both branches and on pull requests. The
[Pages workflow](public-website.md#publication) publishes `website/` from `main`, so
presentation changes go live with the next merge to `main`.

## Versions

The Cargo workspace version is the application version. The running application
records it, and update approvals refer to it. `package.json` and
`package-lock.json` mirror it. Change it only with the version script, which also
updates the lockfiles and refuses manifests that disagree:

```bash
python3 scripts/release_version.py show         # print the version after checking every manifest
python3 scripts/release_version.py set 0.2.0    # write a new version everywhere
python3 scripts/release_version.py release      # print main's releasable version or the reason it is not
```

| Channel | Source | Version | GitHub release |
| --- | --- | --- | --- |
| Nightly | `develop` | `<next>-nightly.<UTC date>.<run number>`. `<next>` is the manifest version, or the patch after the latest release when that version is already released. The build stamps it; it is never committed. | A prerelease tagged `nightly`. Each new nightly deletes the previous one and moves the tag. With in-app updates, each nightly is also kept as a prerelease tagged `nightly-<UTC date>.<run number>`; the newest ten are kept. |
| Release | `main` | The manifest version as `major.minor.patch`. It must be untagged and higher than every `v*` tag. | `vX.Y.Z`, marked latest. Never replaced. |

A unique version for every nightly keeps builds distinguishable in the
application's version history and update records.

## Cut a release

1. On `develop`, run `python3 scripts/release_version.py set X.Y.Z`, then commit and
   push. The manifest change selects the full CI gate.
2. Merge `develop` into `main` through a pull request.
3. The scheduled release agent publishes it that night. To publish immediately, run
   `gh workflow run package.yml --ref main -f channel=release`.
4. If `main` received a hotfix that `develop` lacks, merge `main` back into `develop`.

If `main` changes without a new version, nothing is published. The release agent
reports the unreleased commits until a version is set.

## Package workflow

[`.github/workflows/package.yml`](../../.github/workflows/package.yml) runs only when
dispatched, with a `channel` input:

| Channel | Allowed ref | Gate | Result |
| --- | --- | --- | --- |
| `artifacts` (default) | Any | None | Unsigned bundles kept as workflow artifacts for 14 days |
| `nightly` | `develop` | Full CI gate | `nightly` prerelease |
| `release` | `main` | Full CI gate | `vX.Y.Z` release |

The packages are not code-signed for Windows or macOS (see below). When the
[updater signing key](#in-app-updates) is configured, the `nightly` and `release` channels
also sign the update packages with Minisign.

The gate calls the [`ci` workflow](../../.github/workflows/ci.yml), which selects
every check for a dispatched run. Bundles build in parallel on the
[packaging](packaging.md) matrix. The publish job runs only after the gate and every
build pass. It names the four installers `vibe-tox-lis_<version>_<os>-<arch>.<ext>`,
adds `SHA256SUMS.txt`, and writes notes with the development-build warning, the
unsigned-installer prompts and the workflow-run link. A release is created as a
draft and becomes visible only with every asset attached.

The macOS app uses an [ad-hoc bundle signature](packaging.md#macos-development-signatures),
and packaging fails if `codesign --verify --deep --strict` rejects it. This is still
an unsigned development channel in the publisher-trust sense: no Developer ID
certificate or notarization is configured, and the manual Gatekeeper exception
remains necessary. Browser-download and clean-host launch checks remain separate.

| Asset | Built on |
| --- | --- |
| `windows-x64-setup.exe` (NSIS, per-user) | `windows-latest` |
| `macos-arm64.dmg` | `macos-latest` (Apple silicon; Intel Macs are not built) |
| `linux-amd64.deb` and `linux-amd64.AppImage` | Ubuntu 22.04 |

With in-app updates configured, each release also carries `latest.json` and
`vibe-tox-lis_<version>_macos-arm64.app.tar.gz`, and the rolling `nightly` release also
carries `nightly.json`.

The rolling nightly needs a deletable tag, and so do the retained `nightly-*` builds.
Enabling GitHub's immutable releases would stop the workflow from replacing `nightly`.

## In-app updates

[ADR 0020](../architecture/decisions/0020-in-app-updates.md) (accepted 2026-10-27) lets the app check,
download and install updates from these releases, on **Versions and updates → Available
updates**. The [updates guide](../operations/updates.md) describes the operator steps.

| Channel | The app reads | Offered to |
| --- | --- | --- |
| Release | `releases/latest/download/latest.json`. GitHub's latest release, which is never a prerelease | Every configured build with a lower version |
| Nightly | `releases/download/nightly/nightly.json`: the newest build's manifest plus the retained builds, newest first. Each entry points to the `latest.json` of its own `nightly-*` release | Only builds that are themselves nightly builds |

Each `latest.json` is a Tauri updater manifest. It holds the version, the change summary
(the commit subjects since the previous build of that channel, merge commits excluded), the
publication time and, for each updater target, the package URL and its Minisign signature:

| Target | Package |
| --- | --- |
| `windows-x86_64-nsis` | `…_windows-x64-setup.exe` |
| `linux-x86_64-appimage` | `…_linux-amd64.AppImage` |
| `linux-x86_64-deb` | `…_linux-amd64.deb` |
| `darwin-aarch64-app` | `…_macos-arm64.app.tar.gz` |

[`scripts/update_feed.py`](../../scripts/update_feed.py) writes the manifest and the index,
and its tests run with the tooling checks. The workflow prints the tags that fall out of the
index and deletes those releases after publishing.

### Configure the signing key

Until both values below exist, the workflow publishes as before, without manifests. A build
from that time cannot update itself. Only the project owner or a person they name handles
the private key:

1. Generate a key pair once, on a trusted computer:
   `npx tauri signer generate -w ~/.tauri/vibe-tox-updater.key`. Keep the private key and its
   password offline and backed up. Losing them means every installed build must be updated
   by hand once to a build signed with a new key.
2. In the repository settings, add the secret `TAURI_SIGNING_PRIVATE_KEY` with the private
   key's contents, and the secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key has a
   password.
3. Add the repository variable `TAURI_UPDATER_PUBKEY` with the contents of the `.pub` file.
4. Run the `nightly` channel. Its notes then name `latest.json`, and the rolling `nightly`
   release carries `nightly.json`.

Setting only one of the two values fails the build, so a half-configured channel is never
published. Builds compiled with the public key accept only packages signed by the matching
private key. Changing the key therefore needs one manual installation, like the first
configured build.

The app's own requests, checks and installation rules are in ADR 0020. A local build has no
repository or public key compiled in, so it cannot update itself.

### Try the screen locally

A debug build can read a local feed instead of GitHub, so the screen can be checked without
publishing anything. Release builds, which CI publishes, ignore this.

1. Generate a throwaway key pair, outside the repository, with `npx tauri signer generate`.
   Sign synthetic package files with `npx tauri signer sign --app-version <version>`, and
   write their manifests and index with `scripts/update_feed.py`. A debug build has no
   bundle type, so each manifest also needs a plain `linux-x86_64` entry. Serve the folder
   over loopback HTTP in the `releases/` layout, for example with
   `python3 -m http.server 8765 --bind 127.0.0.1`.
2. Write a `--config` overlay outside the repository with a nightly `version`, a separate
   `identifier` such as `org.vibetox.lis.nightlytest` (so the laboratory's data folder and
   key-store entry are not touched) and `plugins.updater.pubkey` set to the throwaway public
   key.
3. Start the app with `VIBE_TOX_UPDATE_FEED_URL=http://127.0.0.1:8765/ npx tauri dev
   --config <overlay>`. Only a loopback `http` address is accepted.

Download, signature checks and approval work as published. Installation is refused while a
local feed is in use. The engine still records the workspace version from `Cargo.toml` under
**Installed versions**. Afterwards, delete the overlay identifier's data, cache and key-store
entries.

## Scheduled release agent

An agent on the maintainer's computer runs each night with the
[release-automation skill](../../.ai/skills/release-automation/SKILL.md). It
compares `develop` and `main` with the published releases. It then dispatches the
package workflow for whatever changed, reruns a run once after an infrastructure
failure, verifies the published assets and writes a report. It does not change
code, versions, branches, tags, settings or signing. Failures and unreleased
`main` commits wait for a person.

The computer needs `git`, Python 3.11 or newer, and `gh` authenticated with the
`repo` and `workflow` scopes. Preflight uses
`gh auth status --active --hostname github.com` to verify the active account;
when the operator names an account, it must match. Unrelated inactive credentials
do not block the run. The agent does not switch accounts or manually change credentials.
See [GitHub CLI authentication behavior](../research/sources-engineering.md#release-maintenance-sources-2026-10-23)
(ENG-305). Builds run on GitHub-hosted runners, so the agent does not compile locally.

## Source and license

Each release page links the source archives of its tagged commit. The workflow and
version script in that commit describe how it was built. A development build
does not satisfy the remaining [release requirements](packaging.md#other-release-requirements).
In particular, the installers do not yet carry a third-party license and notice
inventory (G2-07).
