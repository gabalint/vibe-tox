---
name: simplicity-review
description: Review code (your own before handoff, or another's) for KISS/YAGNI violations, spaghetti structure, excessive complexity and redundancy, and fix them. Use before every handoff and in coordinator reviews.
---

# Simplicity review

Apply this to every change before handoff, limited to its affected content and code.
Use the [quality-gate matrix](../quality-gate/SKILL.md#verification-matrix) to select
checks; reviewing prose does not require code analysis tools. The budgets are in [coding standards](../../../docs/governance/coding-standards.md#complexity-budgets). Clinical correctness and the safety invariants in [AGENTS.md](../../../AGENTS.md#domain-invariants) always outrank brevity. Simple means "nothing unnecessary", not "fewer checks".

## Smells and fixes

| Smell | Look for | Fix |
| --- | --- | --- |
| **Speculative generality (YAGNI)** | A trait or interface with one implementation, a generic used with one type, unused parameters, options or variants, "for later" hooks, a config flag nobody sets | Delete it. Use the concrete type. Add the abstraction only when the second real use arrives. |
| **Over-layering** | A function that only forwards, a DTO identical to the domain type with a copy function, a manager wrapping a service wrapping a repo | Collapse the layers. Keep a separate DTO only when the shapes truly differ, such as hiding fields or converting decimals to strings. |
| **Long or deep function** | Over the line or nesting budget, many flags, mixed concerns (parse, decide and store together) | Early returns. Extract named steps that follow the command pipeline: parse, validate, authorize, gates, change, audit, outbox. |
| **Boolean or flag arguments** | `do_thing(x, true, false)` | An enum, or two well-named functions |
| **Primitive obsession** | `String` IDs, `&str` states, `f64` quantities, `(i64, i64)` tuples | `lis-core` newtypes, domain enums, small structs |
| **Spaghetti dependencies** | A crate reaching into another module's tables or private items; a feature importing another feature; circular module references; a mutable singleton | Call the owner's public API or view. Move shared parts down. Pass dependencies explicitly. |
| **Hidden control flow** | Errors swallowed (`let _ =`, `.ok()`, empty `catch`), defaults hiding failures (`unwrap_or_default` on clinical data), side effects in getters | Propagate typed errors. Make failure states explicit (REQ-OPS-004, NFR-06). |
| **Clever code** | Dense iterator chains needing comments to follow, macro tricks, type-level gymnastics, regexes for structured data | Plain loops and named intermediate values; a real parser for structured formats |
| **Duplication or redundancy** | Copy-pasted blocks, the same SQL or rule in two places, re-checking what a type guarantees, dead code | The [reuse-first skill](../reuse-first/SKILL.md) |
| **Comment noise** | Comments restating code, stale TODOs, commented-out code | Delete them. Keep why-comments and requirement IDs. |
| **UI complexity** | Components over about 150 lines, `useEffect` data fetching, prop drilling through more than 2 levels, inline styles, logic in JSX | Split into child components. Use the command hooks. Use composition, or context for truly cross-cutting values. CSS Modules. Derive values before the return. |

## Procedure

1. Read the diff top-down, as a newcomer would. Every name should say what it is for. If you cannot explain a unit in one sentence, it does too much.
2. Check every new abstraction (trait, generic, module, component, hook, config option) for a second real user in this milestone. If there is none, inline or delete it.
3. Check every function against the budgets: lines, cognitive complexity, nesting and parameters. Run the linters rather than eyeballing ([quality-gate skill](../quality-gate/SKILL.md)).
4. Check dependencies: crate graph direction, table ownership and feature isolation.
5. Check that errors and failure states are explicit and that nothing clinical is defaulted.
6. Fix what you find, rerun only checks affected by those fixes, and record any intentional exception, with its reason, in the handoff. Reuse valid results for unchanged inputs under the quality-gate policy.

When reviewing someone else's change, report findings as: file and line, smell, why it matters, and the concrete simpler alternative. Do not report style preferences that the formatter or linter already settles.
