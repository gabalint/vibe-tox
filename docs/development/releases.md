# Branches and releases

Status: unsigned development builds, published by project-owner instruction on
2026-10-20. They are for evaluation with synthetic data. They are not validated for
clinical use and are not supported releases. Signing, publisher identity, support
windows, security-response commitments, the updater and the binary's third-party
notice inventory remain open ([Q-19, Q-34](../planning/open-questions.md);
[G2-01, G2-03, G2-07](../quality/open-items-g1-g2.md)).

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
| Nightly | `develop` | `<next>-nightly.<UTC date>.<run number>`. `<next>` is the manifest version, or the patch after the latest release when that version is already released. The build stamps it; it is never committed. | A prerelease tagged `nightly`. Each new nightly deletes the previous one and moves the tag. |
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

The rolling nightly needs a deletable tag. Enabling GitHub's immutable releases
would stop the workflow from replacing `nightly`.

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
