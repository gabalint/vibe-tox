# Authoring the user manual

The JSON files here are the canonical, bundled user manual for the signed-in application.
This guide defines their content. The [maintenance contract](../development/contextual-help.md)
describes delivery, scope, rendering and verification; the
[user-manual skill](../../.ai/skills/user-manual/SKILL.md) gives the review procedure for
each relevant UI or engine change.

## Completeness contract

A topic summary is not a manual. Inspect the actual page, its child components, tabs,
dialogs, repeatable groups, filters, table facts and row actions. Trace the inputs through
engine validation and authorization. Each page reference must let a user answer these questions:

| Surface | Required explanation |
| --- | --- |
| Page | What task does it support, who can use it, what prerequisites apply, and where does it fit in the workflow? |
| Input or filter | What is it and why is it used? Is it required or optional? What happens when blank? What are its default, format, units, precision, allowed values and enforced limits? What makes it visible, editable or disabled? |
| Read-only field, column or status | What does the value mean, where does it come from, what values may appear, and what does it establish or leave uncertain? |
| Configured selection | Name the source of choices, active/version restrictions and what to do if a choice is missing. Do not substitute a fictional fixed list for lab-maintained data. |
| Action, including dialog controls | Use the exact label. Explain its effect, capability and state gates, confirmation/reason requirements, saved versus unsaved effects, resulting state and next step. |
| Workflow | Give practical ordered steps, including verification of the result and relevant alternate or blocked paths. |
| Restriction | Name unavailable behavior, safety holds, authorization boundaries, policy dependencies and the consequence of a limitation. |
| Failure | Name the observable problem, likely cause where known, and a concrete safe recovery. Distinguish retry, correction, escalation and an unsupported operation. |

Document what this UI supports separately from a broader engine API. Do not claim that an
engine-supported field is editable if the form does not expose it. Never invent a clinical
cutoff, interpretation, SOP or permission. A known UI/engine discrepancy belongs in the
limitations and handoff. Use plain laboratory language and synthetic examples only.

## Topic contract

One `*.json` file contains one topic. All prose is plain text, without HTML, Markdown,
executable content, record data or remote links. Keep stable IDs when names change.

| Field | Type | Rule |
| --- | --- | --- |
| `id` | String | Unique kebab-case ID matching the filename. |
| `title` | String | Concise user-facing subject. |
| `summary` | String | Purpose in one or two sentences; detailed content belongs below. |
| `reviewedOn` | String | Valid `YYYY-MM-DD` date of the latest review against implemented behavior, not an automatic build timestamp. |
| `routes` | String array | Exact patterns from navigation/detail screens, including parameters and optional `/:tab?`. Only `getting-started` has no routes. |
| `reviewSources` | String array | Existing repository-relative source/test files actually inspected. Include relevant UI and engine files; these are review metadata, not user-facing prose. |
| `pages` | Object array | Nonempty page references using the contract below. Together their routes cover the topic's routes exactly once. Separate list/detail/task pages when their fields or actions differ. |
| `sections` | Object array | Background and good-practice explanations, shown separately from the precise reference. Each has a unique `title`, `paragraphs` string array, and optional `steps` string array. At least one paragraph or step per section. |

### Page reference

| Field | Type | Rule |
| --- | --- | --- |
| `title` | String | Unique within its topic; identifies the actual page or closely shared task. |
| `routes` | String array | The topic's route patterns served by this entry. Tabs sharing an optional route require explicitly named tab fields/actions. |
| `purpose` | String | What the user accomplishes and when to use it. |
| `access` | String | Roles/capabilities and prerequisite records/configuration; distinguish read from write authority. |
| `fields` | Object array | Each has unique `name`, `meaning`, `values`, `rules`. Cover every visible input, selection, filter and meaningful displayed fact, including dialogs. Qualify repeated labels by dialog or group. |
| `fields[].meaning` | String | What it is and what it is used for. |
| `fields[].values` | String | Exact fixed choices, accepted format/range or configured source, including meaningful blank/default values. |
| `fields[].rules` | String | Required/optional, limits, dependencies, read-only conditions and limitations, as applicable. |
| `actions` | Object array | Each has unique `name`, `does`, `availableWhen`, `result`. Include dialog/row/navigation actions. An empty array is permitted only for a page with no page-specific actions. |
| `actions[].availableWhen` | String | Capability/state/configuration conditions and why the action can be blocked. |
| `actions[].result` | String | What changes, what remains pending, and where to verify or continue. |
| `restrictions` | String array | Concrete page-specific limits and policy boundaries. |
| `workflow` | String array | Ordered task steps, separate from the field/action reference. |
| `troubleshooting` | Object array | Each has `problem` and `recovery`; symptom-specific guidance grounded in implementation. |

Use existing topics as structural examples after reviewing their source. Do not copy generic
prose to satisfy a length threshold. There is no word-count target. Group inseparable fields
only when every member's meaning, values and limits remain explicit; broad labels such as
“Other fields” hide omissions. Shared dialogs may reference a clearly named canonical topic,
but each entry point must still explain prerequisites, effects and key field constraints.

Take route patterns from the route inventory named in the
[maintenance contract](../development/contextual-help.md#content-and-loading). A new page
needs one explicit mapping at both topic and page-reference level. The fallback manual entry
has empty route arrays.

## Update a topic

1. Inventory the changed surface and inspect the relevant UI, engine rules and tests.
2. Update all affected page references and background sections in the same change. Refresh
   `reviewSources` to the actual files inspected, removing renamed or deleted paths.
3. Compare the page/control inventory with the manual; check conditional and failure paths.
   Review the whole affected topic before advancing `reviewedOn`. If the text remains
   correct, record why in the change review rather than skipping the review.
4. Verify as the [quality-gate matrix](../../.ai/skills/quality-gate/SKILL.md#verification-matrix)
   requires for bundled help:

   ```bash
   npx vitest run ui/src/app/help/articles.test.ts
   npm run build
   ```

   Inspect rendered page selection, field/action details, search and narrow layout when
   those behaviors or layouts are affected. Report actual checks/results, relevant skips
   and remaining limits.

Structural tests catch missing/duplicate/stale routes, invalid structure, empty reference
attributes and missing source files. They do **not** prove that every control is covered or
that prose matches behavior; the source-to-manual review is required. The manual explains
software use; essential identity, holds, field constraints, current errors and
irreversible-action consequences stay visible in the workflow. A documentation review is not
laboratory approval or clinical validation.
