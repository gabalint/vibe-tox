# Security, audit and logging standards

Status: **in force**. This document says how code implements the controls designed in [security and privacy](../architecture/security-and-privacy.md), [ADR 0008](../architecture/decisions/0008-tamper-evident-audit-and-immutable-recovery.md) and NFR-08, NFR-18, NFR-19, NFR-31 and NFR-32. When the two disagree, the design document wins; raise the conflict with the coordinator.

## Three separate records

| Record | Purpose | Storage | PHI | Read by |
| --- | --- | --- | --- | --- |
| **Audit trail** | Who did what, when, why, under which authority (REQ-OPS-001) | `audit_events` table; hash-chained per stream; written in the same transaction as the change | Record identifiers only; never copies clinical content | Auditors and quality staff, in the app, with the `ViewAudit` capability |
| **Diagnostic log** | Troubleshooting the software | Rotating JSON-lines files in the app's log directory, written through `tracing` | **Never** | Maintainers and support; bundles stay PHI-free (NFR-32) |
| **Clinical records** | The facts themselves, including custody and revisions | Domain tables of the owning module | Yes | Through engine commands and queries only |

Never use one for another: logs are not an audit trail, and audit events are not a log.

## Audit trail

**When to write.**

- **State changes.** Every command that changes state appends at least one audit event through `ctx.audit(…)`, inside its `BEGIN IMMEDIATE` transaction. If the append fails, the whole command fails and nothing commits.
- **Sensitive reads.** Opening a patient or report record, exporting, printing a report, viewing the audit trail and delivering a report each append an `access` event before data leaves the engine. If that event cannot be written, the read fails.
- **Security events** use the `security` stream: sign-in success and failure, MFA failure, lockout, lock and unlock, user switch, enrolment, role or grant change, break-glass use, update approval, backup, restore and export. An authorization denial is recorded in its own transaction after the denied command rolls back.
- **Clinical configuration** (catalog, methods, rules, mappings, templates) is recorded on the `clinical` stream, including drafts, approvals and retirements, because it determines results.
- **Listener and worker actions** are attributed to the instrument, partner or worker principal, never to whoever is signed in.

**Event fields** (the `lis-audit` crate owns the schema):

| Field | Content |
| --- | --- |
| `stream`, `seq`, `prev_hash`, `hash`, `hash_alg` | Chain position and integrity |
| `occurred_at`, `recorded_at` | UTC timestamps from the host clock |
| `principal` | Kind (user, instrument, partner, system worker), ID, session ID, workstation |
| `action` | Dotted lowercase `<module>.<verb>`, for example `specimen.receive` or `report.release` |
| `outcome` | `succeeded`, `denied` or `failed` |
| `record_type`, `record_id`, `old_revision`, `new_revision` | What changed |
| `reason` | Required for corrections, overrides, cancellations, break-glass and any lab-policy exception |
| `authority` | The capability or grant that permitted the action |
| `correlation_id` | Shared with the diagnostic log line for the same command |
| `details` | Small JSON of non-PHI references: IDs, version numbers, codes, digests |

**No PHI in `details` or `reason` templates.** Do not include names, birth dates, addresses, phone numbers, MRNs, result values or free-text comments. Reference the record by ID; the record and its revisions hold the content. A user-typed reason is stored as entered, because it is part of the record. Never echo it into logs.

**Authority attribution.** For a command allowing either of two capabilities, record the capability actually granted to the actor. Audited reads establish that grant in the same transactional context as their access event. Session-only reads record session authority without inventing a clinical capability. Historical audit events are never rewritten to repair attribution.

**Integrity (ADR 0008).**

- **Hashing.** Each event's hash is SHA-256 over a versioned canonical serialization: fixed field order, UTF-8, no floats, `hash_alg` recorded. The hash covers the predecessor's hash and the per-stream sequence number.
- **Append-only tables.** Triggers abort `UPDATE` and `DELETE`. This stops application bugs, not the host administrator; the off-host anchors in the next point cover that.
- **Sealing and anchors.** Sealing writes segment digests. Anchors carry digests and metadata only, never identifiers or content.
- **Verification** recomputes the chain and reports gaps, forks, reordering and anchor mismatches.

## Diagnostic logging

- **Through `tracing` only.** Never use `println!`/`eprintln!`; clippy denies them. In the UI, use no `console.*` in committed code. Engine errors reach the user through `CommandError`, and the engine logs them.
- **Levels.**

  | Level | Use |
  | --- | --- |
  | `error` | An operation failed and needs attention |
  | `warn` | Degraded operation, a retry, or a dead-lettered item |
  | `info` | Lifecycle: app start and stop, listener up and down, worker job outcome, one completion line per command |
  | `debug`, `trace` | Developer detail; off in release builds |

- **One completion line per command, written centrally.** The engine pipeline logs `action`, `correlation_id`, principal ID, `outcome`, error code and `duration_ms`. Commands do not log their own success; doing so duplicates the central line.
- **Allowed fields:** opaque IDs (UUIDs), correlation IDs, action names, counts, durations, error codes and kinds, module, instrument and listener IDs, software and schema versions, content digests.
- **Forbidden fields:**
  - patient names, birth dates, addresses, phone numbers, MRNs or other identifier values;
  - result values, free text and comments;
  - raw analyzer, HL7 or file payloads, and parser messages that echo input;
  - SQL with bound values;
  - report content;
  - passwords, TOTP secrets or codes, keys, session tokens;
  - file paths that contain patient data.
