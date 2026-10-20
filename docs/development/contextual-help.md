# Contextual page help

This is the maintenance contract for on-screen documentation in the signed-in local application.
Help explains software use; laboratory policies remain supplied or approved by the responsible
laboratory.

The application provides one **Page help** button in the top bar. It opens documentation for
the current page, with a searchable library of bundled topics. Content lives in
[`docs/help/`](../help/README.md) and ships with the application, so the installed software
and its instructions travel together and work offline.

## Interaction contract

| Situation | Behavior |
| --- | --- |
| Initial signed-in view | Help is collapsed. The Page help control stays in the same top-bar position across pages. |
| Open help | Load the help feature from bundled assets on first use and show a labelled, nonmodal aside. The current page and its input remain available. Opening help does not navigate away or submit a form. |
| Wide window, above 85rem | Place help beside the page; default width 400px, normally 320–800px, capped to leave 480px for the working page. |
| Narrow window or increased zoom | Place help below the page; default height 320px, minimum normally 160px, maximum 60% of actual working height after notices. Re-clamp bounds as geometry changes. |
| Resize help | Drag the left divider (side layout) or top divider (bottom layout). The focusable separator exposes actual pixel bounds. Arrow keys move it physically in 32px steps; Home/End use minimum/maximum. Size options provide narrower/wider or shorter/taller actions and reset without dragging. |
| Size persistence | Save width and height independently in local UI preferences after a drag ends. Invalid or unavailable storage falls back safely; pointer cancellation restores the starting size. Bounds temporarily constrain the saved preference when a window shrinks. |
| Navigation | Reset the selected topic to the current route's guide. Details and tabbed pages have explicit route coverage. |
| Find another topic | Search the local topic library without changing the working page or sending the search to a service. |
| Close help | Hide the aside and return focus sensibly to Page help when closing from within it. Escape respects an inner open menu or dialog. |
| Reopen on the same page | Preserve the selected help topic. |

