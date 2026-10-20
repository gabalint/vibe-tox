---
name: reuse-first
description: Before writing new code or UI, find and reuse what exists; place shared code in its single correct home; extract on the third repetition. Use at the start of every implementation task and when duplication is reported.
---

# Reuse first

Duplicate and redundant code is the most common defect in AI-written changes. Run this procedure **before** writing code, and again before handing off.

## 1. Search before you write

- **Name the capability first**, for example "parse a lexical decimal", "show a status", "append an audit event" or "lease outbox rows". Search for it by concept, not only by your intended function name:
  - `rg -n "fn .*decimal|Decimal" crates/`
  - `rg -n "export function|export const" ui/src/components ui/src/lib`
  - `rg -ni "status(badge|tone)" ui/src`
- **Read the crate map** in [the implementation plan](../../../docs/planning/implementation-plan.md#crate-map) and the public API (`lib.rs` re-exports) of the crates below yours. Shared kernel types (IDs, decimals, time, principals, `CommandError`) live in `lis-core`. Do not redefine them.
- **For UI**, check `ui/src/components/`, the dev component gallery and `ui/src/lib/` before creating anything ([UI standards](../../../docs/governance/ui-standards.md#reuse-before-creation)).

## 2. Decide where the code belongs

| Situation | Action |
| --- | --- |
| Something already does it | Call it. If it nearly fits, extend it with a parameter or variant. Copying and tweaking it is not allowed. |
| It belongs to a crate or feature you do not own | Do not edit it. Report `BLOCKED:` or add a "Requests" note in your handoff, with the proposed change. |
| A domain rule (gate, threshold, mapping, state transition) | Exactly one function in the owning domain crate. Every caller uses it. |
| Needed in one place only | Keep it private to that module, `pub(crate)` or feature-local. |
| Needed by two crates or features now | Move it to the lowest owner both depend on, `lis-core` or `ui/src/components/` or `lib/`, as a small, named, tested unit. |
| Similar-looking code with a different meaning | Keep it separate. Merging accidental look-alikes couples unrelated rules. |

Never create `utils`, `helpers`, `common` or `misc` modules. Name the module after the concept it owns.

## 3. Rule of three for incidental code

Two short copies (a few lines) may stay. On the third, extract a function, component or test fixture builder. Test setup counts: use the crate's `testing` module builders instead of copy-pasted fixtures.

## 4. Remove redundancy

- Delete code your change made unused, and do not leave old paths "just in case". This covers dead functions, unused exports, unused dependencies and commented-out blocks.
- Do not add wrappers that only forward to another function, configuration nobody sets, or abstraction layers with one implementation.
- Do not re-validate in a lower layer what a parsed domain type already guarantees. Do validate again at every trust boundary: UI to engine, and external bytes to engine.

## 5. Verify

Select duplication and unused-code checks for the affected code and dependencies using the [quality-gate matrix](../quality-gate/SKILL.md#verification-matrix). Run `jscpd`, `knip` or `cargo machete` only where relevant; prose and license-only edits do not trigger them. Fix findings by reusing or extracting code, not by suppressing them. In the handoff, list shared units added or extended and callers switched to them.
