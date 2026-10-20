# Implementation plan

Status: **active implementation on synthetic data**. [ADR 0013](../architecture/decisions/0013-implementation-scope.md) authorizes the application scope. This document records current scope, module ownership, contracts and milestone criteria. The [operator guide](../operations/README.md) describes implemented workflows; the [quality overview](../quality/README.md) distinguishes verification from laboratory acceptance.

## Scope

Build the **US clinical toxicology LIS** defined in the [project scope](../project-scope.md#product-decision) on synthetic data. The initial slice follows one result lineage from order through analysis, report, delivery and operational recovery. Clinical billing capture and performed-service handoff are part of the local LIS.

[ADR 0018](../architecture/decisions/0018-remediation-authority-and-method-continuity.md) governs configuration authority, accepted-work continuity and standing-order applicability. DM-07 source-document custody, RPT-06 urgent/delayed-result communications and RPT-07 patient/proxy disclosure workflows remain separate [scope decisions](../quality/scope-decisions.md).

| Boundary | Current instruction |
| --- | --- |
| License and repository content | GPL-3.0-only ([ADR 0017](../architecture/decisions/0017-gplv3-license.md)); content rules follow the [AGENTS.md mandate](../../AGENTS.md#current-mandate). |
| Publication | The repository is public and GitHub Pages publishes `website/` ([website maintenance](../development/public-website.md)). Software releases remain gated (Q-19, Q-34). |
| Future capabilities | Optional hosted services, LAN workstations (Q-28), patient portal, and HL7 v2/FHIR routes beyond canonical local exports require separate scope decisions. |
| Release and clinical use | Signing waits for publisher identity (Q-19). Synthetic implementation and local builds do not authorize deployment, real patient data or clinical operation. |

The [coding](../governance/coding-standards.md), [UI](../governance/ui-standards.md), [library](../governance/library-policy.md) and [security, audit and logging](../governance/security-audit-logging.md) standards apply. [AGENTS.md](../../AGENTS.md) governs data handling and domain invariants.

## Repository layout

| Path | Contents |
| --- | --- |
| `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/config.toml` | Workspace, pinned toolchain, lint budgets, dependency policy, ts-rs output directory |
| `crates/lis-*` | Engine crates, one per module ([crate map](#crate-map)); each holds `src/`, `migrations/` (if it owns tables) and `tests/` |
| `src-tauri/` | `lis-desktop`: the Tauri shell, window configuration, capabilities, thin command adapters |
| `ui/` | React UI source ([UI standards](../governance/ui-standards.md#structure)); `ui/src/generated/` holds ts-rs bindings |
| `package.json` and JS tool configuration at the root | One npm project for the UI and the JS tooling (Biome, Vitest, jscpd, knip, Tauri CLI) |
| `fixtures/` | Synthetic data shared across crates, added when two crates need the same fixture |
| `scripts/`, `.github/workflows/ci.yml` | Small repository checks (for example the file-length budget) and the CI quality gate on Linux, macOS and Windows |
| `docs/`, `.ai/` | Documentation, standards, agent roles and skills |
| `website/`, `.github/workflows/pages.yml` | Static presentation, video, captions, transcript and application stills; GitHub Pages deployment workflow |

## Crate map

Crates are listed in dependency order. A crate may depend only on crates above it, which also fixes migration order. Table names carry the crate's prefix.

| # | Crate | Owns | Prefix | Depends on | Milestone |
| --- | --- | --- | --- | --- | --- |
| 1 | `lis-core` | Typed IDs, lexical decimals and comparators, UTC timestamps and the clock, `Phi<T>`, principals and capabilities, `CommandError`, `Migration`, correlation IDs | — | — | M0 |
| 2 | `lis-audit` | Audit events, hash chain, verification, seals and digest-only anchors | `audit_` | core | M0 |
| 3 | `lis-store` | Database open and pragmas, single writer thread, read connections, `CommandContext` (`BEGIN IMMEDIATE` unit of work), migration runner with checksums, schema lint, transactional outbox, content-addressed artifact store, `VACUUM INTO` snapshots | `store_` | core, audit | M0 |
| 4 | `lis-astm` | ASTM E1381/E1394 codec and receive-side driver state machine (pure) | — | core | M0 |
| 4a | `lis-au` | AU400/AU640 fixed-width codec, frame decoder, block assembly and explicit requisition encoding (pure) | — | core | Instrument interface |
| 5 | `lis-report` | Typst renderer wrapper, bundled fonts, versioned report templates | — | core | M0 |
| 6 | `lis-labels` | ZPL label templates, safe field encoding, TCP 9100 and CUPS raw transports | — | core | M0 |
| 7 | `lis-auth` | Local accounts, Argon2id/TOTP or verified personal security keys, credential recovery, sessions, idle lock, user switching, roles to capabilities, lockout, separation policy (ADR 0015/0019) | `auth_` | core, audit, store | M1 |
| 8 | `lis-identity` | Subjects and identifiers with assigning authority, client organizations and locations, practitioners, identity holds | `idn_` | core, store | M1 |
| 9 | `lis-catalog` | Analytes, methods, orderables and panels, with draft, approved and retired versions | `cat_` | core, store, auth (separation policy, ADR 0015) | M1 |
| 10 | `lis-interfaces` | Instruments and connections, durable inbound receipts, raw payload artifacts, listener status | `ifc_` | core, store, astm, au | M1 |
| 11 | `lis-orders` | Orders, items, frozen component manifests, order evidence as received, clarification holds | `ord_` | identity, catalog | M2 |
| 12 | `lis-specimens` | Accessions, specimens, containers, aliquots, receipt decisions, locations, label jobs | `spc_` | orders, labels | M2 |
| 13 | `lis-analytical` | Runs and positions, QC assessments, deviations | `ana_` | specimens, catalog, interfaces, auth (separation policy, ADR 0015) | M2 |
| 14 | `lis-results` | Import attempts, measurements, result revisions, technical review | `res_` | analytical | M2 |
| 15 | `lis-reporting` | Report candidates and revisions, release with gate recheck, corrections, delivery obligations, attempts and evidence | `rpt_` | results, report | M3 |
| 16 | `lis-billing` | Performed-service events and the class-based service-derivation candidate | `bil_` | results, orders, auth (separation policy, ADR 0015) | M3 |
| 17 | `lis-ops` | Backup snapshots and targets, restore test, exit export, anchor export, host health, update approvals | `ops_` | store, audit | M4 |
| 18 | `lis-engine` | Composition root, command and query pipeline, central logging and error mapping, outbox workers, listener supervisor, startup reconciliation | — | all of the above | M1 onward |
| 18a | `lis-security-key` | Native FIDO2 client, cancellable PIN/touch interactions; no account authority or signature-verification decision | — | external native WebAuthn/USB libraries | ADR 0019 |
| — | `lis-desktop` (`src-tauri/`) | Tauri shell and thin command adapters, native security-key operation controls | — | engine, security-key | M0 onward |

The crate map describes module ownership. Confirm actual manifest dependencies before changing a crate; new modules require a current scoped requirement.

## Core contracts

These contracts define the application boundary and support parallel work. Changes require review of affected callers and tests.

- **Commands.** Domain crates expose synchronous functions `fn verb(ctx: &mut CommandContext<'_>, input) -> Result<Output, ModuleError>`. `CommandContext` is defined in `lis-store` and wraps the `BEGIN IMMEDIATE` transaction with the principal, the clock time and the correlation ID. Its methods are `tx()`, `principal()`, `now()`, `correlation_id()`, `require(capability)`, `audit(event)` and `enqueue(outbox_item)`. The engine runs the closure on the writer thread and handles commit, rollback, the completion log line, denial auditing and mapping to `CommandError` ([engine-command skill](../../.ai/skills/engine-command/SKILL.md)).
- **Gate reasons.** Engine error mapping preserves typed domain gate reasons in `CommandError.gateReasons`; UI presentation does not reconstruct clinical gate decisions.
- **Queries** run on read connections with the principal's capability check. Sensitive reads go through the engine's audited-read path.
- **Migrations.** Each crate with tables exposes `pub const MIGRATIONS: &[lis_core::Migration]`, with SQL in `migrations/NNNN_name.sql` via `include_str!`. The store applies them per module in crate-map order and records a checksum per `(module, version)`.
- **Outbox.** `ctx.enqueue(OutboxItem { topic, dedup_key, payload })` in the same transaction as the change. Workers lease rows with an expiry, retry with bounded backoff, and move exhausted rows to a dead-letter state visible to the lab. The row is the authority (NFR-02, NFR-06).
- **Artifacts** are content-addressed files under the data directory. The store writes and syncs them to disk before any row references the digest.
- **Durable receipt before acknowledgement.** The ASTM driver emits an acknowledgement only after the caller confirms that the received bytes are durably stored (REQ-INT-009).
- **IPC.**
  - One thin `#[tauri::command]` per engine command.
  - DTOs derive `ts_rs::TS` and export to `ui/src/generated/`.
  - The UI calls the engine only through `ui/src/lib/ipc.ts` and the hooks in `ui/src/lib/commands.ts`.
  - Errors cross the boundary as `CommandError` only.
  - The active session lives in the engine; the UI holds no token.


### Shared safety and transport boundaries

| Boundary | Contract |
| --- | --- |
| Audit detail | Maximum 4 KiB, no floats or arbitrary clinical text. Use typed detail fields; keep user-supplied reasons in their dedicated field. |
| Identifier correction | Requires `manage_quality` and a signed-in user. Identifiers previously assigned to one subject cannot be silently reassigned to another. |
| Snapshot integrity | A snapshot object includes its integrity result; backup/restore callers must refuse a snapshot that is not intact. |
| Analyzer intake defaults | TCP peers require an allowlist; ASTM ENQ must arrive within 5 seconds; transfers are bounded to 8,192 frames. Machine tokens are bounded to 128 bytes. |
| Operational identity | The application identifier `org.vibetox.lis` remains subject to publisher choice (Q-19). OS key-store and data-directory identity must be reviewed before distribution. |
| Laboratory qualification | Label stock/layout, actual instruments, report accessibility and per-OS behavior require laboratory or reference-host evidence; synthetic fixtures do not qualify them. |

## Milestones

| Milestone | Outcomes | Requirement focus | Exit evidence |
| --- | --- | --- | --- |
| **M0 Foundations** | Standards, workspace and tooling; `lis-core`, `lis-audit`, `lis-store`; ASTM codec; Typst and ZPL rendering; desktop shell with the design system and app frame | NFR-01, NFR-02, NFR-10, NFR-18, NFR-31, REQ-OPS-001, REQ-INT-009 (codec part) | Workspace quality gate green. Tests prove: decimals round-trip without scale change; outbox recovery after a crash between commit and dispatch; tamper detection by the audit chain; deterministic report PDF; exact ZPL bytes to a TCP test printer; ASTM conformance corpus. The app launches to a sign-in screen; a 10,000-row grid demo works. |
| **M1 Platform** | `lis-auth` with sign-in, TOTP, lock, switch user and user administration; `lis-engine` pipeline, workers, listener supervisor and startup reconciliation; `lis-identity`, `lis-catalog` and `lis-interfaces`; admin UI | REQ-IDN-001, REQ-IDN-003, REQ-IDN-004, REQ-ANA-002 (configuration versions), REQ-INT-005, REQ-INT-009, REQ-OPS-001, NFR-08, NFR-19 (TOTP part) | Sign-in with TOTP in the running app. An ASTM message from a simulator over TCP is received while the app is at its sign-in screen and acknowledged only after commit. Security events appear in the audit trail. The table-driven authorization test covers every command. |
| **M2 Order to result** ([contract](../architecture/order-to-result-contract.md)) | Orders with evidence as received; accessioning, specimens, aliquots and labels; runs and QC; result import from receipts; technical review; worklist grid | REQ-ORD-001–003, REQ-ORD-005–007, REQ-SPC-001–005, REQ-SPC-008, REQ-ANA-001, REQ-ANA-003–007, REQ-RES-001–005 | Synthetic journey: order → accession → label → ASTM import → QC assessment → review, including a reprocessed re-export and a duplicate import |
| **M3 Report and handoff** ([contract](../architecture/reporting-contract.md)) | Report candidate, Typst render, release that rechecks every gate in one transaction, corrections, local delivery log with evidence levels, billing derivation candidate, work queues | REQ-RPT-001–003, REQ-RPT-005, REQ-RPT-011, REQ-RPT-013 (local part), REQ-BIL-001, REQ-BIL-002, REQ-BIL-005, REQ-OPS-004, NFR-01, NFR-07 | Release versus QC hold, identity hold and cancellation tested in both commit orders; amendment preserves the prior version; no screen or export shows a bare "delivered" |
| **M4 Operations** | Encrypted backup to a lab target, restore and restore test, exit export, audit sealing with anchor export, host health checks, open-application operation, update-approval records | REQ-OPS-002, REQ-OPS-003, REQ-OPS-006–010, REQ-OPS-013, NFR-04, NFR-12, NFR-18, NFR-23, NFR-30 | Backup restored to a fresh directory verifies the chain and artifact digests. Synthetic offline-day scenario. Close warning, single instance and startup reconciliation demonstrated. |
| **M5 Hardening** | Adverse-scenario suite, security review, performance budgets, per-OS packaging configuration (unsigned), developer and operator documentation | NFR-05, NFR-29, NFR-31–33, the [validation plan's](../quality/validation-plan.md#critical-scenario-seed-matrix) adverse scenarios | Scenario report, threat-model review, measured writer wait and grid responsiveness, and a list of open items for G1 and G2 |

## Status

M0–M5 application components are integrated. Their milestone criteria remain the verification baseline; integration does not close the human-owned release gates. Current gaps and qualification work are recorded in [G1/G2 open items](../quality/open-items-g1-g2.md), with capability decisions in [scope decisions](../quality/scope-decisions.md).

| Area | Implemented scope and remaining boundary |
| --- | --- |
| Core laboratory journey | Local identity, catalog, order evidence, accessioning, containers, runs/QC, result import/review, report release/correction, delivery reconciliation and performed-service handoff. Requirements outside the authorized slice retain their own priority and status. |
| Connected intake | Patient/order selection continues into receiving with historical test labels, audited accession context, eligible material allocation and explicit receipt/label decisions. Supplied collection zones and ambiguous civil times require explicit resolution. See [daily operation](../operations/daily-operation.md). |
| Billing | Reusable patient financial profiles, confirmed versioned order billing, evidence/deficiency review, collection corrections and rejected-handoff recovery. Laboratory payer policy and external claims processing remain Q-16/Q-21; see the [billing intake contract](../architecture/reporting-contract.md#billing-intake). |
| Instrument and file intake | Durable ASTM and AU400/AU640 receipts plus approved delimited-file templates and watched folders. AU automatic order dispatch and actual instrument/firmware/assay qualification remain outside the supported receive workflow; see [AU contract](../architecture/au-protocol.md) and [result files](../operations/result-files.md). |
| Catalog and demo | Standard Catalog loads as drafts; Catalog Preview binds review to exact content. A separate DEMO installation can prepare synthetic data after normal sign-in, with persisted, atomic and retryable setup. No clinical approval is supplied by starter data. See [starter-data contract](starter-data-brief.md). |
| Authentication and authority | Password/TOTP alternative and personal security keys; administrator configuration authority and laboratory separation-of-duties policy follow ADR 0015/0018/0019. Additional staff finish password/TOTP onboarding before migrating their own account. Role-wide FIDO2 enforcement for release and administration (REQ-IDN-004, NFR-19) and the sealed break-glass account are not implemented. Physical-key/OS qualification, production authentication policy and the patched OpenSSL gate remain open in G2-09/Q-42. |
| Personal unlock PIN | Optional six-digit PIN resumes only the current locked session, preserves its 12-hour deadline and ends it after five wrong/malformed submissions. Normal authentication is required for configuration; recovery/reset removes it. It is a local convenience, not an MFA claim. See [ADR 0019](../architecture/decisions/0019-security-key-authentication.md). |
| Laboratory touch-only setting | Off by default. An administrator needs fresh normal confirmation and a reason to enable it; policy changes end other sessions and invalidate action proofs. Existing verified credentials, first-administrator signup, recovery and sensitive actions retain verification. A failed PIN never falls back to touch-only. See [security architecture](../architecture/security-and-privacy.md). |
| Operations | Encrypted full backups, recovery kit, restore/reconciliation, portable export, audit seals/anchors, host health and open-application controls. Incremental backup remains proposed in ADR 0014. Replacement-host recovery and native OS behavior require qualification. |
| UI and guidance | Contextual Page help and a searchable offline manual, with same-change page/field/action and recovery documentation. Compact page layouts follow the [page layout standard](../governance/page-layout-standard.md); the [help maintenance contract](../development/contextual-help.md) governs content. |

Native startup, physical-key behavior, platform qualification and installation success are claimed only with current qualification evidence. Signed installers/updater (REQ-OPS-006), real-host backup/health/open-application checks (REQ-OPS-007/008/013), capacity qualification and laboratory acceptance remain explicit gates.

## Fleet and coordination

The coordinator owns contracts, integration review and the current plan. In-process subagents may work in parallel with explicit disjoint paths. External fleets use separate accounts and require the project owner's confirmation of the proposed split before launch.

| Rule | Required practice |
| --- | --- |
| Ownership | Assign explicit paths and shared registration blocks. Workers edit only their assigned paths; the coordinator resolves cross-owner changes. |
| Git | Workers do not write the index or history. The coordinator reviews the complete working tree and staged content before committing. |
| Build isolation | Use each worker's assigned warm `CARGO_TARGET_DIR`, scope checks to affected crates and dependents, and use the shared Cargo queue. Never share a target across checkouts or create a fresh target just to avoid a lock. |
| Shared state | One owner at a time changes root manifests, lockfiles or generated interfaces. Dependencies follow the [admission rule](../governance/library-policy.md#admission-rule). |
| Verification | Run `python3 scripts/select_checks.py` and the [quality-gate matrix](../../.ai/skills/quality-gate/SKILL.md#verification-matrix). Verify completed changes; reuse valid evidence while relevant inputs remain unchanged. Prose alone does not trigger application builds/tests. Full checks apply to cross-cutting changes, unknown impact and release/milestone gates. |
| Review | Apply [reuse-first](../../.ai/skills/reuse-first/SKILL.md) and [simplicity-review](../../.ai/skills/simplicity-review/SKILL.md). Review migrations, authorization, audit, decimal handling, capabilities and CSP line by line. Obtain independent review for consequential changes, return confirmed findings for fixes, and integrate only after the final gate is green. |
| Handoff | Report changed paths, requirement IDs, API/contract changes, decisions, limitations, owner requests and exact check results or justified skips. A routine correction needs no standalone dated report. |
| Records | Keep current knowledge in canonical documents. Store development-session reports, build logs, screenshots and handoffs under ignored `.local/quality-history/`; do not make tracked guidance depend on local artifacts. |

Verification evidence does not replace independent review, real-host qualification or the laboratory's acceptance of a configured release.
