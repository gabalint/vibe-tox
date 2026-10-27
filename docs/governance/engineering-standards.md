# Engineering and AI-assisted development standards

These standards apply to the implementation authorized by [ADR 0013](../architecture/decisions/0013-implementation-scope.md), on the stack in [ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md). Day-to-day rules live in the [coding standards](coding-standards.md), [UI standards](ui-standards.md), [library policy](library-policy.md) and [security, audit and logging standards](security-audit-logging.md). This document governs process, review depth, AI-assistance controls, verification, release and the OS test matrix. A tool not yet configured, such as `cargo-vet`, SBOM generation or branch protection, applies from the milestone that introduces it in the [implementation plan](../planning/implementation-plan.md).

## Accountable changes

Every substantive change identifies its problem, affected requirement/risk/decision IDs, expected behavior, migration/configuration effects, verification and residual limitations. Prefer small reviewable changes. AI-generated code receives the same human accountability, evidence requirements and license review as other contributions. Do not accept an explanation or a green build as proof of clinical correctness.

Risk determines review depth. A documentation typo needs a local check. Patient matching, numerical semantics, authorization, QC/release, report corrections, retention and migrations need independent technical review plus the appropriate laboratory/quality or privacy owner. Clinical rule changes require approved specifications and independently supplied expected outcomes. Operational gates are defined in [validation](../quality/validation-plan.md).

## Implementation principles

- Use supported dependency/runtime versions, reproducible lockfiles, explicit module ownership, type checking and consistent formatting/linting. Do not maintain competing toolchains without need.
- Keep domain transitions and authorization in the Rust engine, never in the UI. Enforce database uniqueness/references as well as application validation. Represent physical units, decimals, statuses and versions explicitly at API boundaries.
- Separate transport parsing, clinical validation and authorized state change. Use durable work, bounded retries and reconciliation for external effects. Never hide failed clinical operations behind a generic success response.
- Version external contracts, the relay protocol and configurations; identify breaking changes and retirement periods. Test actual partner/instrument versions and document unsupported variants.
- Treat migrations as data changes: forward-only, applied after a verified pre-upgrade snapshot, rehearsed on production-shaped synthetic history, with audit/provenance preserved and lock/runtime measured. Prefer expand/contract; rollback means snapshot restore plus the prior signed installer, followed by reconciliation.
- Telemetry, crash reports and support bundles are PHI-free by default; crash dumps and raw logs stay on the host (NFR-32). The [security, audit and logging standards](security-audit-logging.md) define the code-level rules.
- Keep runtime AI out of the first core. A future model feature needs its own intended use, data permissions, evaluation, fallback and human-control decision. Development agents cannot approve clinical results.

Language, database, desktop-shell, report, label and interface rules are in the [coding standards](coding-standards.md) and [security, audit and logging standards](security-audit-logging.md). Optional hosted services remain proposed in [ADR 0010](../architecture/decisions/0010-optional-cloud-services.md); local application standards do not authorize a service or establish its isolation, security or operational evidence.

## AI-assisted development controls

