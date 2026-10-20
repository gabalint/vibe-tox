---
name: user-manual
description: Create, review or maintain the bundled user manual when pages, fields, dialogs, actions, permissions, validation, configuration, statuses or failure behavior change. Use for user-visible UI and engine changes and before handing off documentation.
---

# User manual

Maintain a usable reference and task guide, not a page summary. The canonical content is
[`docs/help/*.json`](../../../docs/help/README.md); its schema and completeness contract live
in that authoring guide. Read it before editing. Do not keep duplicate manual prose in UI
components, global skills or a second document.

## Procedure

1. **Inventory the affected surface.** Start from the actual route and follow its tabs,
   child components, dialogs, expandable sections, repeatable groups, filters, columns and
   row actions. Include conditional controls and read-only facts. Trace shared controls
   into every topic that exposes them. Record the source paths in `reviewSources`.
2. **Read authoritative behavior.** Inspect visible labels, input bindings, generated
   types, engine authorization, validation, transitions and relevant tests. The rendered
   label alone is insufficient evidence for allowed values or limits. Distinguish what
   the engine accepts from what this screen lets a user enter.
3. **Write the page reference.** Cover purpose, intended task, access and prerequisites.
   For every field explain what it is, why it is used, possible values and rules/limits.
   Name exact fixed choices; identify the configuration source for dynamic choices. State
   required/optional, default/blank behavior, format, units, precision, length/range and
   read-only or conditional behavior where applicable. Do not invent an unenforced limit.
4. **Write the action reference.** Use the exact visible label, including dialog submit,
   cancel and destructive controls. Explain effect, capability/state gates, confirmation
   or reason requirements, saved versus unsaved consequences, and the next place to check
   the result. Distinguish transport evidence from completion.
5. **Write task and recovery guidance.** Provide an ordered realistic workflow,
   restrictions and known unavailable functions, then symptom-specific recovery for
   validation errors, missing configuration, permission/state blocks, stale records and
   failures that actually apply. Do not use a generic “contact admin” paragraph in place
   of the documented remedy. Do not imply retry is harmless without evidence.
6. **Respect laboratory boundaries.** Use synthetic examples. Approved policy supplies
   clinical decisions, cutoffs and interpretation; the manual explains where configured
   values come from and how software uses them. Expose implementation limitations without
   turning proposed features or policies into current functionality.
7. **Reconcile and review.** Compare the complete field/action inventory with the updated
   page entries, including previously documented behavior that depends on the change.
   Resolve contradictions with current source; report UI/engine discrepancies explicitly.
   Advance `reviewedOn` only after reviewing the whole affected topic. If no text changes
   are needed, state why in the handoff.
8. **Verify.** Follow the [quality-gate matrix](../quality-gate/SKILL.md#verification-matrix)
   and [maintenance contract](../../../docs/development/contextual-help.md). Changes to
   bundled `docs/help/*.json` require article coverage/metadata tests and the UI build.
   Add relevant feature/renderer tests and inspect search, details, keyboard controls and
   narrow-panel rendering when those behaviors or layouts are affected; content-only
   edits do not require the full application suite. Structural tests cannot prove prose
   accuracy or field completeness. Prose-only authoring-guide changes use documentation
   checks, not bundled-help checks.

## Handoff

Report topics/pages reviewed, relevant requirement IDs, source and UI checks performed,
field/action omissions or uncertainty, and exact verification commands/results. Preserve
unresolved limitations rather than claiming a complete manual from route coverage alone.
