# Desktop application: platform evidence and qualification

This document retains the platform evidence needed to qualify the desktop application
chosen in [ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md).
Checked dates are in the
[engineering ledger](sources-engineering.md#desktop-application-stack-sources-2026-10-12);
source research does not establish that this application passes on a supported host.
`Cargo.lock`, the [library policy](../governance/library-policy.md) and the
[packaging guide](../development/packaging.md) define current dependencies and bundle settings.

## One process: what it means

The Rust engine runs inside the desktop application. The embedded UI invokes Tauri
commands; the installed application exposes no browser UI server or separate background
service. The development server used by `npm run dev` is development tooling.

| Boundary | Current design and qualification consequence |
| --- | --- |
| Application lifetime | Analyzer listeners and durable work run while the application is open, including at the sign-in screen. Closing it, signing out of the OS, rebooting or sleeping the host interrupts availability. Verify restart reconciliation and sender replay on the actual analyzer routes. |
| Individual identity | A signed-out or locked LIS can receive analyzer data; human commands still require an authorized individual session. OS login does not replace the LIS account. |
| UI and engine | The Rust engine owns validation, authorization, calculations, clinical gates and transactions. Tauri capabilities and the content security policy bound UI access; every engine command still checks its own inputs and authority. |
| Concurrent work | The database writer queue, listeners and worker tasks share the application lifetime. Parsing, rendering and hardware prompts stay outside short write transactions. |
| Host behavior | Single-instance control, sleep inhibition, close handling and recovery require native-host testing under the lab's OS and power policy. These controls do not make a desktop process survive OS sign-out or power loss. |

The current operational contract is in [deployment](../architecture/deployment.md)
and the [implementation plan](../planning/implementation-plan.md). Additional LAN
workstations remain Q-28 and require a separate access and identity design.

## The cost of Tauri: three web engines and Linux

Tauri uses WebView2 on Windows, WKWebView on macOS and WebKitGTK on Linux. Those
engines and their update paths differ. The recorded upstream Linux issues document
rendering and GPU failures on particular configurations, so Linux qualification must
use the intended distribution, driver and hardware rather than assuming parity
([ENG-130–136](sources-engineering.md#desktop-application-stack-sources-2026-10-12)).

The Windows bundle includes the WebView2 **Evergreen offline installer**. It supports
installation without an initial download but does not freeze the installed web engine.
A Fixed Version runtime is not configured. macOS and distribution updates can likewise
change the embedded web engine independently of an LIS release. Critical user journeys,
keyboard/scanner input, accessible controls and representative worklists need testing
on each qualified platform (NFR-33, Q-29, Q-36). Capacity evidence comes from
benchmarking the actual application.

## Per-platform matters

| Concern | Current configuration or evidence | Remaining qualification |
| --- | --- | --- |
| Windows bundle | Current-user NSIS installer with WebView2 offline installation; `src-tauri/tauri.windows.conf.json` | Authenticode publisher, signing and clean-host installation; printer, serial and native-key behavior |
| macOS bundle | `.app` and `.dmg`; `src-tauri/tauri.macos.conf.json` | Developer ID, notarization and clean-host installation on each supported architecture |
| Linux bundle | `.deb` and AppImage; WebKitGTK/GTK runtime dependencies in `src-tauri/tauri.linux.conf.json` | Distribution and driver compatibility, USB permissions and clean-host installation |
| Updates | Updater artifacts disabled; no endpoint or signing key configured | Q-19/Q-34, release verification, lab approval, verified pre-update backup and recovery procedure |
| Keys and storage | Platform key-store integration and full-disk encryption are separate controls | Custody, OS-account behavior, lost-host recovery and the lab's host hardening policy |
| Serial devices and printing | Built-in analyzer drivers and local label/report paths | Actual adapter, printer model, driver, media and scanner qualification; source documentation is insufficient |

Unsigned developer bundles are not supported clinical releases. Exact build commands,
platform prerequisites and release gates belong in the
[packaging guide](../development/packaging.md), not a parallel installer specification.

## Engine libraries

Storage, exact-decimal and report-rendering evidence for the implemented engine libraries
(`rusqlite`, `rust_decimal`, Typst) is in the
[local technology research §2–§3](single-pc-technology.md#2-language-and-runtime); backup behavior
is in the [backup guide](../operations/backups-and-recovery-kit.md).
