# Coding standards

Status: **in force**. Applies to every change, human or AI-written. Companions: [UI standards](ui-standards.md), [library policy](library-policy.md), [security, audit and logging](security-audit-logging.md), and [engineering standards](engineering-standards.md) for process, review depth, AI-assistance controls and release. Repeatable procedures live in the [skills](../../.ai/README.md): `reuse-first`, `simplicity-review`, `engine-command`, `ui-screen` and `quality-gate`.

## Principles

| Principle | What it means here |
| --- | --- |
| **KISS** | Choose the simplest design that meets the requirement and keeps the [domain invariants](../../AGENTS.md#domain-invariants). Plain functions and data first. Add a trait only when two real implementations exist now. A fixed clock and a system clock count; "a future backend" does not. Add generics only when two concrete types use them now. Use macros only to remove real repetition that a function cannot. |
| **YAGNI** | Build only what the current milestone's requirement IDs need ([implementation plan](../planning/implementation-plan.md)). Do not add speculative options, flags, extension points, unused enum variants, "future" parameters or layers of indirection. A deferred capability is a line in the plan, not code. |
| **One home for each piece of knowledge** | A rule, gate, constant, SQL statement, format or mapping lives in exactly one place and is called from everywhere that needs it. Duplicated *domain rules* are defects. For incidental code, apply the rule of three: tolerate a second copy of a few lines, and extract on the third. Do not merge code that only looks alike but means different things. |
| **Explicit over clever** | No stringly-typed dispatch, global mutable state, hidden side effects, reflection-style tricks or "magic" defaults. Data flow is visible in function signatures. |
| **Make illegal states unrepresentable** | Newtypes for IDs, units and lexical decimals; enums for states, comparators and outcomes; constructors that validate. Parse untrusted input into domain types at the boundary, once. |
| **Small, layered steps** | Every command follows one pipeline: parse → validate → authorize → recheck gates → change state → audit → outbox. Each step lives in its own place ([engine-command skill](../../.ai/skills/engine-command/SKILL.md)). |

## Complexity budgets

These budgets are enforced by tools where possible and by review otherwise. A budget exceeded "for now" is a defect to fix, not a backlog item.

| Measure | Budget | Enforced by |
| --- | --- | --- |
| Function body | Aim for 40 lines or fewer; 80 is the hard limit | clippy `too_many_lines`; review for TypeScript |
| Cognitive complexity per function | 15 or less | clippy `cognitive_complexity`; Biome `noExcessiveCognitiveComplexity` |
| Nesting depth | 4 levels or fewer; use early returns | clippy `excessive_nesting`; review |
| Parameters | 6 or fewer; otherwise pass an input struct | clippy `too_many_arguments`; review |
| File length (excluding tests) | About 400 lines; split by responsibility before that | `quality-gate` script; review |
| Duplication | No clone of 8 or more lines or 60 tokens in non-test code; total under 2% | `jscpd` |
| Unused code and dependencies | None | rustc `dead_code`, `unreachable_pub`, `cargo machete`, `knip` |

Suppress a lint only locally, with a reason. In Rust use `#[expect(clippy::lint_name, reason = "…")]`, which warns once the suppression is no longer needed. In TypeScript use `// biome-ignore lint/group/rule: reason`. Never suppress a lint crate-wide or file-wide without coordinator approval.

## Structure and dependencies (no spaghetti)

- **The crate graph is a DAG** declared in the [implementation plan](../planning/implementation-plan.md#crate-map). A crate depends only on crates listed below it. There is no `utils`, `common` or `helpers` grab-bag. Shared code goes to the lowest crate that owns the concept, usually `lis-core`.
- **A module owns its tables.** Only the owning crate writes them; table names carry the module prefix from the crate map, for example `spc_specimens`. Another module reads them only through the owner's public query functions, or SQL views the owner defines in its migrations (`spc_v_…`), never by raw access to private tables.
- **Layers inside a crate.**
  - `model` holds types, enums, state transitions and validation. It is pure and needs no database to test.
  - `repo` holds the SQL for this module's tables.
  - `commands` and `queries` hold the use cases that combine the two.
  - `lib.rs` holds only module declarations, re-exports and the crate documentation.
- **No global singletons.** State and dependencies are passed explicitly, through the command context, the engine state or function arguments.
- **Domain crates are synchronous.** They run on the database writer thread or a reader. Async (`tokio`) is confined to the engine's listeners and workers and to the Tauri shell.
- **UI layering:** `components/` ← `features/<area>/` ← `app/`. A feature never imports another feature; anything shared moves down to `components/` or `lib/` ([UI standards](ui-standards.md#structure)).

## Rust

- Edition 2024 on the pinned toolchain (`rust-toolchain.toml`); toolchain upgrades are reviewed changes. `Cargo.lock` is committed and builds use `--locked`. Every crate sets `[lints] workspace = true`. Clippy runs with `--all-targets --locked -- -D warnings`; warnings are denied by CI flags, not by `#![deny(warnings)]` in source, so new compiler lints do not break old tags.
- **No `unsafe`.** `unsafe_code` is forbidden workspace-wide. An exception is allowed only in a named FFI or platform crate, with a `// SAFETY:` comment per block and a second reviewer.
- **Errors.** Each crate has one `thiserror` enum, or one per module when the crate is large. Variants name the domain failure (`SpecimenOnHold`, `RevisionConflict`), never `Other(String)`. Use no `anyhow`. Never `unwrap`, `expect` or `panic!` outside tests. A clinical command returns an explicit failure, never a default value. A panic on the writer thread stops clinical writes and is reported; it is never caught and ignored. Errors convert to `lis_core::CommandError` at the engine boundary. `Display` text is PHI-free ([security standards](security-audit-logging.md#errors-returned-to-the-ui)).
- **Exhaustive state handling.** Domain crates add `#![warn(clippy::wildcard_enum_match_arm)]`, so adding a state forces every transition to be revisited. Internal state enums are never `#[non_exhaustive]`.
- **Numbers.** Clinical quantities use `lis_core` lexical decimal types, backed by `rust_decimal` for comparison and arithmetic. Never use `f32`/`f64` for them, and never deserialize them from JSON numbers. Parsing rejects values outside `Decimal`'s range or the declared precision instead of rounding.
- **Visibility.** Default to `pub(crate)`. Make an item `pub` only when another crate uses it now (`unreachable_pub` is on). Each public item gets a `///` doc line that states purpose, invariants or the requirement ID. Do not restate the item's name.
- **SQL** goes through `rusqlite` with bound parameters, never string-built values. Use `prepare_cached` for repeated statements. Each statement lives in the module's `repo`, next to the single function that runs it.
- **Naming.** Follow the Rust API guidelines. Commands are verbs (`receive_specimen`), queries are `get_*`/`list_*`, errors are `XError`, identifiers are `XId`.
- **Comments** explain why, not what. Reference requirement IDs at the code that enforces them (`// REQ-INT-009: acknowledge only after durable receipt`). Commit no commented-out code. Write `TODO(M<n>): …` only with a milestone from the plan.

## TypeScript

- Run `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride` and `noFallthroughCasesInSwitch`. Do not use `any`, non-null assertions (`!`) or `as` casts. The single exception is the IPC boundary in `ui/src/lib/ipc.ts`.
- Use function components and hooks only. Use named exports only, no default exports. Name component files in PascalCase (`StatusBadge.tsx`) and other modules in camelCase. Keep one component per file, with its `.module.css` and `.test.tsx` beside it.
- Import from the project with the `@/` alias, which maps to `ui/src/`.
- Install UI packages from the lockfile with `npm ci`.
- **Decimals stay strings** from the engine to the screen and back. Never call `Number()`, `parseFloat()` or arithmetic on them.
- **No clinical logic in the UI**: no gates, thresholds, calculations, unit handling or permission decisions. The UI may hide or disable a control based on `allowedActions` the engine returns, as a convenience. The engine still decides every action.

## SQL and migrations

- Each module owns `crates/<crate>/migrations/NNNN_description.sql`, numbered from `0001` and sequential within the module. The engine applies modules in crate-map order. Migrations are forward-only: never edit one after the coordinator has integrated it. The runner stores a checksum and refuses edited migrations.
- Every table is `STRICT`, with foreign keys declared. `REAL` and `ANY` columns are forbidden; a schema lint test enforces this. Constrain enum-valued `TEXT` columns with `CHECK (col IN (…))`.
- **Conventions.**

  | Kind of value | Storage |
  | --- | --- |
  | Identifiers | `TEXT` UUIDv7 |
  | Timestamps | `TEXT` RFC 3339 UTC (`…Z`), with source offset or precision in separate columns where the domain needs them |
  | Clinical quantities | Lexical `TEXT` with separate unit, comparator and scale columns |

  Mutable aggregates carry `revision INTEGER NOT NULL` for optimistic concurrency; a stale `expected_revision` fails with a conflict error.
- Append-only tables (audit events, evidence, receipts, released revisions) get triggers that raise `ABORT` on `UPDATE` and `DELETE`.
- **Database runtime.** SQLite 3.51.3 or later through `rusqlite`'s bundled build, never the system library ([ENG-144](../research/sources-engineering.md#desktop-application-stack-sources-2026-10-12)). Foreign keys on for every connection; WAL with `synchronous=FULL`; a busy timeout. No clinical quantity column has NUMERIC, REAL or INTEGER affinity; a derived scaled-integer column for range queries is allowed but is never the source (schema lint, VAL-49). No ORM decimal mapping ([ENG-102](../research/sources-engineering.md#single-computer-technology-sources)).
- **One process, one writer.** Only the application process opens the database, and only one instance per database runs (single-instance lock plus database lock file). One writer thread owns the write connection; reads use separate connections. The app refuses to start with the live database on a network share, NAS or synced folder.
- **Transactions.** Every write runs in `BEGIN IMMEDIATE`, never a deferred transaction that upgrades. A clinical command reads its gates and writes the change, the audit record and the outbox rows in that one transaction; release rechecks every gate inside it. Keep write transactions short: render reports and labels outside, then verify the snapshot revision in the final transaction.
- Snapshots use `VACUUM INTO`, verified with `PRAGMA integrity_check` and the audit-chain head.

## Reports, labels and interfaces

- Typst templates are versioned files in the repository. `typst` and `typst-pdf` are pinned and wrapped behind one renderer module; each issued report records template, renderer and font versions. Release verification checks PDF/A-2 and PDF/UA-1 output ([ENG-146](../research/sources-engineering.md#desktop-application-stack-sources-2026-10-12)).
- Raw ZPL label templates are versioned the same way. The engine implements TCP 9100 and CUPS transport; native Windows RAW spooler support remains an implementation and qualification gap. Each enabled route needs printer/stock verification.
- The ASTM driver sends ACK only after durable receipt and ships with a conformance corpus. Qualify each analyzer profile against malformed input and replay cases. HL7 v2/MLLP and FHIR routes remain unimplemented proposals under [ADR 0007](../architecture/decisions/0007-integration-edge-and-interface-engines.md); codec selection and fuzzing evidence belong to that separately scoped work.

## Tests

- **Levels.** Pure unit tests sit in-module (`#[cfg(test)]`). Module integration tests in `tests/` use a real SQLite file with production pragmas. Engine scenario tests run synthetic end-to-end journeys. UI component tests use Vitest and Testing Library. Never mock the database.
- **Expected clinical outcomes** come from specifications or human-authored fixtures, not from the implementation under test ([engineering standards](engineering-standards.md#ai-assisted-development-controls)).
- **Every safety guard has a negative test** showing that the guarded action is refused. Examples: release blocked by a QC hold, acknowledgement withheld before durable receipt, a command refused without its capability. Name tests for behavior, such as `release_fails_when_qc_hold_commits_first`.
- **Property tests** (`proptest`) cover codecs, decimal parsing and state machines; parsers must not panic on arbitrary bytes.
- **Deterministic tests**: inject the clock and identifiers, never sleep, and use synthetic data only. Shared fixture builders live in a crate's `testing` module (`#[cfg(any(test, feature = "testing"))]`), not copied between test files.

## Definition of done

A change is done when all of the following hold:

- The [quality gate](../../.ai/skills/quality-gate/SKILL.md#verification-matrix) passes for the actual change and its affected dependents.
- Public items and crate docs are current, and requirement IDs are cited at enforcement points and in test names or comments.
- User-visible UI or engine changes include a source-to-manual review and the matching [bundled manual](../help/README.md) update in the same change ([user-manual skill](../../.ai/skills/user-manual/SKILL.md)).
- No dependency was added outside the [library policy](library-policy.md).
- No file outside the change's owned paths was edited.
- The handoff note lists:
  - what was built, with requirement IDs;
  - decisions taken and alternatives rejected;
  - deviations from these standards, with reasons;
  - known limitations;
  - the exact verification commands run and their results, relevant skipped checks
    with reasons, and any reused evidence.

Keep evidence proportionate to the change; a standalone report is required only when the risk, milestone/release scope or a project owner request warrants it.