These controls apply to AI-assisted implementation under the current [definition of done](coding-standards.md#definition-of-done). They supplement independent human review. Release tooling not yet configured remains subject to the implementation and release gates.

- **Independent expected outcomes.** Expected clinical outcomes for P0 scenarios come from lab-approved specifications or human-authored synthetic fixtures. They do not come from the agent session that writes the implementation. An agent may write test harness code. It may not decide what the clinical answer should be. For safety-critical invariants (identity holds, QC gating, release/invalidation, decimal boundaries, disclosure checks), show that the test fails when the guarded behavior is removed, using a deliberate-fault or mutation check.
- **State and numeric properties.** Explicit state machines and exact-decimal semantics suit property-based and model-based tests that generate adverse sequences (concurrent release/hold, duplicate import, cancellation during reflex). Do not rely on example tests alone.
- **Dependency admission.** A dependency is never added just because an agent suggested it. Before adding one, confirm that the package exists in the expected registry under the expected publisher. Then check maintenance status and license, pin it with hashes, and review the addition as part of the change. One study found code-generating models frequently reference packages that do not exist. The same invented names recurred across runs, so attackers can pre-register them ("slopsquatting"). [ENG-68: Spracklen et al., USENIX Security 2025 preprint](https://arxiv.org/abs/2406.10279)
- **Agent permissions and untrusted input.** Development agents get no production credentials, no patient data and no deployment rights. Vendor files, issues, fetched pages and partner specifications are untrusted input that can carry injected instructions. Keep agent instructions in reviewed, versioned repository files ([AGENTS.md](../../AGENTS.md)) rather than ad hoc prompts; OpenSSF publishes a security-focused pattern for such instruction files. [ENG-69: OpenSSF guide, 2025-08-01](https://best.openssf.org/Security-Focused-Guide-for-AI-Code-Assistant-Instructions.html)
- **Review depth by consequence, not authorship.** Migrations, authorization policy, numeric semantics, release logic and interface mapping receive line-level human review whatever their origin. Each change record names the affected requirement and risk IDs and the checks that ran. Generated volume is not progress evidence.
- **Rights and context hygiene.** Do not place licensed manuals, proprietary vendor code, restricted standards, credentials or real requisitions in model context. License review covers AI-produced code that may reproduce third-party material.

NIST SP 800-218A profiles secure development for producers of AI models and AI systems. For AI-*assisted* coding of a conventional application, the base SSDF practices remain the primary reference. [ENG-70: NIST SP 800-218A, July 2024](https://csrc.nist.gov/news/2024/nist-publishes-sp-800-218a)

## Meaningful automated verification

The [quality-gate matrix](../../.ai/skills/quality-gate/SKILL.md#verification-matrix) is the canonical policy for verification scope, check selection, reuse of results and handoff evidence; local work and CI share its selector.

Applicable code checks include formatting/linting/types, risk-relevant unit/integration/contract/UI tests, migration checks, dependency/secret/license scans and reproducible build artifacts. Prioritize adverse cases and invariants over a blanket coverage percentage. Avoid tests that merely restate the implementation or inspect documentation wording. For relational integrity and concurrency, use SQLite with the application settings and realistic transaction boundaries. Any separately authorized service needs its own database and isolation verification. Installer, listener, serial, printing, sleep and power-loss tests run on real hosts of each supported OS from the [OS test matrix](#os-test-matrix).

Select and map applicable ASVS requirements by exact version; use NIST SSDF to structure secure development responsibilities. These are proposed engineering reference frameworks rather than blanket legal mandates. [OWASP ASVS](https://owasp.org/projects/asvs), [NIST SSDF](https://csrc.nist.gov/pubs/sp/800/218/final)

## Release and maintenance

A future release record identifies source revision, dependency/SBOM inventory, build provenance, change notes, schema/config/contract versions, verification evidence, supported upgrade path and the actions the laboratory must take. Protect release credentials, scope automation identities, review vulnerability advisories, and record a maintainer and response plan for dependency exceptions. Set actual security response targets only after staffing exists.

**Release engineering.** Current unsigned development packages and exact build commands are in [packaging](../development/packaging.md). Signing, publication, update distribution and supported-platform qualification remain outstanding. The following requirements apply before supported distribution:

- **Platform packages.** Qualify the configured NSIS, macOS application/DMG, and Linux Debian/AppImage packages on clean supported hosts. Additional formats need their own scope and verification. Apply the signing and notarization requirements in the [packaging release criteria](../development/packaging.md#other-release-requirements).
- **Signing route.** The publisher name, Windows signing route and Apple Developer account are decided under Q-19 and Q-34 (RISK-53).
- **Updater ([ADR 0020](../architecture/decisions/0020-in-app-updates.md), accepted).** The shell installs only newer, Minisign-signed packages with signature-bound versions, after a recorded approval of that package and a verified pre-update backup, on request and never on a schedule. Authenticated, expiring metadata and freeze protection remain required before a supported release (NFR-21/24, Q-19/Q-34); see [Updates](../operations/updates.md).
- **SBOM and provenance.** Every release artifact must have a signed SBOM with the CISA minimum elements and signed build provenance: `cargo-cyclonedx` for the Rust dependency graph, `cargo-auditable` to embed the dependency list in each binary, `npm sbom` (or the package manager's equivalent) for the UI, plus entries for sidecars and any bundled WebView2 runtime. Release verification must reject unverifiable artifacts (NFR-21). Any bundled sidecar binary is pinned, hash-checked, signed with the app and listed in the SBOM.
- **Dependency gates.** `cargo-deny` runs in CI; the release pipeline adds `cargo-audit` and `cargo-vet` audits or trusted imports for every third-party crate.
- **Patch cadence.** Maintainers watch RustSec, Tauri, npm and sidecar advisories, and the OS web engines' security releases. Security-relevant fixes ship as an expedited Class 1 release that the lab still approves. Never silent (RISK-54). Web-engine updates arrive through the OS outside our releases; release notes say which engine versions were tested.
- **Support window.** How long each version is supported is Q-34. No version's support can outlast its supported OS baseline.
- **Lab-approval package.** Each release ships the [release validation package](../quality/validation-plan.md#release-validation-package-for-laboratory-acceptance): change summary in laboratory terms, risk class, executed verification evidence per OS, migration and rollback notes, the lab's acceptance protocol and an acceptance record template. The lab approves before installation; a service provider never installs remotely without that recorded approval (REQ-OPS-006, NFR-24).
- **License record.** Each release lists every bundled component's license and commercial condition, including any WiX licensing terms counsel reads as applying to the build pipeline.

Maintain a compatibility matrix for host OS and version, webview engine and version, Rust toolchain, Tauri, SQLite, Typst, label printers and serial adapters, each analyzer and partner profile, and the relay-protocol version. Release compatibility is not inferred from protocol name alone. Include backup/restore and rollback evidence when a change affects them. Updates in a laboratory follow that laboratory's quality and change process.

## OS test matrix

Proposed reference configurations for verification and release evidence. They test each supported version in the host baseline ([NFR-25](../requirements/nonfunctional-requirements.md), [deployment host baseline](../architecture/deployment.md#host-baseline)); the baseline itself is Q-29 and must be confirmed against partner hardware.

| Dimension | Configurations to test |
| --- | --- |
| Windows (x64) | Each supported edition and version. Negative cases: Home, a release past its servicing date, Windows 10 without commercial ESU (installer refuses or health check flags, VAL-48) |
| macOS | Each supported release, on Apple silicon and, while supported, x64 |
| Linux | Each supported distribution (x64); Wayland and X11 sessions; Intel, AMD and NVIDIA graphics |
| Web engines | WebView2 Evergreen current and previous; WKWebView on each macOS release; WebKitGTK 4.1 as shipped by each distribution. Post-update UI smoke check; 10,000-row worklist responsiveness |
| Security settings | BitLocker (TPM 2.0, Secure Boot), FileVault and LUKS on, off and suspended where possible; native authentication keys and recovery custody per OS; device certificates only if an external connector enters scope |
| Storage and power | SSD with power-loss protection and a consumer SSD; hard power-off with and without a UPS (VAL-38) |
| Session state | App open at its sign-in screen with nobody signed in while analyzers send; OS screen locked; app users switching at the app's sign-in screen on one shared OS login; idle lock; sleep prevented while listening; close warning; second launch focuses the first; OS sign-out, reboot and OS update gaps reconciled on next start |
| Locale and time | en-US; a locale with a comma decimal separator; daylight-saving transitions; a clock moved forward and back |
| Analyzer connections | Native serial port and USB-serial adapters per OS (COM, `/dev/cu.*`, `/dev/ttyUSB*` with `dialout`); TCP; watched folder on a local disk and on a network share written by vendor software |
| Printers | Zebra label printer over TCP 9100 and over USB through the Windows spooler RAW path or a CUPS raw queue; printer offline; a standard office printer for PDF reports per OS |
| Network | Offline local operation and analyzer reachability; add proxy, filtering and external-service cases only if a connector enters scope |
| Upgrade paths | Clean install from each signed package type; lab-approved update through the updater; upgrade from each supported prior version; rollback by snapshot plus prior package; restore to a replacement host, including onto a different supported OS if the lab changes OS |

ARM64 Windows and ARM64 Linux hosts are out of scope until Q-29 changes.

## Repository and contribution conventions

Use the documented default branch, `develop` ([branches and releases](../development/releases.md)), and reviewed pull requests. Preserve accepted ADRs and stable requirement IDs. Store reusable protocols and synthetic fixtures in the project; keep sensitive laboratory or service-provider evidence in its authorized repository/system. Supply module ownership and review assignments when maintainers are named; do not create fictitious CODEOWNERS or contacts.

Contributors disclose material AI assistance and retain responsibility for sources, test outcomes and submitted rights. Do not paste restricted manuals or copied incompatible code into a contribution. A proposed DCO sign-off process concerns origin and submission rights, not a clinical approval or warranty. [DCO 1.1](https://developercertificate.org/)

Before public software release: verify the GPL-3.0-only release license, compatible dependency/content inventory, required notices and corresponding-source delivery; establish maintainers and a private security channel; decide the signing route and release channel (Q-19, Q-34); publish support/status limits; and provide setup, update-approval, backup, restore and recovery documentation for each supported OS baseline. [Open-source strategy](open-source-strategy.md) records the accepted license and remaining ownership, contribution, trademark and future-service questions.
