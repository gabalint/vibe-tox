import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { WorklistEntry } from "@/generated/WorklistEntry";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { CONTROL_LOT, INSTRUMENT, METHOD_VERSION_ID, RUN, RUN_ID, sessionWith } from "../testData";
import { RunsPage } from "./RunsPage";
import { WorklistPage } from "./WorklistPage";

const METHOD = {
  id: "00000000-0000-7000-8000-000000000c01",
  code: "SYN-IA",
  name: "Synthetic immunoassay",
  versionInEffect: 1,
  draftVersion: null,
};

const VERSIONS: CatalogVersionSummary[] = [
  {
    id: METHOD_VERSION_ID,
    version: 1,
    status: "approved",
    revision: 2,
    createdBy: "author-1",
    createdAt: "2026-10-12T09:00:00Z",
    approvedBy: "approver-1",
    effectiveFrom: "2026-10-12T10:00:00Z",
    effectiveTo: null,
    replacedBy: null,
    retiredBy: null,
    retiredAt: null,
    retireReason: null,
  },
  {
    id: "00000000-0000-7000-8000-000000000d02",
    version: 2,
    status: "draft",
    revision: 1,
    createdBy: "author-1",
    createdAt: "2026-10-12T11:00:00Z",
    approvedBy: null,
    effectiveFrom: null,
    effectiveTo: null,
    replacedBy: null,
    retiredBy: null,
    retiredAt: null,
    retireReason: null,
  },
];

const ENTRY: WorklistEntry = {
  containerId: "container-1",
  labelCode: "C00000001",
  accessionNumber: "A26000001",
  specimenTypeCode: "urine",
  componentIds: ["component-1"],
  orderables: ["SYN-SCR"],
  receivedAt: "2026-10-12T09:30:00Z",
};

function renderPage(path: string, commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["perform_analysis"]),
    list_methods: () => [METHOD],
    list_method_versions: () => VERSIONS,
    list_worklist: () => [ENTRY],
    list_instrument_readiness: () => [INSTRUMENT],
    list_lots: () => [CONTROL_LOT],
    list_prep_batches: () => [],
    list_runs: () => [
      {
        runId: RUN_ID,
        instrumentId: INSTRUMENT.instrumentId,
        methodVersionId: METHOD_VERSION_ID,
        state: "open",
        createdAt: "2026-10-12T10:00:00Z",
        positionCount: 2,
      },
    ],
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/bench/worklist", element: <WorklistPage /> },
      { path: "/bench/runs", element: <RunsPage /> },
    ],
    path,
  );
}

describe("WorklistPage (REQ-ANA-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists waiting containers for an approved method version and creates a run", async () => {
    const create = vi.fn(() => RUN);
    const worklist = vi.fn(() => [ENTRY]);
    const { user, container, router } = renderPage("/bench/worklist", {
      create_run: create,
      list_worklist: worklist,
    });
    await screen.findByRole("button", { name: /Method$/ });
    expect(worklist).not.toHaveBeenCalled();
    await chooseWithKeyboard(user, /Method$/, "Synthetic immunoassay (SYN-IA)");
    await user.click(await screen.findByRole("button", { name: /Version/ }));
    const listbox = await screen.findByRole("listbox");
    // Drafts cannot be run.
    expect(within(listbox).queryByRole("option", { name: "Version 2" })).toBeNull();
    await user.click(within(listbox).getByRole("option", { name: "Version 1 — current" }));
    expect(await screen.findByRole("gridcell", { name: "C00000001" })).toBeTruthy();
    expect(worklist).toHaveBeenCalledWith({ input: { methodVersionId: METHOD_VERSION_ID } });
    expect(router.state.location.search).toContain(`version=${METHOD_VERSION_ID}`);
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Create run" }));
    const dialog = await screen.findByRole("dialog", { name: "Create run" });
    await chooseWithKeyboard(user, /Instrument$/, "Synthetic analyzer 1: in service");
    await user.click(
      within(dialog).getByRole("checkbox", { name: /Synthetic control, lot CTL-001/ }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Create run" }));
    expect(await screen.findByText(`Navigated to /bench/runs/${RUN_ID}`)).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: {
        instrumentId: INSTRUMENT.instrumentId,
        methodVersionId: METHOD_VERSION_ID,
        lotIds: [CONTROL_LOT.lotId],
      },
    });
  });

  it("lists why the engine refuses a run", async () => {
    const { user } = renderPage(
      `/bench/worklist?method=${METHOD.id}&version=${METHOD_VERSION_ID}`,
      {
        create_run: () =>
          Promise.reject(
            engineError("gate_blocked", "The run cannot be created.", {
              gateReasons: [
                { code: "instrument_out_of_service", message: "The instrument is out of service." },
              ],
            }),
          ),
      },
    );
    await user.click(await screen.findByRole("button", { name: "Create run" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Instrument$/, "Synthetic analyzer 1: in service");
    await user.click(within(dialog).getByRole("button", { name: "Create run" }));
    const gates = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain("The instrument is out of service.");
  });

  it("keeps the waiting containers as the one growing grid, two identifiers per row", async () => {
    renderPage(`/bench/worklist?method=${METHOD.id}&version=${METHOD_VERSION_ID}`);
    const grid = await screen.findByRole("grid", { name: "Waiting containers" });
    expect(grid.hasAttribute("data-fill")).toBe(true);
    expect(within(grid).getByRole("gridcell", { name: "C00000001" })).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "A26000001" })).toBeTruthy();
  });

  it("says when nothing is waiting", async () => {
    renderPage(`/bench/worklist?method=${METHOD.id}&version=${METHOD_VERSION_ID}`, {
      list_worklist: () => [],
    });
    expect(await screen.findByText("No containers waiting")).toBeTruthy();
  });
});

describe("RunsPage (REQ-ANA-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists open runs with their instrument and opens one with Enter", async () => {
    const { user, container } = renderPage("/bench/runs");
    await screen.findByRole("gridcell", { name: "Synthetic analyzer 1" });
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "2026-10-12 10:00 UTC");
    expect(await screen.findByText(`Navigated to /bench/runs/${RUN_ID}`)).toBeTruthy();
  });

  it("opens a run with a single click, passing the list's address and order", async () => {
    const { user, router } = renderPage("/bench/runs?state=all");
    await openRow(user, "Synthetic analyzer 1");
    expect(await screen.findByText(`Navigated to /bench/runs/${RUN_ID}`)).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: { list: { label: "Runs", href: "/bench/runs?state=all" }, ids: [RUN_ID] },
    });
  });

  it("shows empty and error states", async () => {
    renderPage("/bench/runs?state=closed", { list_runs: () => [] });
    expect(await screen.findByText("No runs")).toBeTruthy();
  });

  it("shows the engine's refusal without a retry", async () => {
    renderPage("/bench/runs", {
      list_runs: () => Promise.reject(engineError("forbidden", "You cannot list runs.")),
    });
    expect(await screen.findByText("You cannot list runs.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });
});
