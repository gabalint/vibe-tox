# Developer guide

This is the short contributor path for the Rust/Tauri/React application. Read
[AGENTS.md](../../AGENTS.md), the [implementation plan](../planning/implementation-plan.md) and
[CONTRIBUTING.md](../../CONTRIBUTING.md) before changing anything.

The separate [website and video](public-website.md) are static files; they do not run the
desktop application.

## Toolchain

| Tool | Repository version or rule |
| --- | --- |
| Rust | `rustup` with the pinned Rust 1.97.1 toolchain from `rust-toolchain.toml`, including `rustfmt` and `clippy` |
| Node.js | Node 24 is used in CI; the repository does not currently enforce it with a local version file |
| npm | Use the lockfile with `npm ci` |
| Python | Python 3.11 or newer for check selection, repository metadata and their standard-library tests; CI selects 3.11 |
| Tauri system dependencies | Install the current desktop prerequisites for your OS from the [official Tauri 2 guide](https://v2.tauri.app/start/prerequisites/) (checked 2026-10-13). CI installs WebKitGTK 4.1, Ayatana AppIndicator, librsvg and patchelf on Ubuntu. |
| Native authentication build tools | The pinned OpenSSL dependency enables `vendored`: a C compiler, Perl with core modules and make are required. Windows MSVC builds need native Windows Perl, such as Strawberry Perl, and Visual Studio's nmake; macOS uses the Xcode tools and system Perl; Linux CI installs `build-essential` and `perl`. |
| Policy tools | Install `cargo-deny` and `cargo-machete` for the full gate |
| Workflow validator | `actionlint` 1.7.12 checks GitHub Actions syntax and expressions; CI uses its versioned Docker image only when tooling checks are selected |

Windows development needs the Microsoft C++ Build Tools and WebView2. macOS desktop development
needs Xcode Command Line Tools. Linux package names vary by distribution; use the Tauri guide and
the Linux CI job as the project reference.

The WebAuthn verifier uses OpenSSL on all three desktop platforms. The workspace pins
`openssl` 0.10.81 and enables its `vendored` feature through `lis-auth`; `Cargo.lock` pins
the corresponding OpenSSL source. This builds static libraries instead of relying on a
developer's Homebrew installation or separately installed Windows OpenSSL DLLs. Do not
set `OPENSSL_NO_VENDOR` to a nonzero value for project builds. If several Perl versions
are installed on Windows, set `OPENSSL_SRC_PERL` to the native Perl executable; CI checks
its platform before building. The source build discovers MSVC's nmake automatically;
NASM assembly is optional and detected by the upstream build. See the primary
[openssl-rs build guidance](https://docs.rs/openssl/0.10.81/openssl/#vendored),
[OpenSSL source builder](https://github.com/alexcrichton/openssl-src-rs#openssl-src) and
[Windows prerequisites](https://github.com/openssl/openssl/blob/master/NOTES-WINDOWS.md),
checked 2026-10-15 (ENG-281–283). A linked static library is still an application security
dependency: update the lockfile and rebuild/distribute the application when it needs a fix.
The current source is OpenSSL 3.6.3 and lacks later upstream security fixes; the
[packaging qualification note](packaging.md#native-cryptography-qualification)
records the synthetic-development exception and mandatory patch gate before production.

## Build and run

Optional [local build storage guidance](local-build-storage.md) explains target paths and cache relocation.

From the repository root:

```bash
npm ci
npm run tauri dev
```

Useful narrower commands:

| Command | Purpose |
| --- | --- |
| `npm run dev` | Run only the Vite UI development server |
| `npm run build` | Type-check and build the bundled UI |
| `npm run tauri dev` | Build and run the desktop application in development |
| `npm run tauri build` | Build the desktop application and configured unsigned bundles for the current OS; see the [packaging guide](packaging.md) and [branches and releases](releases.md) |
| `cargo test -p <crate> --locked` | Test one Rust crate and regenerate its `ts-rs` bindings |

Do not use real patient data, requisitions, analyzer exports, credentials or licensed manuals. The
repository's fixtures and examples must be synthetic and non-identifying.

## Quality gate

Start with the changed inputs. The shared selector prints the checks and reasons; it
does not execute them:

```bash
python3 scripts/select_checks.py
python3 scripts/select_checks.py --base <base-commit> --head HEAD
python3 scripts/select_checks.py --full
```

Without `--head`, selection compares the working tree with the base (default `HEAD`),
including staged, unstaged and untracked files. With `--head`, it compares the two commit
trees. Unknown inputs, dependency/build changes or an unavailable comparison select the
full gate. `--full` explicitly selects every check. The rules live in
[`scripts/select_checks.py`](../../scripts/select_checks.py); do not reproduce them in
another path filter.

| Change | Selected verification |
| --- | --- |
| Ordinary Markdown | Local Markdown links/anchors and skill frontmatter; the link graph includes unchanged documents so renamed headings and deleted files cannot leave broken inbound links |
| License text or license-only manifest metadata | Repository metadata, documentation checks and `cargo deny check licenses`; no application compilation or UI suite |
| Static website | Website Biome check and repository/documentation checks |
| `docs/help/*.json` | Bundled-help coverage/metadata tests and UI build; review the affected prose against implementation |
| UI source | `npm run check` and `npm run build` |
| Rust, Tauri shell or runtime/test fixtures | Rust checks plus bundled-help coverage/build; embedded export documentation and verifier changes also select Rust |
| Generated IPC bindings | Rust, UI and help checks |
| Selector, its checks or CI wiring | Python check tests, repository metadata and workflow validation |
| Dependency/build configuration or an unclassified change | Full verification; manifest edits that change only license metadata are identified by parsed values |

Markdown is not always documentation-only: `docs/architecture/backup-and-export-formats.md`
is embedded in exit exports, so it selects Rust, help and documentation checks.
`scripts/verify_exit_export.py` has the same selection because it is the independent
export verifier. JSON topics under `docs/help/` are bundled application content and
select help tests/build rather than only Markdown link checks.

For a focused Rust change, run format, Clippy, tests and unused-dependency checks for the
affected crate and affected dependents from the
[quality-gate skill](../../.ai/skills/quality-gate/SKILL.md). CI deliberately uses a broader
Rust workspace matrix on Linux, macOS and Windows when the `rust` flag is selected, including
the independent export verifier and generated-binding check on Linux. Its UI build is needed
by the Tauri build script. The Linux Rust job also runs `cargo machete`, plus duplication
and file-length checks unless the full UI check already covers them.

The selected commands are:

| Check | Command |
| --- | --- |
| Repository metadata / local Markdown targets | `python3 scripts/check_repository_metadata.py` (same base/head convention; scans the local Markdown link graph when docs are selected; `--full` forces documentation and license metadata checks) |
| License policy | `cargo deny check licenses` |
| Website | `npx biome check website` |
| Bundled help | `npx vitest run ui/src/app/help/articles.test.ts`, then `npm run build` |
| UI | `npm run check`, then `npm run build` |
| Selector and metadata regression tests | `python3 -m unittest discover -s scripts -p 'test_*.py'` |
| Workflow syntax / expressions | `actionlint` |

The full Rust workspace and dependency-policy gate is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
cargo machete
```

The full gate also includes the UI, help, website, tooling and documentation checks above.
`npm run check` runs Biome, TypeScript, Vitest, unused-code, duplication and file-length checks;
it already includes the help tests and website Biome check, so CI does not repeat those
checks when the full UI check runs.
Checks of license metadata and links do not establish license compliance, prose accuracy or
clinical validity. Report what ran on the final change and what remains unverified.

### CI selection and the required check

The [`ci` workflow](../../.github/workflows/ci.yml) always starts selection on pushes to
`develop` and `main` and on pull requests, without workflow-level path filters. A manual `workflow_dispatch`
run selects the full gate. Selection publishes its reasons in the run summary. Pull-request
runs compare the base commit with GitHub's checked-out merge commit, so classification and
tests use the same proposed merged tree; pushes compare the event's previous and current
commits. New-branch or missing baselines select full verification. Checkout fetches history
to make these comparisons available. These choices follow GitHub's
[pull-request event semantics](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request)
and [checkout history controls](https://github.com/actions/checkout) (ENG-275, ENG-277).

The stable `quality-gate` job uses `always()` and waits for selection and every optional job.
It fails when selection fails, a flag is invalid, a selected job fails or is skipped, or any
job is cancelled. It accepts intentionally unselected jobs being skipped. This avoids
depending on a skipped job's success status or a path-filtered workflow that leaves a required
check pending, as described in GitHub's
[required-check guidance](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks#handling-skipped-but-required-checks)
and [`needs` semantics](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idneeds)
(ENG-276). Repository administrators can require `quality-gate`; editing this workflow does
not itself change branch protection. CI does not deploy. The
[package workflow](releases.md#package-workflow) calls it as the full gate before
publishing nightly and versioned builds, and the separate
[Pages workflow](public-website.md#publication) publishes only `website/`.

The governing rules are the
[coding](../governance/coding-standards.md), [UI](../governance/ui-standards.md),
[library](../governance/library-policy.md), [security/audit/logging](../governance/security-audit-logging.md)
and [engineering](../governance/engineering-standards.md) standards.

## Repository and crate map

The complete dependency-ordered map is in the
[implementation plan](../planning/implementation-plan.md#crate-map). The main areas are:

| Path | Responsibility |
| --- | --- |
| `crates/lis-core`, `lis-audit`, `lis-store` | Shared types, audit chain, SQLite writer/readers, migrations, outbox, artifacts and snapshots |
| `crates/lis-astm`, `lis-report`, `lis-labels` | Protocol codec, report rendering and label rendering/transports |
| `crates/lis-auth` | Accounts, roles, TOTP, sessions and security policy |
| `crates/lis-identity`, `lis-catalog`, `lis-interfaces` | Identity, versioned configuration and durable analyzer intake |
| `crates/lis-orders`, `lis-specimens`, `lis-analytical`, `lis-results` | Order-to-result domain workflow |
| `crates/lis-reporting`, `lis-billing`, `lis-ops` | Report/delivery, billing handoff, and backup/restore/health/update operations |
| `crates/lis-engine` | Composition root, command/query pipeline, workers, listeners and startup reconciliation |
| `src-tauri/` | Thin Tauri adapters, handler registration, capabilities, startup and desktop shell |
| `ui/src/components`, `features`, `lib`, `generated` | Design system, screens, IPC/query helpers and generated Rust DTO bindings |
| `fixtures/`, `scripts/`, `docs/`, `.ai/` | Shared synthetic fixtures, repository checks/tools, project knowledge and reusable work procedures |

A crate may depend only on crates above it in the plan. A module owns its tables. Do not read or
write another module's private tables directly.

## Add an engine command

Use the [engine-command skill](../../.ai/skills/engine-command/SKILL.md); do not copy its procedure
into another guide. Before writing code, apply
[reuse-first](../../.ai/skills/reuse-first/SKILL.md).

At a high level, the owning domain crate supplies the validated command/query and DTOs, the engine
registers its command name and authorization, `src-tauri` adds a thin adapter plus explicit
permission/capability registration, and the UI calls it through the typed command hooks. Every
state-changing command needs engine authorization, an audit event in the same transaction,
PHI-free diagnostics and typed errors. Sensitive reads use the audited-read path.

Update only the marked area blocks in shared registration files. Add negative authorization and
safety-gate tests, then run the affected crate tests and the desktop capability/registry tests.

## Add a screen

Use the [ui-screen skill](../../.ai/skills/ui-screen/SKILL.md) and the existing design-system
components. Screens live in `ui/src/features/<area>/`, call the engine only through
`ui/src/lib/commands.ts`, and contain no clinical rules or permission decisions. Add loading,
empty, error and normal states, accessible interaction tests, and a route/navigation entry where
needed. Run the UI gate from the quality-gate skill.

Add or review the matching [built-in page help](contextual-help.md) in the same change.
The [help authoring guide](../help/README.md) defines its source format, route coverage and
review checklist. Help ships with the application; it is available offline.

## Generated bindings and `CommandName`

Rust IPC DTOs derive `ts_rs::TS` and export into `ui/src/generated/` when their owning Rust tests
run. These files are committed and never edited by hand.

`CommandName.ts` is generated separately by the `export_command_names` test in `lis-desktop`. Its
union comes from the engine command registry plus the small shell-only command list. UI command
constants use that union so a misspelled or unregistered command fails TypeScript checking.

After changing IPC types or registration, run at least:

```bash
cargo test -p <owning-crate> --locked
cargo test -p lis-desktop --locked export_command_names
npm run typecheck
git diff -- ui/src/generated
```

Review the generated diff. Do not accept an unrelated binding rewrite.

## Coordinated worker review

Multi-agent waves use disjoint path ownership, a private `CARGO_TARGET_DIR` per worker and no
worker writes to the Git index or history. Handoff contents and the coordinator's review and
integration steps are in
[Implementation plan → Fleet and coordination](../planning/implementation-plan.md#fleet-and-coordination).
