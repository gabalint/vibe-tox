---
name: ui-screen
description: Build or change a UI screen or component from the design system, with engine-backed data through the command hooks, accessible forms and tables, and tests. Use for any work under ui/.
---

# UI screen

Read the [UI standards](../../../docs/governance/ui-standards.md) first. For a screen that lists or opens records, also read its row in the [UI information architecture](../../../docs/architecture/ui-information-architecture.md). Screens assemble existing parts; most screens need no new generic component.

## Steps

1. **Define the screen in laboratory terms.** Record:
   - the requirement or workflow IDs;
   - who uses the screen and with which capability;
   - its [kind](../../../docs/governance/ui-standards.md#screens-and-navigation): queue, directory, record page, workspace, task flow or overview;
   - where each record it lists opens: side panel, record page or neither ([record views](../../../docs/governance/ui-standards.md#record-views));
   - its data contract:
     - the engine queries and commands;
     - typical and worst-case row counts, and whether the UI or the engine filters;
     - which reads are audited;
     - what each mutation invalidates;
   - which [screen states](../../../docs/governance/ui-standards.md#screen-states) apply, and why any are left out.
2. **Inventory existing parts** ([reuse-first skill](../reuse-first/SKILL.md)) against the [one-way-to-do-each-thing table](../../../docs/governance/ui-standards.md#reuse-before-creation), `ui/src/components/`, `ui/src/lib/` and the component gallery. Write down which ones you will use. A missing generic part is built first (step 7).
3. **Data.** Use only `useCommandQuery` and `useCommandMutation`, with types from `ui/src/generated/`.
   - If a needed command does not exist, it is engine work for its owner ([engine-command skill](../engine-command/SKILL.md)). Do not fake it in TypeScript.
   - Mutations declare the queries they invalidate.
   - Enable or show actions from engine-provided `allowedActions`, never from a TypeScript rule.
   - Keep the view in the URL: find text, filters, the open record (`?open=`), the tab and the version.
   - Load sensitive records only on an explicit open, never on hover, focus or arrow keys.
4. **Compose the screen** in `ui/src/features/<area>/`, following its [page layout recipe](../../../docs/governance/page-layout-standard.md) and the [opening and action rules](../../../docs/governance/ui-standards.md#opening-and-acting-on-records):
   - One screen component per route file.
   - At most one grid that grows with the data.
   - Split it into feature-local child components once it passes about 150 lines.
   - Derive display values before `return`; keep JSX declarative.
   - Layout CSS uses tokens only.
5. **Forms** follow the [form](../../../docs/governance/ui-standards.md#forms) and [picker](../../../docs/governance/ui-standards.md#find-filter-and-pick) rules: `react-hook-form` with the field components, engine field errors on their fields, `Select` or `RecordPicker` by option count, `ConfirmDialog` for irreversible actions, and barcode fields that submit on Enter.
6. **Content.**
   - Apply the [user-manual skill](../user-manual/SKILL.md): add or review the page's bundled
     guidance in [`docs/help/`](../../../docs/help/README.md) in the same change as routes,
     labels, tasks or failure behavior. Keep required field instructions, current errors,
     identity and holds on the working screen.
   - Apply the [content rules](../../../docs/governance/ui-standards.md#content-rules) and [clinical display rules](../../../docs/governance/ui-standards.md#clinical-display-rules): exact decimal strings with units, lab time zone, delivery channel and evidence level, plain-text payloads, `IdentityBanner` and two identifiers per row, identity confirmed by entry.
   - Map each state to a tone by its [meaning](../../../docs/governance/ui-standards.md#color).
   - No usage hints such as "Select a row to …": if one seems needed, the interaction is hidden.
7. **New generic component.** Make one only when two or more features need it in this milestone. Create it in `ui/src/components/Name/` with:
   - a typed props interface;
   - variants as props;
   - CSS Module with tokens;
   - a test file;
   - an `index.ts`;
   - a gallery entry.

   Keep it domain-free.
8. **Tests.**
   - Feature flows with `mockIPC`: success, engine error (show the message and correlation ID), empty, no-results and loading.
   - Rows open by click and by Enter. Escape closes a side panel and focus returns to the row.
   - Component tests with `user-event` and an axe check.
   - Keyboard-only walkthrough of the main path.
9. **Gate.** Use the [quality-gate matrix](../quality-gate/SKILL.md#verification-matrix) to select affected-file lint, type checks, relevant tests and build/structure checks. Include consumers of shared components. Bundled help changes retain article coverage/metadata tests and a UI build; unrelated Rust checks are not required.