Use a real button with an accurate `aria-expanded` state and a programmatic relationship to
the panel. Give the aside a heading and accessible name; keep its headings ordered and its
controls reachable by keyboard. A nonmodal panel does not trap focus or mark the working
page inert. These semantics follow W3C's
[disclosure pattern](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/) and
[complementary landmark guidance](https://www.w3.org/WAI/ARIA/apg/patterns/landmarks/examples/complementary.html)
(ENG-243). The divider is a named focusable separator with orientation,
current/minimum/maximum value and controlled panel, following the
[W3C window splitter pattern](https://www.w3.org/WAI/ARIA/apg/patterns/windowsplitter/)
(ENG-261); that APG page notes incomplete review/example work, so citing it does not claim full
conformance. Keyboard and pointer input share bounds, the size menu is the single-pointer
alternative to dragging, and closing help is a dedicated action, not Enter on the separator.

Essential information stays in the workflow: patient and specimen identity, holds, current
gate failures, field constraints, validation errors and the consequences of an irreversible
action. Help supplies explanations and task instructions. This preserves the
[page layout rule for identity and holds](../governance/page-layout-standard.md) and avoids
making a safety decision depend on finding optional documentation.

## Content and loading

Each `docs/help/*.json` file contains one topic using the
[content contract](../help/README.md#topic-contract). Vite imports the files through a literal
glob pattern and bundles them into the help feature, which loads on first opening. Vite
supports both JSON modules and glob imports; glob arguments must be literals, not computed paths
([Vite features](https://vite.dev/guide/features.html#glob-import), ENG-247).

The renderer uses ordinary text, native expandable sections, two-column field/action
reference tables and ordered workflows. It selects the matching page entry using router
ranking; a page selector exposes other references within the topic without navigation.
Search covers detailed fields, actions, restrictions and troubleshooting as well as summaries.
Source-file review metadata is not rendered. The feature adds no
HTML or Markdown rendering, remote content, network search, engine command, runtime
dependency or remote documentation synchronization. Its content does not include record
data. Text updates take effect in a rebuilt application; the shipped app does not watch a
developer's checkout. No process generates or rewrites documentation automatically.

Topic route patterns match the router's patterns, including dynamic parameters and the
optional `/:tab?` suffix. The authoritative route inventory is
[`navigation.ts`](../../ui/src/app/navigation.ts), its setup sections, and
[`detailScreens.ts`](../../ui/src/app/detailScreens.ts), composed by
[`routes.tsx`](../../ui/src/app/routes.tsx). Coverage checks use that inventory rather than a
second manually maintained list. Sign-in, first-run setup, the locked session and the
development-only component gallery are outside the signed-in help scope.

## Maintaining help with development

Each topic is a user manual, not a brief guide. Update the affected topic in the same change
whenever a screen's labels, workflow, status meaning, gate behavior or routes change, following
the [completeness contract](../help/README.md#completeness-contract), the
[authoring procedure](../help/README.md#update-a-topic) and the
[user-manual skill](../../.ai/skills/user-manual/SKILL.md). Review is part of completion even when
no prose changes; `reviewedOn` records a content review, not laboratory approval or validation.

## Verification and requirement trace

| Requirement or contract | Verification |
| --- | --- |
| Available help without screen clutter | Open and close Page help; check collapsed initial state, current-page topic, local search, and preserved working-page input. |
| [NFR-09](../requirements/nonfunctional-requirements.md): interface accessibility target | Component accessibility checks plus keyboard open/read/close, focus return, heading order, and narrow-window/zoom review. Check that help does not obscure focused controls. |
| [NFR-31](../requirements/nonfunctional-requirements.md): local UI and remote-content boundary | Inspect the loader and renderer for bundled text only; build the UI and check that help needs no remote fetch or new Tauri capability. |
| Topic coverage | Tests compare topic routes with all navigation and detail routes, including optional tab patterns; missing, duplicate or obsolete mappings fail. |
| Topic structure | Tests check unique topic/page/entry names, valid review dates, nonempty reference attributes, exact page-route coverage and existing reviewed source files. |
| Behavioral accuracy | Review changed labels, tasks, permissions, gates and status meanings against the affected screen and engine behavior. |

The [quality-gate matrix](../../.ai/skills/quality-gate/SKILL.md#verification-matrix)
and [developer guide](README.md#quality-gate) select the checks for bundled topics, the
renderer and this guide. Passing schema checks alone does not establish correct instructions,
keyboard behavior or native-platform accessibility. This contract does not record a completed
accessibility audit or clinical validation.

Vitest's persistent transformed-module cache is disabled in `vite.config.ts` so that coverage
checks read the current topic files; a cached glob result can omit newly added topics. Vitest
documents stale-cache risks when a transform depends on directory contents
([fsModuleCache](https://main.vitest.dev/config/fsmodulecache), ENG-248).

## Research behind the choice

Primary sources checked 2026-10-14 are registered in the
[engineering source ledger](../research/sources-engineering.md). The consistent top-bar button,
resizable inline layout and its dimensions, JSON shape and route tests are project implementation
choices.

| Source | Supported guidance and application |
| --- | --- |
| ENG-242: AWS Cloudscape, [Help system](https://cloudscape.design/patterns/general/help-system/), published 2020-09-10 | Progressively disclose guidance: immediate labels and constraints, contextual help, then deeper documentation. Default to page-specific content and follow navigation. The project keeps its own components and offline content. |
| ENG-243: W3C, [Disclosure](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/) and [Complementary landmark](https://www.w3.org/WAI/ARIA/apg/patterns/landmarks/examples/complementary.html), living guidance | A button exposes expanded state; a labelled aside gives supporting content a meaningful structure. These sources support semantics, not the exact layout. |
| ENG-244: W3C, [Understanding Consistent Help](https://www.w3.org/WAI/WCAG22/Understanding/consistent-help.html), living guidance | Repeated self-help mechanisms retain their relative order. The criterion distinguishes interface-level contextual help and does not require help on every page; consistent placement here is a design choice. |
| ENG-245: Write the Docs, [Docs as Code](https://www.writethedocs.org/guide/docs-as-code/), publication date unspecified | Version control, review and automated checks can keep documentation in the development workflow. This supports updating topics with their feature changes. |
| ENG-246: Diátaxis, [How-to guides](https://diataxis.fr/how-to-guides/), publication date unspecified | Write task guidance around the user's goal, with practical steps and focused content. The project keeps descriptions of status and behavior separate from steps within each topic. |
| ENG-247: Vite, [JSON and glob imports](https://vite.dev/guide/features.html#glob-import), living documentation | Local JSON imports and literal glob patterns support bundling reviewed content without a new content service or parser dependency. |
| ENG-262: Diátaxis, [Reference](https://diataxis.fr/reference/), publication date unspecified | Reference follows the product structure with consistent factual categories, separate from task sequences. Native disclosures keep named fields/actions scannable while exposing full explanations on demand; the disclosure and table choices are project design decisions. |
