---
name: quality-gate
description: Select and run verification for the actual changes and their dependents, interpret failures and report results and justified skips. Use before every handoff and in coordinator integration; full checks apply to cross-cutting changes and releases.
---

# Quality gate

This is the canonical change-sensitive verification policy, accepted by user
instruction on 2026-10-14. It applies locally and in CI. A change is done when
checks for its actual impact pass ([definition of done](../../../docs/governance/coding-standards.md#definition-of-done)).
Fix causes rather than suppressing findings. Do not run unrelated application
tests or builds for documentation, license or static website copy changes.

## Select the checks

Run the shared selector from the repository root before verification:

```bash
python3 scripts/select_checks.py
python3 scripts/select_checks.py --base <base-ref> --head <head-ref>
python3 scripts/select_checks.py --full
```

The default compares the working tree with `HEAD`, including tracked and untracked
changes. `--base` chooses another baseline; explicit `--head` compares committed
revisions. The JSON output is a plan with reasons, not evidence that checks ran.
CI uses the same selector for its comparison and selects subsystems
conservatively: Rust impact runs the existing workspace/platform
coverage, UI implementation impact runs the UI suite, and help-content impact
runs the focused article test and UI build. It does not automate local crate or
individual-test dependency discovery. See the
[developer guide](../../../docs/development/README.md#quality-gate) for execution.

Inspect the diff as well as the selected paths. For mixed changes, take the union
of applicable rows below. A license-only field change inside a manifest is not a
dependency update; a change to bundled JSON is not merely prose documentation.
Include affected callers, dependents, generated interfaces and configuration.
`docs/architecture/backup-and-export-formats.md` is embedded by Rust runtime/test
code and needs Rust checks despite its Markdown extension. Generated TypeScript
changes need their Rust generation/drift check as well as UI checks. Unknown
inputs or an unavailable comparison baseline select full verification. If impact
cannot be bounded confidently, broaden checks and explain why; use `--full` for
cross-cutting changes and release verification.

## Verification matrix

| Change | Required verification | Scope and exclusions |
| --- | --- | --- |
| Documentation or agent instructions | Formatting/whitespace, affected local links and anchors, tables, stable IDs, source support and skill frontmatter where relevant | No application tests, dependency scans or builds for prose alone. Review any workflow instruction against its implementation. |
| License text, notices or license-only metadata | License text/version, SPDX and metadata consistency, required notices, applicable compatibility and dependency-license checks | `cargo deny check licenses` covers Rust license policy. Review other affected licenses separately. No app tests/builds unless runtime or build inputs also change; a scan does not certify a complete release inventory. |
| Static website copy or links | Changed-file format/lint and link checks; check related captions/transcripts if affected | No application tests/builds. Browser checks only when the change affects layout, rendering or interaction. |
| Static website layout, styles or behavior | Relevant format/lint, link checks and focused browser checks for changed layout/interaction, keyboard access and responsive states | Test the website, not the separate LIS application. |
| Bundled help (`docs/help/*.json`) | Content/source review, article coverage/metadata tests and UI build | These files ship in the app. Run `npx vitest run ui/src/app/help/articles.test.ts` and `npm run build`; add renderer/feature tests or browser checks when their behavior/layout is affected. Prose authoring guides stay in the documentation row. |
| UI source or generated UI interfaces | Affected-file format/lint, type checking, relevant feature/component tests; UI build when bundled output is affected | Include consumers of shared components/hooks/types. Run unused-code, duplication and file-length checks when source/dependency structure is affected. Full UI tests apply to cross-cutting UI changes; Rust checks apply only when its inputs/contracts are affected. |
| Rust source, SQL, templates or engine fixtures | Format, clippy and relevant tests for affected crates and dependents; relevant migration/contract/adverse scenarios | Use `-p` and test filters while iterating. Include engine/shell consumers when affected. Run dependency/duplication checks for relevant changes; workspace-wide checks apply to cross-cutting changes. |
| Dependencies, toolchain or build configuration | Applicable license/advisory/unused-dependency checks and checks for the affected runtime/build graph | Root shared dependency/toolchain or cross-language changes may require full application checks. License-only manifest changes use the license row. |
| Verification scripts or CI workflow | Script/selector tests, syntax/configuration checks and review of selected/skipped job behavior | Exercise change classification, comparison failures and job dependencies. Run application checks only when the change also affects application/build behavior or is needed to validate the workflow change. |
| Release, milestone exit or explicitly requested full verification | Full automated suite, dependency policies, builds and the applicable platform/recovery/release checks | Use `--full`; follow the [release gates](../../../docs/quality/validation-plan.md#stage-gates). Passing local tests does not establish clinical validation or authorize publishing. |

## Execute only the applicable commands

For Rust, repeat `-p <crate>` for the affected packages and dependents. Examples:

```bash
cargo fmt -p <crate> -- --check
cargo clippy -p <crate> --all-targets --locked -- -D warnings
cargo test -p <crate> --locked
cargo machete crates/<crate>
```

For UI work, use `npx biome check <changed-files>`, `npm run typecheck` and
`npx vitest run <relevant-test-files>`. Use `npm run build` for changed bundled
output. `npm run knip`, `npm run dup` and `npm run lines` check relevant source
structure; they are not prerequisites for prose or license-only handoffs.
The duplication limit remains 8 lines / 60 tokens in non-test code. Resolve clones
by reuse/extraction, not by reshuffling code to defeat detection.

Full application verification includes the workspace forms of Rust format,
clippy and tests, `cargo deny check`, `cargo machete`, `npm run check` and
`npm run build`, plus any required native/platform and release checks. These
commands are not an unconditional per-handoff checklist.

## Reuse results and coordinate workers

A passing result remains usable while its relevant source, dependency, generated,
configuration and tool inputs are unchanged. After a correction, rerun only checks
whose inputs or conclusions it can affect. Expand or repeat verification for new
failures or unresolved concerns, not merely because another document was edited.
Record the earlier run and why it still applies; never describe an unrun check as
passed. The selector does not itself prove prior results remain valid.

Parallel workers keep explicit path ownership, use their assigned warm Cargo
target directory and scope commands to their impact. Never write the git index or
history. Do not bypass the shared Cargo queue or create a fresh target merely to
avoid a lock. The coordinator combines completed changes and runs only integration
checks not already covered by valid worker evidence; see the
[coordination rules](../../../docs/planning/implementation-plan.md#fleet-and-coordination).

## Review and handoff

Review the affected domain invariants, typed errors, authorization/audit, PHI-free
logging and complexity budgets when the change touches them. New commands retain
the [engine-command minimum tests](../engine-command/SKILL.md#steps). User-visible
behavior changes retain the [user-manual review](../user-manual/SKILL.md); structural
coverage does not prove prose accuracy. Confirm owned paths and explain any
remaining warning or local suppression.

Report the change scope, exact commands/results, relevant checks skipped and why,
reused evidence, and remaining limits. Keep records proportionate: a concise
handoff is enough for a routine correction. Store run records as
[AGENTS.md](../../../AGENTS.md) directs; current validation policy, release criteria
and unresolved limitations belong in `docs/quality/`. A separately authorized
release evidence package must contain its own reproducible evidence. Historical
test results are not fresh checks.