- **Type-level redaction.** Hold PHI in `lis_core::Phi<T>` and secrets in `secrecy::SecretString`. Their `Debug` and `Display` print a redaction marker, so an accidental `{:?}` cannot leak content. Serialization to the UI is explicit.
- **Files.** Logs go to the app data directory under `logs/`, rotate daily, follow a configurable retention, and are readable only by the app's OS user and administrators. A PHI-canary test seeds a synthetic marker in clinical data and asserts that it never appears in log output (NFR-32).

## Authorization

- **Engine-side and deny-by-default.** Every command calls `ctx.require(Capability::…)`, plus a scope check where records are client- or site-scoped, before reading gates or changing state. Queries call `principal.require(…)`. A command without an explicit check fails review. A table-driven engine test asserts that every registered command refuses a principal who lacks its capability.
- **Separate authorities.** OS administration confers nothing in the app. App administration (`ManageUsers`, `ManageConfiguration`) never grants clinical authority (`ReviewResults`, `ReleaseReports`).
- **Fresh authentication for release.** Release requires re-authentication bound to the specific candidate revision (REQ-IDN-004, NFR-19). Self-approval is prevented wherever lab policy requires separation: the laboratory's separation-of-duties policy decides it per duty, each permitted self-approval needs the person's confirmation, and its audit event carries `self_approval: true` ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)).
- **Intake principals.** Analyzer listeners act as `Principal::Instrument`, holding only intake capabilities.

## Input handling

- **Untrusted input:** UI command arguments, analyzer and HL7 bytes, imported files and anything from the network. Apply size limits, parse into domain types once at the boundary, and reject unknown fields (`#[serde(deny_unknown_fields)]` on command inputs).
- **Durable receipt first.** Store inbound bytes, verify the digest and commit the receipt before acknowledging or semantically processing them (REQ-INT-008/009).
- **Payload text is data.** Render it as text in the UI, never as HTML; no `dangerouslySetInnerHTML`.
- **Paths** are never built from untrusted input. Files live in the app data directory under content-addressed names.

## Secrets and credentials

- **Never in** the repository, installers, logs, error messages, the UI, crash reports or audit `details`.
- **Passwords** are hashed with Argon2id at or above the current OWASP minimum, with parameters stored next to the hash.
- **TOTP secrets** are encrypted at rest with a key from the OS key store (`keyring`).
- **Personal unlock PINs and security keys** follow the design in [security and privacy](../architecture/security-and-privacy.md#identity-and-authorization) and [ADR 0019](../architecture/decisions/0019-security-key-authentication.md). In code:
  - Store PINs only as salted Argon2id verifiers encrypted with the installation data key and user/purpose-bound AEAD. Installation key-binding and recovery checks cover PIN-only installations. PIN input and verifiers never enter audit, diagnostics or persisted UI state. The engine alone decides PIN eligibility and failure counting, transactionally; a PIN never extends a session's absolute lifetime or issues a sensitive-action proof.
  - Verify security-key registrations and assertions only through the maintained verifier ([library policy](library-policy.md#rust-crates)), with a fixed installation RP/origin; write no custom signature verification. Rust retains challenges and frozen typed actions. A native authenticator PIN collected on USB platforms is transient and stays out of logs, audit and persisted UI state; the OS owns the Windows prompt.
  - Action proofs are private, expiring and one-use; check and consume them inside the original command transaction and never return them through IPC. Recheck policy, credential/account revisions and session status after hardware interaction and inside completion transactions.
- **Key recovery codes** are random, hashed, one-use and shown only at issuance. They authorize verified replacement-key enrollment, never clinical access or role changes. PINs and recovery codes must not remain in React Query mutation variables/cache after use.
- **Comparisons** of secrets and codes are constant-time.
- **In memory**, use `secrecy::SecretString` or `zeroize` buffers, and drop secrets as soon as they are used.
- **Failed sign-ins** follow a lab-configurable lockout or back-off. Responses do not say which factor failed.

## Errors returned to the UI

`lis_core::CommandError` is the only error type that crosses into the UI. It contains:

- a stable `code`, such as `forbidden`, `validation`, `conflict`, `gate_blocked`, `not_found`, `session_locked` or `internal`;
- a safe, PHI-free message the user can act on;
- optional field errors and gate reasons;
- the `correlation_id`.

It never contains SQL, stack traces, file paths or internal error text. Internal failures are logged with the correlation ID, and the UI shows that ID so support can find the log line.

## Desktop shell (NFR-31)

- **Content and navigation.** A strict CSP: bundled scripts and styles only, no inline script, no `eval`, no remote origins. No remote content, and no navigation away from the bundled UI.
- **Capabilities** allow-list exactly the commands the window uses. Do not expose the shell, file system or HTTP plugins to the UI. A plugin is added only after review under the [library policy](library-policy.md).
- **Configuration.** No `dangerous*` settings; a release check verifies it. Devtools are off in release builds.
- **Session identity.** The engine records the webview name and version in each session's audit context ([ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md#consequences)).

## Security checks for every change

- A negative authorization test for each new command.
- Validation tests for malformed and oversized input.
- A no-panic property test for each parser.
- No PHI or secrets in log statements, error text or fixtures; review every new `tracing` call against the forbidden list.
- `cargo deny check` and `npm audit` show no new unreviewed advisories or licenses.
- Changes to capability files, CSP, authorization, audit or migrations get line-level review by the coordinator ([engineering standards](engineering-standards.md#ai-assisted-development-controls)).
