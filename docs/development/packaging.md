# Packaging the desktop application

Status: M5 developer configuration for unsigned bundles. They are published as development
builds ([branches and releases](releases.md)). They are not a production release and are not
validated for clinical use. Code signing, notarization and the updater remain blocked on the
publisher and supported-release decisions in [Q-19 and Q-34](../planning/open-questions.md).

## Configured bundles

Tauri automatically merges the matching `src-tauri/tauri.<platform>.conf.json` file into the main
configuration. The common bundle metadata is in `src-tauri/tauri.conf.json`; each overlay selects
only the formats for its host OS. This follows Tauri's
[platform-specific configuration](https://v2.tauri.app/reference/config/#platform-specific-configuration)
and [bundle configuration](https://v2.tauri.app/reference/config/#bundleconfig), checked 2026-10-13.

| OS | Outputs | Installation and contents | Deliberate constraints |
| --- | --- | --- | --- |
| Windows x64 | NSIS `-setup.exe` | Installs for the current user under `%LOCALAPPDATA%`, so routine installation does not require administrator privileges. It includes the WebView2 offline installer. | The offline WebView2 choice permits installation without internet and adds about 127 MB. Tauri recommends it for offline environments. No Authenticode certificate, timestamp server or signing command is configured. |
| macOS Apple silicon or x64 | `.app` and `.dmg` | The `.app` is the application bundle. The `.dmg` wraps it in the usual drag-to-Applications window. `LSMinimumSystemVersion` is 15.0, matching the current-plus-two-previous-major table in `lis-ops` as of 2026-10-13. | No Developer ID identity or notarization provider is configured. Tauri may apply an identity-free ad-hoc signature, which supplies no distribution trust. A downloaded artifact may be refused by Gatekeeper; it is for build verification only. |
| Linux x64 | `.deb` and AppImage | The Debian package explicitly depends on `libwebkit2gtk-4.1-0` and `libgtk-3-0`. The AppImage carries most application libraries for a portable launch, but still inherits compatibility constraints from the build host. | Build on Ubuntu 22.04, the oldest CI baseline used here, to avoid unnecessarily raising the glibc floor. The supported deployment baseline remains the project OS-support policy, such as Ubuntu 24.04 LTS. |

The Windows choices follow Tauri's [NSIS install-mode and WebView2 guidance](https://v2.tauri.app/distribute/windows-installer/).
The macOS formats follow the [application-bundle](https://v2.tauri.app/distribute/macos-application-bundle/)
and [DMG](https://v2.tauri.app/distribute/dmg/) guides. Tauri documents WebKitGTK 4.1 and GTK 3
as the stock Debian runtime dependencies and recommends building Linux bundles on the oldest
intended compatible base system; see the [Debian](https://v2.tauri.app/distribute/debian/) and
[AppImage](https://v2.tauri.app/distribute/appimage/) guides. These official sources were checked
2026-10-13.

RPM is not configured. The project has not selected a supported RPM-family distribution, and its
WebKitGTK 4.1 runtime package name is distribution-specific. Add RPM only after choosing that
baseline and verifying the generated requirements on a real supported host.

## Local builds

Install the platform prerequisites from Tauri's
[current prerequisites guide](https://v2.tauri.app/start/prerequisites/), then run `npm ci` from the
repository root. Build only on the target OS; these commands do not cross-compile the installers.

Also install the [native authentication build tools](README.md#toolchain). OpenSSL is
built from the locked vendored source and statically linked; the installer must not
depend on a developer's Homebrew library or an unbundled OpenSSL DLL. Both CI matrices
select this feature, and the packaging job inspects the final executable's direct
dependencies with `otool`, `readelf` or `dumpbin` before uploading. These configuration
and inspection steps still require a successful build on each target OS and a clean-host
installation test; local macOS checks do not qualify Windows or Linux packages.

| Host | Command | Output below `$CARGO_TARGET_DIR/release/bundle/` |
| --- | --- | --- |
| Windows | `npx tauri build --bundles nsis -- --locked` | `nsis/` |
| macOS | `npx tauri build --bundles app,dmg -- --locked` | `macos/` and `dmg/` |
| Ubuntu 22.04 or compatible Debian build host | `npx tauri build --bundles deb,appimage -- --locked` | `deb/` and `appimage/` |

Set `CARGO_TARGET_DIR` to an absolute path so Tauri’s Cargo invocation under `src-tauri`
and artifact collection use the same directory. Reuse the usual warm cache; use the assigned
worker directory when other workers are building. See [build storage](local-build-storage.md). The manually dispatched
`.github/workflows/package.yml` matrix uses separate Windows, macOS and Ubuntu 22.04 runners,
sets an absolute workspace target directory, builds with the lockfile, caches npm and Rust inputs, and uploads the unsigned outputs for 14 days. Its `nightly` and `release`
channels also run the full CI gate and publish a GitHub release; see
[branches and releases](releases.md#package-workflow). Tauri's [GitHub Actions guide](https://v2.tauri.app/distribute/pipelines/github/)
is the upstream basis for the runner matrix and Linux packages (checked 2026-10-13).

## Release readiness

### Native cryptography qualification

As checked 2026-10-15, the published pinned combination `openssl` 0.10.81 /
`openssl-sys` 0.9.117 selects `openssl-src` 300.6.1+3.6.3. It therefore embeds
**OpenSSL 3.6.3**, which lacks the security fixes in 3.6.4 (2026-08-25) and 3.6.5
(2026-09-29). See the upstream [release notes](https://openssl-library.org/news/openssl-3.6-notes/)
and [August](https://openssl-library.org/news/secadv/20260825.txt) /
[September advisories](https://openssl-library.org/news/secadv/20260929.txt) (ENG-284).
The current Passkey-verification and USB CTAP paths were reviewed for applicability;
no reachable affected path was identified in the [scoped review](../research/security-key-authentication.md#openssl-applicability-review--2026-10-15). The project continues synthetic-data
development only and must be reassessed when authentication or crypto use changes.
It does not make the bundled version current, establish general non-applicability,
or replace upstream advisory review with a passing `cargo deny` result.

The investigated [upstream OpenSSL 4 integration PR](https://github.com/rust-openssl/rust-openssl/pull/2692)
was unmerged and had no maintainer review at the checked date. This project retains
the published graph instead of adopting that PR or a custom source/build wrapper
(ENG-285). **Before production**, adopt a maintained, reviewed integration carrying
all applicable upstream fixes, rebuild the application, and repeat advisory review,
native-key tests and package/clean-host checks on Windows, macOS and Linux. Static
linkage requires redistributing the rebuilt application to deliver the fix. This is
part of [G2-09](../quality/open-items-g1-g2.md), not a waiver of that gate.

### Other release requirements

| Item | Current state | Release requirement |
| --- | --- | --- |
| Project licence | GPL-3.0-only adopted ([ADR 0017](../architecture/decisions/0017-gplv3-license.md)); the canonical [LICENSE](../../LICENSE) defines the project license. Third-party material retains its applicable terms and notices. | Include license texts and applicable notices; verify dependency compatibility and provide corresponding source for each covered binary under GPLv3 section 6. Ownership administration remains Q-11. |
| Publisher identity | The plain metadata publisher is “Vibe Toxicology LIS project,” not a legal publisher or signing identity. | Decide the legal publisher entity and name (Q-19/Q-34). |
| Windows signing | Certificate thumbprint, digest algorithm, timestamp URL and custom signing command are explicitly empty. | Authenticode-sign and verify both the executable and NSIS installer. |
| macOS trust | Developer ID identity and notarization provider are explicitly empty. An ad-hoc signature is not release signing. | Sign, notarize, staple and verify on a clean host. |
| Linux signing | Packages and repository metadata are unsigned. | Choose the release channel and signing method, then verify on each supported distribution. |
| Updater | Updater artifacts are disabled; no endpoint, public key or private key is configured. | After Q-19/Q-34, retain lab approval and verified pre-update snapshot requirements (NFR-24). |
| File associations | None. | Add only if a user workflow and safe parser boundary are approved. |

Before conveying a covered binary, prepare and verify its matching corresponding
source and choose a GPLv3 section 6 delivery method. For a download, equivalent
source access alongside the binary at no further charge is one permitted method;
a private repository unavailable to recipients does not supply that access.
Include the build/install scripts and other material required by the license,
with applicable third-party licenses and notices. Review libraries, bundled fonts,
media and installer/runtime components in the actual artifact for compatibility
and distribution obligations. Patient data, credentials and operational backups
are not release source material. License selection alone does not produce or
qualify this full release package. The [development builds](releases.md) published on
GitHub do not meet these requirements and are not supported releases (Q-19, Q-34).

The Windows offline installer supplies the Evergreen WebView2 installer when it is missing; it does
not freeze WebView2. Windows continues to patch the runtime. A fixed runtime would add roughly 180
MB and transfer patch responsibility to this project; the current configuration does not use one,
and the web-engine update choice remains open in [Q-34](../planning/open-questions.md).

## M5 checks on real hosts

CI proves that the bundles can be assembled; it does not prove installation, host integration,
cross-platform parity (NFR-33), or clinical suitability. Run and retain evidence for the
[real-host qualification](../architecture/operations-contract.md#real-host-qualification):

| Area | Required checks on Windows, macOS and Linux |
| --- | --- |
| Install and launch | Install as a standard user, launch, confirm one instance, exercise uninstall/reinstall, and record the native web-engine version. Confirm the unsigned-artifact warnings expected in this M5 build. |
| Keep-awake | Confirm the inhibitor appears in `powercfg /requests`, `pmset -g assertions` or `systemd-inhibit --list`, and that the AC-powered host does not sleep while listeners are active. |
| Close and OS shutdown | Exercise the close button and platform shortcuts, including Cmd+Q and Alt+F4. Verify that OS sign-out, reboot and shutdown are not blocked and are classified correctly. |
| Host probes | Check BitLocker on/off/suspended, FileVault, and direct and LVM-backed LUKS; test the documented unknown cases. Check supported and unsupported OS versions/editions, locked-down PowerShell, clock status and free space on native file systems. |
| Native dialogs | Exercise every open/save/confirmation dialog on each OS and WebKitGTK version, including cancellation and off-main-thread behavior. |
| Backup targets and power loss | Exercise SMB, NFS/CIFS and removable targets as applicable, including FAT32's 4 GiB limit. Interrupt a copy and verify that it is never reported as verified. |
| Clean-machine recovery | Restore from only a recovery kit and off-host copy onto a clean replacement of each OS; time it, verify artifacts and audit chains, write the OS key store, and relaunch with the single-instance guard. |
| Critical parity | Run sign-in/switching, accessioning, analyzer intake at sign-in, review, release, label/report printing, backup/restore, update/rollback rehearsal, and the 10,000-row worklist check in WebView2, WKWebView and WebKitGTK. |

The laboratory and project release owners must review this evidence. A successful unsigned CI build
does not satisfy NFR-25 or NFR-33 and does not authorize a pilot or production release.
