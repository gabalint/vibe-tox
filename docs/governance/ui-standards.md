# UI standards

Status: **in force**. These rules apply to new screens and to existing screens as they change. The [UI information architecture](../architecture/ui-information-architecture.md) maps record types to their screens and distinguishes implemented behavior from remaining proposals. Sources are listed in [Sources](#sources).

The UI is a TypeScript/React app rendered by the OS web engine inside the Tauri window ([ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md)). It presents data and collects input; the Rust engine decides everything clinical ([coding standards](coding-standards.md#typescript)). Approved packages are in the [library policy](library-policy.md#npm-packages). Procedure: [ui-screen skill](../../.ai/skills/ui-screen/SKILL.md).

## Structure

```text
ui/src/
  app/          Shell, routes, navigation, providers. The only place that knows every feature.
  components/   Design system: generic, domain-free, reusable. One folder per component:
                Name.tsx, Name.module.css, Name.test.tsx, index.ts
  features/     One folder per area (for example auth, accessioning, workqueues, reports, configuration).
                Screens and feature-only parts. Uses components/ and lib/. Never imports another feature.
  lib/          ipc.ts (the only Tauri invoke wrapper), commands.ts (query and mutation hooks),
                errors.ts (CommandError to UI messages), format.ts (display helpers), test helpers
  generated/    ts-rs bindings from Rust DTOs. Generated; never edited by hand.
  styles/       tokens.css (design tokens), global.css (reset and base)
```

**Dependency direction:** `app → features → components, lib → generated`. Components never import from features or call the engine.

**Engine-aware shared parts** live in `lib/`. These are parts that call the engine and are used by two or more features, such as `CommandDialog` and `CommandConfirmDialog`. They cannot live in `components/`, which never calls the engine, and features may not import each other. Keep them few, small and domain-light: a shared part that knows one area's types belongs in that feature.

## Reuse before creation

1. **Find first.** Before writing any UI element, check `components/` and the dev component gallery for an existing one. Extend a component with a prop or variant rather than copying it.
2. **Where new code goes.** A new *generic* component goes in `components/` only when two or more features use it in the current milestone. A part used by one feature stays in that feature folder. When a second feature needs it, move it down and generalize it.
3. **No domain knowledge in components.** `components/` knows nothing about specimens, orders or results. It takes typed props, does not fetch data and holds no business rules.
4. **One way to do each thing:**

   | Need | The only approved way |
   | --- | --- |
   | Engine call | `useCommandQuery` / `useCommandMutation` from `lib/commands.ts` |
   | Form | `Form` plus `react-hook-form` plus the field components |
   | Table of records | `DataGrid` |
   | Modal | `Dialog`, or `ConfirmDialog` for destructive or irreversible actions |
   | Transient notice | `useToast`, shown in the top-right corner below the top bar |
   | Loading, empty, error | `LoadingState`, `EmptyState`, `ErrorState` |
   | Status | `StatusBadge` |
   | Page frame | `Page` with `PageHeader` |
   | One record beside its list | `DetailPanel`, placed with the list by `DetailLayout` |
   | Actions on one grid row | `RowActions`: a menu built on react-aria `Menu`. A record view lays out the same actions with `RecordActions` |
   | Peer views of one subject | `Tabs`: react-aria `Tabs`, one route per tab |
   | Find in a list | `FindField`: react-aria `SearchField` |
   | Pick one record in a form | `Select` for up to 15 fixed options; above that, `RecordPicker`, built on react-aria `ComboBox` |
   | Who and which specimen a screen is about | `IdentityBanner` in `lib/`: it reads the subject from the engine and renders a generic sticky `Banner` from `components/` |

   Each part has a test and a gallery entry. The [shared components](../architecture/ui-information-architecture.md#shared-components) list what each does. They are built on react-aria-components 1.22 primitives, with no other dependency.
5. **Styling.** Variants come from props (`variant`, `size`, `tone`), never from feature CSS overriding component internals. Feature CSS is limited to layout (grid, flex, gap) and uses tokens only. Never hard-code colors, spacing or font sizes.

## Screens and navigation

**Every screen is one of six kinds.** Choose the kind before designing. It decides the layout, what a row does and where the record opens. The [page layout standard](page-layout-standard.md) defines each kind's layout recipe, placement, spacing and [acceptance checklist](page-layout-standard.md#acceptance-checklist).

| Kind | For | A row opens |
| --- | --- | --- |
| Queue | Work waiting for a person, most urgent or oldest first, with its count beside the navigation entry: result review, release, delivery, billing work, result imports, orders needing a decision | The item's record page, with Previous and Next through the queue |
| Directory | One kind of reference record: tests and panels, analytes, clients, practitioners, instruments, users | The record in a side panel, or on its record page when the record is a hub ([record views](#record-views)) |
| Record page | One record with child lists, versions or decisions: order, accession, specimen results, report, run, client, method, test or panel | Rows of its child lists open the child record |
| Workspace | Deciding one item after another without leaving the list: result lines in technical review, QC observations | The item in the side panel that holds the decision |
| Task flow | Creating or doing something in steps: new order, accessioning, restore | Not applicable |
| Overview | The state of one subject first, then its parts: work queues, host health, backups | The item's own screen |

**One scrolling grid per screen.** A screen has at most one grid that grows with the data, and that grid fills the remaining height (`DataGrid` with `fill`). Any other list on the screen is one of two things ([ENG-163](../research/sources-engineering.md#ui-interaction-and-information-architecture-sources-2026-10-13)):

- a child list that stays small, such as a client's locations;
- a preview of at most 10 rows, ending in "View all (n)", which opens its own screen or tab.

Never stack two growing grids. They create nested scrollbars and push the second grid below the fold.

**Each record type that people manage on its own is a separate screen.** It gets its own navigation entry and directory. For example, the catalog is split into Tests and panels, Methods, Analytes and Specimen types, and Clients and Practitioners are two screens. Do not put such types in tabs. Tabs are not navigation, and large independent sets of content belong in the navigation (ENG-160, ENG-165, ENG-171).

**Tabs split one subject.** Use `Tabs` only for peer views of one record or one subject, which a person uses one at a time. Examples: a method's Content and Versions, or the Targets, Runs and Restore tests of backups.

- Use at most 6 tabs. The first holds the main task (ENG-160, ENG-163, ENG-168).
- Each tab has its own URL, so Back, reload and links work.
- Never use tabs for steps done in order (use a task flow), or for content people compare side by side (show it together).
- A grid inside a tab follows the one-scrolling-grid rule.

**Navigation has two levels: work-area headings, then screens** (ENG-167, ENG-168). An area that grows past about 8 screens splits into two areas; it does not gain a third level. Record pages are not in the navigation. They are reached from rows, links and the scan field, and show a breadcrumb to their list, for example "Tests and panels › TOXDEF". The navigation label and the page `h1` are the same words.

**Related records link to each other.** Wherever a screen names another record, the name links to it: an order's client, a result's method version, a destination's client. The domain's relationships are the navigation between records (ENG-174).

## Opening and acting on records

**One click opens a record.** A single click anywhere on a row, or Enter on any of its cells, opens the row's record. The record opens in the side panel on a directory or workspace, and on its page everywhere else (ENG-159, ENG-167, ENG-175).

- The identifier cell is styled as a link, so it is visible that rows open.
- Rows show a hover state.
- Double-click needs no handling of its own: the first click already opened the record.
- Arrow keys only move focus. They never open or load a record.

**Selection is only for bulk actions.** A grid shows the check column only when it offers an action on several records at once, such as handing off billing items together (ENG-165, ENG-166). On such a grid:

- Space toggles a row;
- a bar above the grid shows how many rows are selected, the bulk actions and Clear;
- changing the find or a filter clears the selection and says so.

No action on one record ever requires ticking a box first, and no toolbar button waits for a selection.

**Where a record's actions live:**

- **On the record view.** Every action on one record is in its side panel or page header. The main action is a primary button, up to two more are buttons, and the rest go in a "More" menu.
- **On the row, as a shortcut.** A grid may add a last column with a "⋯" button (`RowActions`, named "Actions for" plus the row's identifier). Its menu holds the same actions as the record view, from the same `allowedActions`. Right-clicking the row opens the same menu. A queue may also show one inline button for its usual next step, such as "Verify" on a destination waiting for verification. No action exists only on the row (ENG-167).
- **Only what the engine allows.** Show the actions in the record's `allowedActions` and hide the rest. When the engine says why an action is blocked, such as gate reasons or holds, the record view shows that reason where the action would be. Do not show a disabled button without a reason (ENG-164).
- **No usage hints.** A hint that explains how to use a grid ("Select a row to …") means the interaction is hidden, and these rules remove the need for it. Toolbar text is for counts and limits.

**Records open in view mode, and every change is explicit:**

- A short change opens a dialog from the action, titled with the record, for example "Edit connection of Chem analyzer 2". A short change is up to about 8 fields, or one decision with its reason.
- Longer edits put the record page into an edit state with Save and Cancel.
- A dialog that still needs a lot of scrolling, such as the AU400/AU640 connection settings, is wide (`size="wide"`). Its fields sit in `FieldSection`s: each has a visible title, and its short fields sit side by side, falling back to one column on a narrow window. The title stays at the top and Save and Cancel stay at the bottom while the fields scroll. Dialogs that fit on the screen keep the default width and a single column. A dialog that is long only for some choices widens only then: Register analyzer and Edit connection stay narrow until an AU400/AU640 connection is chosen. A long checkbox list in a wide dialog, such as Roles in Create user, uses `CheckboxGroup` with `layout="columns"`: a titled section with the options side by side.
- Every save is one engine command that carries the record's revision. Never edit in grid cells and never save when a field loses focus. Each change to audited data is a deliberate act that can be attributed to one person. This is a project rule, consistent with the explicit-confirm rule of inline-edit guidance (ENG-164, ENG-166).
- Closing a panel, dialog or page with unsaved input asks first.

**After creating a record, open it.** "Add" plus the record type ("Add client") sits in the directory header. It opens a short dialog with the required fields. On success, the new record opens in its panel or page, so the person can carry on, for example by adding the client's locations.

**Add child records from their parent.** For example, "Add location" sits in the Locations section of the client's record. Never select the parent in a list and press a toolbar button.

## Record views

| View | Use when | Rules |
| --- | --- | --- |
| Side panel (`DetailPanel`) | The record fits one column: up to about 12 facts and at most one short child list, and people move between records of one list (directories, workspaces) | See [Side panel](#side-panel). |
| Record page | The record is a hub: several child lists, versions, long content, or a decision in several parts | Own route. Breadcrumb to the list it was opened from, keeping that list's find and filters. The `h1` is the identifier and name. Previous and Next when opened from a list or queue, in that list's order. |
| Dialog | A short task on one record: create with the required fields, one edit, one decision with its reason, or a confirmation | Never used to view a record. Titled with the action and the record. |

Sources: ENG-161 to ENG-163, ENG-165 to ENG-167. Clinical systems use the same side panel: OpenMRS calls it a workspace and warns about unsaved changes (ENG-191).

### Side panel

- Docked on the right and nonmodal: the list stays usable.
- The header holds the identifier, status, Close, Previous and Next, plus "Open page" when the record also has a page.
- The open row stays highlighted.
- The open record is in the URL (`?open=<id>`).
- Escape closes the panel when focus is in it and no menu or dialog is open, and focus returns to the row.
- When the panel would leave the list narrower than about 40rem, it replaces the list and offers "Back to list".
- Its left edge is a splitter, like the navigation's: drag it, or use Left and Right arrows, Home and End. The width runs from 22rem to 64rem and never leaves the list narrower than 38rem. It is remembered on this computer for each page.
- Content too wide for the panel, such as a correction history table, asks for the widest panel while it is shown (`useWidePanel`). Hiding it returns the panel to the width chosen before.

### Record page body

Summary facts come first, then the child lists:

- Up to 3 small child lists (typically 10 rows or fewer) stack as sections.
- With more child lists, or any that grows, use tabs.
- A child list that can grow large shows its first 10 rows and "View all (n)", which opens the directory filtered to this record (ENG-163, ENG-172, ENG-173).

### Versioned records

This covers methods, tests and panels, validity policies and billing class mappings. The record page shows:

- **In the header:** the version in effect, and any draft waiting for approval.
- **Versions:** a list beside or above the content, with version, status, effective from and to, and who drafted and approved it (display names). Choosing a version shows its content.
- **The version in the URL** (`?version=`). The default is the open draft, otherwise the version in effect.
- **Lifecycle actions:** the ones the engine allows on the version shown (Edit draft, New draft from this version, Approve, Retire). When the approver rule applies, it is stated in words: "Approval needs someone other than the author, unless your laboratory's separation-of-duties policy allows self-approval." Where the policy allows it, the engine asks the author to confirm each self-approval, and the dialog shows that request with an unticked checkbox (`SelfApprovalConfirm`, [ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)).

### Sensitive records

Opening a patient, order, report or audit record is an audited read ([security standards](security-audit-logging.md)). Load such a record only when the person explicitly opens it: a click, Enter, Previous or Next, or a link. Never load it on hover, focus or arrow keys, and never by prefetch or polling.

## Find, filter and pick

- **Every directory has a Find field** (`FindField`) above the grid. It searches the identifiers and names the grid shows, such as code, name, NPI, LOINC or user name. It filters as the person types, after a short pause, and keeps the shown rows until the new answer arrives. Its text goes in the URL (`?q=`); the [page layout standard](page-layout-standard.md#counts-filtering-and-data-contracts) defines the count wording (ENG-164, ENG-168).
- **Filters on one or two properties**, such as kind, state or status, are compact `Select`s placed as the page layout standard defines. They go in the URL, with "Clear filters" whenever one is set. Where records can be retired, retired records are hidden by default and a "Show retired" filter shows them.
- **Where filtering runs.** The UI may filter and sort a collection itself when the engine returns the whole collection and the screen's data contract bounds its worst case within NFR-33 (10,000 rows). Otherwise the engine filters, sorts and pages, and the list says what it shows (`ListLimit`).
- **Empty and no results are different states.** When nothing exists yet, say so ("No analytes yet") and offer "Add analyte". When the find or a filter excludes everything, name the search ("No analytes match 'oxy'") and offer "Clear search".
- **Picking a reference record in a form.** Use `Select` for up to 15 fixed options. Above 15, and for any list that grows (clients, practitioners, tests, analytes), use `RecordPicker`: the person types to narrow the list and chooses one existing record (ENG-162, ENG-169, ENG-170). A click anywhere in the field opens the full list, as its arrow and a `Select` do; keyboard focus alone does not open it.
- **Picking a reference record in a form.** Use `Select` for up to 15 fixed options. Above 15, and for any list that grows (clients, practitioners, tests, analytes), use `RecordPicker`: the person types to narrow the list and chooses one existing record (ENG-162, ENG-169, ENG-170). Its arrow has the same icon, size, colour and position as a `Select`'s, so both kinds of dropdown look alike.
  - Each option shows what tells near-duplicates apart: code and name for a client, name and NPI for a practitioner.
  - Pickers start empty (REQ-ORD-005).
  - A choice that depends on another, such as a client location after the client, lists only the matching records and clears when its parent changes.
- **Picking several records**, such as the tests marked on a requisition. Use a labelled checklist for up to 15 options. Above that, use `RecordPicker` in multiple-selection mode with a compact, searchable, virtualized result popup. Keep chosen records visible outside the popup, each removable, so searching or filtering never hides the selection. Keep any per-record fields outside listbox options. Selection stays explicit; typing, arrow navigation and filters never select records (ENG-264/265).

## Data and state

- **Engine state** goes through TanStack Query, via the command hooks only. Each query key starts with the command name. Mutations invalidate the queries they affect, and the success path does not refetch by hand.
- **Local UI state** uses `useState` or `useReducer`. There is no global store; the [library policy](library-policy.md#rejected-or-avoided) explains why.
- **The URL holds the view:** the find text, filters, sort, the open record (`?open=`), the tab (a path segment) and the version shown (`?version=`). Back closes the panel, or returns to the list exactly as it was. Multi-row selection is session state, not part of the URL.
- **Reference data** (catalog, clients, practitioners, instruments, users) changes only through this app's own commands, which invalidate it. Its queries may therefore stay fresh until invalidated (a long `staleTime`), so moving between screens is instant. Signing out, locking and the MFA step already reset every query, so cached data never carries over to another session.
- **Sensitive reads** load only when the person explicitly opens the record ([record views](#sensitive-records)).
- **Effects.** Never use `useEffect` to fetch or derive data. Use it only to synchronize with something outside React.
- **Engine-provided permissions.** Screens show or enable actions from the engine's `allowedActions`. Never re-implement a permission or gate rule in TypeScript.

## Screen states

Each screen design and each feature test names which of these states apply, and why any state is left out:

| State | Meaning |
| --- | --- |
| `default` | Typical data, typical role |
| `loading` | First load, nothing cached: skeleton rows, with the page header already in place |
| `refreshing` | Data shown, newer data loading: rows stay, "Updated hh:mm" changes |
| `empty` | Nothing exists yet: say so and offer the first action |
| `no-results` | The find or a filter excludes everything: offer to clear it |
| `error` | The screen's main data failed: the reason and Retry, or, for `forbidden`, what to ask for |
| `partial` | One part failed and the rest works: the failed part shows its own error |
| `stale` | Data older than its freshness need: shown with when it was read |
| `dense` | Worst-case volume and the longest values |
| `denied` | A role without access, or with read-only access |
| held or blocked | A domain state such as an identity hold, a QC hold or a release gate: the engine's reasons, in laboratory terms |

## Forms

- Labels are always visible, and required fields are marked.
- Engine field errors (`CommandError.fieldErrors`) map onto the matching fields. Editing a field hides its engine error at once; the next submit checks it again. An error for a whole list of choices or rows, such as an empty test-code map, shows under that list's legend until the next submit. A field error that no field on the form shows, such as one about the instrument behind the form, appears in the error box above the fields; when no field is highlighted it replaces the engine's "Check the highlighted fields" message.
- An invalid field shows its label, border and message in the danger tone until it is corrected; an empty choice also shows its "Select…" placeholder in that tone. Submitting moves focus to the first invalid field.
- Client-side checks are limited to presence and format, for faster feedback. The engine validates everything again.
- Submit is disabled while pending. Input is kept after an error.
- **Barcode scanners** act as keyboards: a text field that submits on Enter. Do not use keystroke-timing heuristics.
- **Before an irreversible action**, the `ConfirmDialog` states what will happen in laboratory terms. Irreversible actions include release, correction, cancellation and disposal.

## Tables

`DataGrid` wraps TanStack Table with TanStack Virtual. It provides:

- sticky headers, sortable columns, keyboard navigation, and grid semantics for assistive technology;
- opening a row with a click or Enter (`onRowAction`), a hover state, and a highlighted open row;
- a check column only when the screen has bulk actions;
- an optional last column for `RowActions`;
- text that does not fit ending in "…" with the full text on hover, or wrapping for status and summary columns;
- numbers aligned right with tabular digits;
- virtualization above 100 rows.

It must stay smooth at 10,000 rows in WebView2, WKWebView and WebKitGTK (NFR-33).

**Column rules:**

- The first column is the identifier people use, such as code, accession number or name, styled as a link.
- Near-duplicate names keep a short unique key visible beside them.
- The count appears once, as the [page layout standard](page-layout-standard.md#counts-filtering-and-data-contracts) defines.

## Visual design

- **Conservative, dense and high-contrast**, suited to bench work:
  - the system font stack at a 14px base;
  - a 4px spacing scale;
  - a light gray page with white panels, grids and dialogs, because a tinted page reduces glare over a long shift (ENG-176, ENG-187).
- **Accessibility tokens.** A visible 2px focus ring. Controls at least 32px high; never below the WCAG 2.2 minimum target size of 24×24.
- **Light theme only.** Tokens are named by role, so a dark theme can be added later without any color changing its meaning. Add one only when a requirement asks for it. Light text on a dark background is not easier to read for most people (ENG-189).
- **Motion.** No animation over 150ms. Honor `prefers-reduced-motion`. Nothing flashes or blinks (ENG-178).
- **Rendering.** No canvas, WebGL or experimental CSS. Anything used must work in WebKitGTK 4.1.

### Color

The palette supports clinical bench work: routine states are neutral, and distinct attention colors retain one meaning each.

**Principles:**

1. **Color marks what needs attention.** Everything routine, finished or inactive is neutral. Bright color is kept for conditions a person must act on, so it still stands out on a busy screen (ENG-176 to ENG-178, ENG-184).
2. **Each color has one meaning and no other use.** Blue means "you can act here". Status colors never decorate. Red never marks a routine or finished state (ENG-176, ENG-178).
3. **Never color alone.** Every status shows its text and its icon (ENG-09, ENG-178, ENG-180).
4. **Weight carries urgency as well as hue.** About 1 in 12 men has a red-green color-vision deficiency, which leaves mostly a blue-yellow axis and lightness to tell colors apart (ENG-186). So the steps are plain text, then a tinted badge, then a solid badge. Each step is distinguishable without seeing hue.
5. **At most four status colors:** red, amber, raspberry and green, with green only for passing outcomes (ENG-184).

**Base and interactive tokens.** The neutral scale is a cool gray.

| Role | Token | Value | Use | Contrast |
| --- | --- | --- | --- | --- |
| Text | `--color-text` | `#1a1f26` | Body text | 16.6:1 on white |
| Muted text | `--color-text-muted` | `#525c6b` | Secondary text and identifiers | 6.3:1 on the page, 5.8:1 on the open-row tint |
| Page | `--color-surface-sunken` | `#f6f7f9` | Page background | Not applicable |
| Surface | `--color-surface` | `#ffffff` | Panels, grids, dialogs | Not applicable |
| Control border | `--color-border-control` | `#6a7484` | Inputs and other controls | 4.7:1 on white |
| Accent | `--color-accent` | `#0b5cad` | Primary buttons, links, the open-row bar | 6.7:1 on white; white text on it 6.7:1 |
| Accent hover | `--color-accent-hover` | `#094b8e` | Hovered primary button | White text 8.7:1 |
| Accent subtle | `--color-accent-subtle` | `#e7eff9` | Open-row and selected-row tint | Text 14.3:1 on it |
| Focus | `--color-focus` | Same as the accent | Focus ring | 6.7:1 on white, 5.8:1 on the row tint |

The accent is blue because blue carries no hazard meaning in medical alarm and safety-sign color schemes (ENG-185). Clinical and public-sector design systems use one blue for interactive elements (ENG-187, ENG-188). Teal and cyan are not used, because cyan is the low-priority alarm color for medical equipment (ENG-185).

**Status tones:**

| Tone | Means | Badge | Values | Examples |
| --- | --- | --- | --- | --- |
| `neutral` | Routine, finished or inactive | Plain text with a muted icon; no fill or border | Text `#3c4550` | Active, Approved, Enabled, Reviewed, Final, Current, Superseded, Retired, Withdrawn, Rejected, Closed |
| `info` | The system is working or waiting; informational notices | Quiet tint | Fill `#edf1f6`, border `#7d8ca1`, text `#3a4b61` | Queued, Rendering, Received |
| `warning` | A person needs to act, but nothing is blocked yet | Tint | Fill `#fff1c7`, border `#b07800`, text `#6b4500` | Draft awaiting approval, Needs a decision, Review out of date, Partial, Not connected |
| `hold` | A hold stops release until someone resolves it | Solid | Fill `#9c1f6e` with white text; alert tint `#f8e8f3` with text `#7d1858` | Identity hold, QC hold, Clarification hold |
| `danger` | Clinical work is stopped or has failed, or the laboratory's rules flag a result as critical | Solid | Fill `#b3221b` with white text; alert tint `#fdeceb` with text `#a3170f` | Label print failed, Out of service, Quarantined, Release blocked |
| `success` | A check or action the person just ran has passed | Tint, only in toasts and in outcomes of a check | Fill `#e5f3e9`, border `#2f7d4a`, text `#165c31` | Saved, Restore test passed, Chain verified |

Every text value is at least 6.6:1 on its fill, above the 4.5:1 that WCAG AA requires. Fills and borders are at least 3:1 against white, except tints, which always carry text and an icon. All values were checked on 2026-10-13 for WCAG contrast. They were also checked for separation under simulated protanopia and deuteranopia (the Machado 2009 model, ENG-190). Hold, danger and the accent stay apart under both.

**Rules for using color:**

- **No green for a steady state, no red for a finished one.** A record that is simply active, approved or final is neutral. A record that is withdrawn, rejected or expired and needs nothing more is neutral too, and its label says what happened.
- **A state shared by every row is plain text.** When every row of a list is in the state the list is about, such as every row of the identity-holds queue, that column shows plain text, not a column of identical badges.
- **Alerts** (banners) use the tone's tint, a solid left edge in the tone's color, its icon and a title. Critical alerts never close on their own.
- **Red fill is only for status.** Destructive buttons, such as Withdraw report or Reject order, use the `danger` text style on a neutral button. Their `ConfirmDialog` states the consequence.
- **Rows.** Hover uses `--color-surface-hover`. The open or selected row uses the accent-subtle tint plus a 3px accent bar on its left edge, so the shape marks it without relying on color. There is no zebra striping.
- **Result flags.** Flags come only from the engine, which applies the laboratory's rules. The flag's text and icon sit next to the value. A critical flag uses `danger`, and any other flag uses `warning`. A result with no flag gets no color and no "Normal" label, because a missing flag does not mean normal (ENG-180). Never color a value merely because it is positive or high; this is a project display rule.
- **Charts** use a categorical palette that stays distinct for color-vision deficiencies (Okabe-Ito, ENG-186), never the status colors. No chart exists yet; choose the palette with the first one.
- **Checking changes.** Any change to a color token re-runs the contrast and color-vision checks. The token test (`ui/src/styles/tokens.test.ts`) holds the pairs above.

## Clinical display rules

These come from health-IT safety guidance for clinical software. They apply to every screen about a patient, order, specimen, result or report.

- **Identity banner.** Every screen about one patient, order, specimen or report shows the identity banner under the page title. It stays in view while the page scrolls (ENG-179 to ENG-181).
  - It shows at least two identifiers: the family name in capitals, the given names, the date of birth with the month as text and the age, and the medical record number.
  - On specimen screens, it adds the accession number and the container label code.
  - The primary identifier is bold (ENG-191).
  - When the engine reports an identity hold or a possible match, the banner shows it.
  - The banner is one shared part (`IdentityBanner` in `lib/`). It reads identity from the engine, so it lives in `lib/`, not in the domain-free `components/`.
- **Two identifiers per row.** Lists of patients or specimens show two identifiers on every row, so similar names stay apart (ENG-180, ENG-181).
- **Confirm identity by entry, not by OK.** Where a person confirms that a patient or container is the right one, they scan or type an identifier. They never press OK on a prompt. In a randomized trial, re-entering identifiers cut wrong-patient orders by 41%, against 16% for a click-to-confirm alert (ENG-182).
- **No blank cells.** Show "Not provided", "Not reported" or a muted `0` for a real zero (ENG-188).
- **Results are never cut off.** A result value is never shortened without a sign. If a value does not fit, the cell wraps or links to the full value (ENG-178).
- **A value and its unit stay on one line**, joined by a non-breaking space.
- **Names display exactly as configured.** Drug and analyte names show as the laboratory configured them. The laboratory may configure tall-man forms for look-alike names such as oxycodone and oxymorphone (ENG-183). The UI never changes a name's letters or case.
- **Keep the reported precision.** ISMP's rule against trailing zeros applies to medication orders, not to measured results (ENG-183). Results keep the engine's string exactly ([content rules](#content-rules)).
- **Edit mode looks different from view mode.** A record being edited shows form fields and an "Editing" label. A record being viewed shows text, not disabled inputs (ENG-179).
- **Typed input is never lost silently.** An idle lock, a dialog or a move to another screen never silently discards typed input. Either keep it until the person returns, or ask first (ENG-179).

## Content rules

- Plain language, sentence case, and laboratory terms rather than developer terms. Status labels are adjectives or past participles ("Held", "Approved"), not verbs (ENG-187).
- **Decimals** display exactly as the engine string, with units always shown. Comparators appear as received, for example `<0.50 ng/mL`.
- **Times** display in the laboratory time zone, formatted `2026-10-12 14:03`, with the zone where it could be ambiguous. Unknown or estimated times display as such, never as a guess.
- **Dates of birth** display with the month as text and the age, for example `Nov 9, 1961 (64 y)`. People read them aloud and compare them with a requisition or an ID card, where a numeric month and day are easy to swap (ENG-179).
- **Delivery status** always names the channel and evidence level, never a bare "Delivered" (REQ-RPT-011).
- **Payload text** (analyzer, HL7 or imported) renders as plain text.

## Accessibility (WCAG 2.2 AA, NFR-09)

- Build on `react-aria-components`, which provides semantics, keyboard support and focus management.
- Every input has a label. Every page has one `h1` and ordered headings. Dialogs trap and restore focus. Async outcomes are announced through the toast live region.
- Text contrast is at least 4.5:1, and non-text contrast at least 3:1.
- Each component test includes an automated axe check (`expectNoA11yViolations`). Keyboard-only walkthroughs of critical journeys are part of milestone review.
- **Keys behave the same on every screen** (ENG-167, ENG-170):

  | Key | Does |
  | --- | --- |
  | Tab | Moves between regions and controls. A grid is one tab stop. |
  | Arrow keys | Move between grid cells. They never open a record. |
  | Enter | Opens the focused row. In a form, submits. |
  | Space | Toggles the focused row's selection, on grids with bulk actions only. |
  | Shift+F10 or the context-menu key | Opens the focused row's actions menu. |
  | Escape | Closes the innermost open menu, dialog or side panel. Focus returns to what opened it. |

## Tests

- **Components:** render, interaction with `@testing-library/user-event`, and axe, written against roles and labels, never CSS classes.
- **Features:** the main flows, with the engine mocked at the IPC boundary through `mockIPC` from `@tauri-apps/api/mocks`. Include the error, empty and no-results states. For grids, cover opening a row by click and by Enter. For panels, cover Escape closing the panel with focus back on the row.
- **No DOM snapshot tests.** They are brittle and prove nothing.

## Sources

These rules rest on these sources, registered with their checked dates in the [engineering source register](../research/sources-engineering.md#ui-interaction-and-information-architecture-sources-2026-10-13). Rules with no source listed are project decisions.

| ID | Supports |
| --- | --- |
| ENG-159 | One click opens a record. Inline row actions only for one or two actions. A nonmodal panel keeps the list in view. |
| ENG-160 | Tabs are few, used for peer content one at a time, and never for comparison. In-page tabs are not navigation. |
| ENG-161 | Modals cost context; nonmodal views keep it. |
| ENG-162 | Use a filterable text field instead of a dropdown above about 15 options. |
| ENG-163 | Side panel, details page and hub previews ("View all"); at most 7 tabs, one task each. |
| ENG-164 | Text filter, explicit confirm for inline edit, and when to hide or disable an action. |
| ENG-165 | Batch actions appear with selection. Tabs are not navigation and not for steps. Use full pages, not modals, for complex tasks. |
| ENG-166 | Primary-detail with the list kept visible. Single-row actions in a row menu. No selection without actions. |
| ENG-167 | Desktop keys, item click opens, every item command in its context menu, list and details side by side, and two navigation levels. |
| ENG-168 | Sidebars with at most two levels, at most 6 tabs, a persistently highlighted selection, and a search field above its list. |
| ENG-169 | React Aria selection behaviour, row actions, ComboBox and Autocomplete. |
| ENG-170 | WAI-ARIA grid, modal dialog and combobox keyboard patterns. |
| ENG-171 | Tabs are not page navigation and not for content needed at the same time. |
| ENG-172 | Use subgrids sparingly; move them to tabs; keep primary data first. |
| ENG-173 | Related lists preview a few records, with "View all". |
| ENG-174 | Object relationships define contextual navigation. |
| ENG-175 | A row opens its detail page. |
| ENG-176 | Bright color only for abnormal situations; alarm colors used for nothing else; red/green on/off coding is improper; gray backgrounds reduce glare. |
| ENG-177 | Alarm colors are bright and used for nothing else, on non-saturated backgrounds. |
| ENG-178 | Red reserved for errors and yellow for warnings; color backed by another cue; no flashing; no silent truncation. |
| ENG-179 | Patient identification at the upper left of every screen; month as text; input mode distinct from view mode; unsaved work preserved. |
| ENG-180 | Identifiers on all screens; two identifiers in lists; critical flagged apart from abnormal; no flag does not mean normal; color never the only indicator. |
| ENG-181 | Accurate identity on all screens; at least two identifiers. |
| ENG-182 | Re-entering identifiers prevents more wrong-patient orders than a click-to-confirm alert. |
| ENG-183 | Tall-man lettering for look-alike names; trailing-zero rules for medication orders. |
| ENG-184 | Red only for critical information; at most four color codes. |
| ENG-185 | Medical alarms: red high priority, cyan low priority. Safety signs: red danger, yellow caution, green safe, blue mandatory or notice. |
| ENG-186 | Prevalence of red-green color-vision deficiency, and palettes that stay distinct for it. |
| ENG-187 | A tinted page reduces glare. Blue links. Status tags are adjectives and never color alone. |
| ENG-188 | One blue for links and primary actions. No blank table cells. |
| ENG-189 | Light mode reads better for most people; dark mode is optional. |
| ENG-190 | The color-vision simulation used to check the palette. |
| ENG-191 | The clinical side panel ("workspace") with an unsaved-changes warning, and identifiers emphasized in the patient header. |
