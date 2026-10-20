# Page layout standard

Status: **implemented collection baseline; native-engine and platform qualification outstanding**. This document is the home of page layout: it supplements the [UI standards](ui-standards.md) with placement, spacing and acceptance rules. It assigns a predictable home to the title, actions, search, filters, counts and context, avoiding repeated headings, count lines and gaps while preserving readable controls, clinical identity and actionable warnings. Collections use a compact two-row layout: title and actions, then search, filters and count above the results.

## Four recipes, six screen kinds

The recipes describe composition; they do not change the [screen kinds](ui-standards.md#screens-and-navigation), routes, opening behavior or permission rules.

| Recipe | Screen kind | Top of page, in reading order | Working body |
| --- | --- | --- | --- |
| Collection | Directory | Title + Add action → collection toolbar → conditional filter or limit summary | One growing grid; optional record panel |
| Collection | Queue | Title + relevant page actions → collection toolbar → conditional filter or limit summary | One growing grid in the engine's queue order; row opens its record |
| Record and workspace | Record page | Breadcrumb → identifier/title, status and record actions → required identity/holds → compact facts → subject tabs when needed | Record sections or one growing child grid in the selected tab |
| Record and workspace | Workspace | Subject/title and record actions → required identity/holds → compact facts → collection toolbar when it controls the worklist | One growing grid with a decision panel; context stays with the decision |
| Task | Task flow | Breadcrumb → task title → identity/holds when relevant → step indicator only for a real multi-step process | One form; action group immediately after the current step |
| Overview | Overview | Title + subject actions → current status and genuinely distinct counts → subject tabs if needed | Short summaries linking to full screens, or one growing grid in a selected tab |

Examples: Tests and panels uses Collection/Directory; Result review uses Collection/Queue; a test version and Specimen results use Record and workspace; Register patient and New order use Task; Work queues, Host health and Backups use Overview.

## Common placement rules

| Element | Home | Rules |
| --- | --- | --- |
| Breadcrumb | Above the title on record/task pages | Omit on navigation-root directories; return to the originating collection with its safe view state restored |
| Page title | First main row, left | One `h1`; match the navigation label or name the open record; use status beside it when it describes that record |
| Primary page action | First main row, right | At most one primary action; Add belongs here on a directory. A task's Continue/Save belongs with its form, not in a competing header action group |
| Secondary page actions | Beside the primary action | Up to two direct secondary actions, then More. The [record-action rules](ui-standards.md#opening-and-acting-on-records) apply |
| Description | Below title, only when necessary | Explain a consequential scope or limitation. Do not repeat the title or add instructions such as “Select a row” |
| Search | Leftmost control in the collection toolbar | Visible “Find” or a specific label; placeholder gives an example or searchable fields. Label and placeholder are not substitutes |
| Common filters | Immediately after search | One or two most-used properties with persistent inline labels, such as “Kind” and “State”; fixed order within the feature |
| Advanced filters | “More filters” after common filters | Disclose less-used fields; preserve visible applied-filter feedback when closed. Do not ship a rail for two selects |
| Refresh and list actions | After filters, before count | Use only when supported by the query's refresh policy. Export is a collection action only where supported and authorized |
| Count and limits | Toolbar's trailing area, adjacent to its grid | One canonical count for that collection; same region at narrower widths, even when wrapped. State the counted noun and scope |
| Applied-filter summary | Directly below toolbar, when needed | Show it for non-default or hidden filters. Common inline controls already show their values; avoid repeating every value as a chip |
| Bulk selection | Directly above its grid, only while selected | Show selected count, permitted bulk actions and Clear. Keep the result count distinguishable; filter changes clear selection |
| Identity and holds | Above clinical work, visible during use | Retain required `IdentityBanner`, identifiers and engine reasons. Do not move them into More, filters, tooltips or collapsed metadata |
| Freshness | With the status or collection it describes | Only show a time or stale condition justified by the data contract. Avoid a decorative “live” indicator |

Controls are grouped by meaning, not stretched to occupy spare width. Keep search roughly 240–320px when space permits; let it shrink or fill a wrapped row before forcing horizontal scrolling. Labels, buttons and status text can wrap. Align labels inline only where the full label remains visible.

## Spacing and height budgets

These are project layout targets, not accessibility thresholds. Use the [tokens](../../ui/src/styles/tokens.css) and the sizes in the [visual design rules](ui-standards.md#visual-design), with a 24px page heading. Do not reduce type or control size to meet a height target.

| Relationship | Target spacing |
| --- | --- |
| Page inset | `--space-4` (16px); allow narrower insets only where needed for reflow |
| Title/actions to toolbar | `--space-3` (12px) |
| Between controls / inline label and control | `--space-2` (8px) |
| Toolbar to table or applicable summary | `--space-2` (8px) |
| Between meaningful body sections | `--space-6` (24px) |
| Compact facts | One wrapping definition list with 12–16px gaps; no card per fact |

Measure from the top of the page's content area, before its inset, to the top of the table header or first working content. Exclude the application shell, navigation and demo banner. Include all actual page content, including any required warnings; record the reason when conditional content increases the height.

At sufficient available width, a plain collection targets **100px to the table header**: 16px inset + 32px title/action row + 12px gap + 32px toolbar + 8px gap. A collection with wrapping labels, a filter summary or a limit explanation can exceed that target. A clinical record, task or overview has no forced 100px ceiling; assess the space occupied by its necessary context against the matching recipe.

An unneeded empty row fails the standard even if the page meets a numerical target. Necessary identity, holds, error explanations, localisation and enlarged text take precedence over the budget.

## Counts, filtering and data contracts

Count copy describes what the response proves. Filtering a loaded subset is not the same as searching the complete collection. Navigation queue counts may remain as orientation; do not add a second title badge or summary card repeating the collection toolbar's count.

| Engine/data evidence | Count wording | Required accompanying behavior |
| --- | --- | --- |
| Complete reference collection, no user narrowing | “28 tests and panels” | Say when the base scope excludes retired records |
| Complete collection, 12 of 28 match | “12 of 28 tests and panels” | Total uses the declared base scope; changing that scope changes the denominator |
| Exact count available, only first 200 loaded | “200 of 340 items” | Identify the order and any subset-only sorting; retain a path to remaining results |
| Engine count capped at 1,000 | “200 shown · 1,000 or more items” | Never present the cap as an exact total |
| Lookahead or cursor proves more, no total | “200 items shown · more available” | Preserve Load more or narrowing behavior; do not display “200 total” |
| Client search narrows an incomplete loaded set | “12 matches in 200 loaded items” | Label its restricted scope; whole-collection search requires engine support |
| Query failed or count unavailable | Omit count or show “Count unavailable” if relevant | Never replace uncertainty with zero; keep unaffected data visible |

`ListLimit` states what a limited list shows. A short count can share the toolbar; a longer explanation about oldest/newest rows, count caps or sorting belongs directly below it. Show one count per collection.

For each collection, review its query, scope, maximum returned rows, total/cursor evidence, filter ownership, refresh triggers, mutation invalidations and audited-read behavior against the source. If the response does not prove a total, use an honest “shown” count or omit it. NFR-33's 10,000-row test does not imply that every endpoint returns all rows. The shared [Directory](../../ui/src/features/configuration/Directory.tsx) handles complete reference collections; clinical queues and cursor-based lists retain their own query contracts.

## Responsive and interaction behavior

Use the width actually available to the page or list, including the effects of navigation and an open detail panel. A window breakpoint alone cannot describe that space.

| Available space / condition | Behavior |
| --- | --- |
| All toolbar controls fit | One row; search first, common filters next, actions and count trailing |
| Controls cannot fit without clipped labels | Wrap in the same reading order; search may occupy its own row; keep the count near the results |
| Extra filters or rail crowd the list | Move advanced fields to disclosure; indicate their applied values outside it |
| Detail panel leaves the list under approximately 40rem | The [side panel](ui-standards.md#side-panel) rule applies: show the record in place of the list, with Back to list |
| Narrow/reflow view | Header, search, filters and pagination reflow. A genuinely two-dimensional table may scroll horizontally within its own region |
| Long values, large text or translations | Wrap labels and actions; preserve meaningful text and visible focus; height budgets become flexible |

Keyboard order follows the visible reading order. A filter disclosure has a named button, expanded state and a programmatic connection to its panel; closing it restores focus sensibly. Announce changed results/counts politely without moving focus. Updating a list keeps existing rows visible where the current query contract permits it; first-load skeletons retain the page structure.

Keep safe reference filters, sort, selected tab and open-record navigation in the URL ([data and state](ui-standards.md#data-and-state)). **Do not add patient-identifying search text, identifiers or other PHI to URLs or persistent browser storage.** Preserve the patient's current in-memory search and return behavior. Sensitive records load only after an explicit user opening action; tighter headers do not authorize hover loading, prefetching or polling.

Sticky identity and decision bars must not obscure keyboard focus or create an unusably small working area. Preserve one growing grid per screen, and avoid an independently scrolling toolbar or filter summary.

## States to review

The implementation review must cover the [screen-state set](ui-standards.md#screen-states). Review actual application states against the engine contract.

| State | Required treatment in this standard |
| --- | --- |
| Default / dense | Same placements; longest realistic labels and capped-list wording fit without obscuring controls |
| Loading / refreshing | Stable heading and toolbar; distinguish first load from updating displayed data |
| Empty / no results | Distinguish nothing created from search/filter exclusion; offer the appropriate Add or clear action |
| Error / partial | Error stays with the failed region; preserve other usable controls and explain retry |
| Stale | Show the age or source of the affected data when needed by its contract |
| Denied / read-only | Explain missing access where relevant; actions follow engine authority |
| Held / blocked | Identity, QC and release reasons remain prominent and attributable; never collapsed for density |
| Selected / unsaved | Bulk selection and unsaved form behavior remain explicit; filter/layout changes cannot silently discard work |

## Implementation

The shared [Page](../../ui/src/components/Page/Page.tsx), [Toolbar](../../ui/src/components/Toolbar/Toolbar.tsx) and [Directory](../../ui/src/features/configuration/Directory.tsx) compose the collection baseline. Features supply domain content and query contracts; shared components own placement. Record pages retain identity, holds, breadcrumbs, tabs and permitted actions.

Use the shared components and spacing tokens. Reuse `FindField`, `FindCount` and `ListLimit` for search and count presentation; retain `IdentityBanner`, `DetailLayout` and `DetailPanel` for clinical context and record opening. Sign-in, lock and setup surfaces follow their own task flow. Layout changes do not authorize new fetching, clinical rules or an alternative table/filter system.

## Acceptance checklist

These `PL-*` identifiers name layout acceptance checks, not new clinical or regulatory requirements. They support REQ-OPS-004 (actionable work and counts), NFR-10 (faithful display), NFR-33 (cross-platform behavior), and the UI and security standards.

| Check | Evidence required |
| --- | --- |
| PL-01 Placement | Screens of the same recipe put title, actions, search, filters and counts in the same relative locations |
| PL-02 Density | Measure top-to-content at ordinary and narrow available widths; no empty rows, duplicated counts or repeated summary cards |
| PL-03 Count truth | Compare count wording with complete, filtered, capped, cursor, failed and zero-page-with-more responses |
| PL-04 Safety context | Clinical identity, two identifiers where required, holds and blocked reasons remain visible; clinical strings and units retain their meaning |
| PL-05 Navigation | Back, tabs, safe filters and record opening preserve view state; patient-identifying criteria remain out of URLs/storage |
| PL-06 Access and states | Keyboard walkthrough, focus visibility, named filters, status announcements, empty/error/refresh and permitted-action behavior |
| PL-07 Reflow | Header and controls reflow at 320 CSS px and enlarged text; test the actual supported web engines before application handoff |
| PL-08 Reuse | Shared components own placement; feature code supplies domain content and query contracts; no duplicate fetch or clinical rule |

## Research basis and decision boundaries

Primary sources checked 2026-10-14 inform this standard; source dates and applicability are registered under [ENG-227–230](../research/sources-engineering.md#page-layout-sources-2026-10-14). Their guidance supports patterns; the four recipes, exact count location and 100px target are project design rules rather than source-mandated requirements.

| Source | Supports | Boundary |
| --- | --- | --- |
| ENG-227: [IBM Carbon data table guidance](https://www.carbondesignsystem.com/building-blocks/core/components/data-table/guidelines) | Predictable title/toolbar/table grouping; search near results; conditional batch actions | Does not establish this project's pixel budgets or clinical behavior |
| ENG-228: [Cloudscape collection select filter](https://cloudscape.design/components/collection-select-filter/) | Common property filters, visible labels, accurate full-collection filtering and match feedback | Placement alternatives are guidance; a loaded subset must not pretend to be a full search |
| ENG-229: [W3C Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html) and [Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html) | Reflow of controls around tables and usable pointer targets | 32px controls are a project choice, not the WCAG minimum |
| ENG-229: [W3C Status Messages](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html) and [Focus Not Obscured](https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html) | Announce changed result status and keep focused controls visible under sticky content | Manual and automated implementation verification are still needed |
| ENG-230: [Cloudscape filter persistence](https://cloudscape.design/patterns/general/filter-patterns/filter-persistence-in-collection-views/) | Return to the prior view; avoid sensitive information in URLs | Project sensitive-read and session handling rules remain authoritative |
