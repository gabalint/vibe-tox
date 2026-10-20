# ADR 0012: Rust engine and Tauri desktop application

Status: **accepted in part**.

- **Accepted by the project owner, 2026-10-12:** a Rust engine in a Tauri 2 desktop application with a TypeScript UI.
- **Implemented on synthetic data:** the local components below, under [ADR 0013](0013-implementation-scope.md). Implementation does not accept the remaining component/operating proposals or establish platform qualification.
- **Proposed:** production packaging, signed updates, host qualification and future components. These require technical-maintainer review and the evidence below.

Decision owner for components: technical maintainer. Supporting evidence is in [desktop application stack research](../../research/desktop-application-stack.md), [ENG-130–136, ENG-144 and ENG-146](../../research/sources-engineering.md#desktop-application-stack-sources-2026-10-12) and [single-computer technology research](../../research/single-pc-technology.md). The [library policy](../../governance/library-policy.md), workspace manifests and lockfiles identify the current dependencies; this ADR does not duplicate their version inventory.

## Context

[ADR 0009](0009-single-computer-local-lis.md) defines the single-computer application: one desktop process with no background service, targeting Windows, Linux and macOS. Analyzer listeners run while that application is open, including at its sign-in screen. A TypeScript UI in the application's own webview is accepted (Q-36); the application does not run a UI web server.

The Rust engine owns clinical state, authorization, exact-decimal handling and release gates. The UI presents engine data and submits typed commands. The embedded renderer and local backup implementation run within that boundary, allowing the local laboratory workflow to operate without an external service.

## Proposal

The accepted stack and current component implementation are the baseline for qualification. Additional transports, hosted services and distribution mechanisms require their own scoped authorization and evidence.

| Layer | Current implementation | Remaining boundary |
| --- | --- | --- |
| Shell and UI | Tauri 2 with the OS webview; React/TypeScript, React Aria, CSS Modules and TanStack query/table/virtualization components | WebView2, WKWebView and WebKitGTK each need reference-host testing |
| Engine and IPC | Rust modules composed by `lis-engine`; thin Tauri adapters, generated TypeScript DTOs, restricted capabilities and content security policy | Clinical authorization remains engine-side regardless of UI visibility |
| Database | `rusqlite` with bundled SQLite; WAL, `synchronous=FULL`, STRICT tables, foreign keys, one writer and `BEGIN IMMEDIATE` transactions | Power-loss and workload qualification; SQLCipher is not configured |
| Quantities | Validated lexical decimal text with separate unit/comparator semantics; `rust_decimal` for exact engine calculations | Source precision and approved display rules remain distinct |
| Reports | Embedded `typst`/`typst-pdf`, versioned templates and bundled fonts; renderer requests PDF/A-2a and PDF/UA-1 output | Independent accessibility and reference-host printing checks remain gates; export options alone do not prove conformance |
| Analyzer intake | In-process serial/TCP ASTM and AU400/AU640 receive workflows, plus approved delimited files and watched folders | Actual instruments/firmware/assays require qualification; HL7 v2/MLLP and FHIR partner routes remain [ADR 0007](0007-integration-edge-and-interface-engines.md) proposals |
| Labels | ZPL generation, TCP 9100 and CUPS raw transports | Windows RAW-spooler support and actual printer/stock qualification are not established by these transports |
| Backup and export | `VACUUM INTO` snapshots, content-addressed artifacts and built-in encrypted `vtbk1` folder-target copies; documented recovery kit and portable export | Replacement-host recovery needs qualification. Incremental `vtbk2` remains [ADR 0014](0014-incremental-backups.md); object-lock targets remain proposed |
| Authentication and keys | OS key-store integration, per-person security keys or password/TOTP, bounded sessions and recovery under [ADR 0019](0019-security-key-authentication.md) | Physical-key/OS qualification, production authentication policy and patched native cryptography remain gates |
| Packaging | Unsigned test-bundle configuration: Windows NSIS, macOS `.app`/`.dmg`, Linux `.deb`/AppImage | Signing, notarization, publisher identity and release channels remain Q-19/Q-34; see [packaging](../../development/packaging.md) |
| Updates | Local version/approval records and pre-update backup contracts | No updater endpoint, keys or artifacts are configured. Signed distribution and lab-approved installation/rollback remain proposed release work |
| External services | None implemented | [ADR 0010](0010-optional-cloud-services.md) defines a separate optional-service proposal; it grants no authority to alter clinical state |

**Open-application operation.** Listeners continue while the application is open and signed out. Human actions require an individual session. Single-instance protection, listener-aware sleep prevention, close warnings and startup reconciliation are part of the local implementation; native behavior remains subject to per-OS evidence (REQ-OPS-013).

## Rationale

One process and one serialized writer keep clinical transitions, audit and durable work in a common transaction boundary. Rust supports the engine's typed state transitions and parsers, while the TypeScript component system supplies forms and large record lists. Embedding Typst avoids a separate renderer process; bundling SQLite provides a controlled database dependency. These choices carry native-platform and dependency-maintenance obligations; they are not a claim that another stack is unsuitable.

## Consequences

- **Three web engines require qualification.** Each supported OS has its own webview and update lifecycle. Clinical calculations and gates remain in Rust. Record the webview version, run the required UI smoke check after OS/webview changes, and retain per-OS evidence under NFR-33. The configured Windows bundle installs Evergreen WebView2; a fixed runtime is not configured.
- **Two implementation languages.** Contributors need Rust for engine work and TypeScript for UI work; generated DTOs keep the boundary explicit.
- **Open application required.** Reboot, sign-out, shutdown or application close stops receiving. Analyzer retention/retransmission, host operating policy and startup reconciliation must account for the gap.
- **Serialized release decisions.** Release and invalidation recheck their gates in `BEGIN IMMEDIATE`; [ADR 0002](0002-modular-monolith.md) defines the conflict protocol.
- **Distribution is a separate gate.** Packaging configuration does not establish installability, signing trust, clinical validation or permission to publish.

## Acceptance evidence

1. Lexical decimals round-trip through SQLite, Rust and the UI without losing source precision; any future HL7 route must demonstrate its own numeric contract.
2. Same-transaction audit/outbox survives interruption between commit and dispatch; release/invalidation works in both commit orders.
3. Power interruption with `synchronous=FULL` preserves committed work on qualified hardware.
4. Analyzer intake continues at the sign-in screen on each supported OS; qualify each claimed serial/TCP protocol with the relevant instrument.
5. ZPL bytes reach the intended printer/transport and print on approved stock; a new Windows spooler transport requires its own evidence.
6. A rendered report passes the required PDF accessibility and printing checks.
7. A signed install, explicitly approved update and rollback through snapshot restore succeed on each supported platform.
8. A clean replacement machine restores the `vtbk1` backup with the recovery kit and an off-host copy, verifies evidence and completes reconciliation.
9. A 10,000-row worklist remains responsive in each supported webview on reference hardware.

Current gaps are tracked in [G1/G2 readiness](../../quality/open-items-g1-g2.md). Synthetic fixtures, successful builds and a component's API contract do not replace this qualification.

## Revisit

Revisit the affected component when measured writer contention, platform failures, dependency support/security changes or a newly authorized workflow show that the current implementation cannot satisfy its requirements. A shell, database, rendering or backup replacement needs a scoped decision, migration/recovery analysis and qualification; no fallback stack is preselected here.
