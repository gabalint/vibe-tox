import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { QcAssessmentView } from "@/generated/QcAssessmentView";
import type { QcConflictView } from "@/generated/QcConflictView";
import type { QcObservationView } from "@/generated/QcObservationView";
import type { RunQcView } from "@/generated/RunQcView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { closePanelWithEscape, panel, rowActionLabels } from "../panelTest";
import { CONTROL_LOT, RUN_ID, sessionWith } from "../testData";
import { RunQcPage } from "./RunQcPage";

const ANALYTE_ID = "00000000-0000-7000-8000-000000000b01";

function observation(id: string, overrides: Partial<QcObservationView> = {}): QcObservationView {
  return {
    observationId: id,
    positionId: "position-2",
    positionLabel: "Q01",
    lotId: CONTROL_LOT.lotId,
    sourceTestCode: "AMP",
    analyteId: ANALYTE_ID,
    valueText: "250.0",
    unitsText: "ng/mL",
    flagsText: "N",
    statusText: "F",
    superseded: false,
    recordedAt: "2026-10-12T11:00:00Z",
    ...overrides,
  };
}

const EARLIER: QcAssessmentView = {
  assessmentId: "assessment-1",
  scope: "run_analyte",
  outcome: "accepted",
  supersedesId: null,
  reason: null,
  assessedBy: "quality-1",
  assessedAt: "2026-10-12T11:30:00Z",
  evidenceObservationIds: ["observation-1"],
};

const PREP_BATCH_ID = "00000000-0000-7000-8000-000000000e01";

const CONFLICT: QcConflictView = {
  conflictId: "conflict-1",
  positionId: "position-2",
  conflictsWithId: "observation-1",
  sourceTestCode: "AMP",
  analyteId: ANALYTE_ID,
  valueText: "262.50",
  unitsText: "ng/mL",
  flagsText: "H",
  statusText: "F",
  state: "open",
  observationId: null,
  reason: null,
  resolvedBy: null,
  resolvedAt: null,
  recordedAt: "2026-10-12T11:50:00Z",
  revision: 1,
};

const QC: RunQcView = {
  runId: RUN_ID,
  analytes: [
    {
      analyteId: ANALYTE_ID,
      code: "SYN-AMP",
      observations: [
        observation("observation-0", { valueText: "240.0", superseded: true }),
        observation("observation-1"),
      ],
      assessments: { current: EARLIER, history: [EARLIER] },
    },
  ],
  unmappedObservations: [],
  conflicts: [CONFLICT],
  runAssessments: { current: null, history: [] },
  prepBatches: [{ prepBatchId: PREP_BATCH_ID, assessments: { current: null, history: [] } }],
  deviations: [
    {
      deviationId: "deviation-1",
      scope: "position",
      scopeId: "position-2",
      analyteId: ANALYTE_ID,
      description: "Control vial left open",
      reason: "Seen at the bench",
      state: "open",
      disposition: null,
      closeReason: null,
      openedBy: "quality-1",
      openedAt: "2026-10-12T11:40:00Z",
      closedBy: null,
      closedAt: null,
      revision: 1,
    },
  ],
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  path = `/bench/runs/${RUN_ID}/qc`,
) {
  mockCommands({
    current_session: () => sessionWith(["manage_quality"]),
    get_run_qc: () => QC,
    list_lots: () => [CONTROL_LOT],
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/bench/runs/:runId/qc", element: <RunQcPage /> },
      { path: "/bench/runs/:runId/qc/:tab", element: <RunQcPage /> },
    ],
    path,
  );
}

const tabPath = (tab: string) => `/bench/runs/${RUN_ID}/qc/${tab}`;

function rowOf(cell: string): HTMLElement {
  const row = screen.getByRole("gridcell", { name: cell }).closest<HTMLElement>("[role=row]");
  if (!row) throw new Error(`no row for ${cell}`);
  return row;
}

describe("RunQcPage (REQ-ANA-004, REQ-ANA-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows each scope's assessment on the first tab, at the run's QC address", async () => {
    const { container } = renderPage();
    const grid = await screen.findByRole("grid", { name: "QC assessments" });
    expect(screen.getByRole("tab", { name: "Assessments" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    // Open items that need a decision are counted on their tabs.
    expect(screen.getByRole("tab", { name: "Changed QC rows (1 open)" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Deviations (1 open)" })).toBeTruthy();
    const amphetamine = rowOf("SYN-AMP");
    expect(within(amphetamine).getByText("Accepted")).toBeTruthy();
    // One current observation; the replaced one is not cited.
    expect(within(amphetamine).getByRole("gridcell", { name: "1" })).toBeTruthy();
    expect(within(rowOf("Whole run")).getByText("Not assessed")).toBeTruthy();
    expect(
      within(rowOf(`Preparation batch ${PREP_BATCH_ID}`)).getByText("Not assessed"),
    ).toBeTruthy();
    expect(within(grid).getAllByRole("row")).toHaveLength(4);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows every observation in one grid exactly as received", async () => {
    const unmapped = observation("observation-9", {
      analyteId: null,
      sourceTestCode: "XYZ",
      valueText: "1.5",
      unitsText: null,
    });
    const { container } = renderPage(
      { get_run_qc: () => ({ ...QC, unmappedObservations: [unmapped] }) },
      tabPath("observations"),
    );
    const grid = await screen.findByRole("grid", { name: "QC observations" });
    expect(screen.getAllByRole("grid")).toHaveLength(1);
    expect(within(grid).getByRole("gridcell", { name: "250.0\u00A0ng/mL" })).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "240.0\u00A0ng/mL" })).toBeTruthy();
    expect(
      within(grid).getByRole("gridcell", { name: "1.5\u00A0(no units reported)" }),
    ).toBeTruthy();
    expect(within(grid).getAllByRole("gridcell", { name: "SYN-AMP" })).toHaveLength(2);
    expect(within(grid).getByRole("gridcell", { name: "Not mapped" })).toBeTruthy();
    expect(within(grid).getByText("Replaced")).toBeTruthy();
    expect(
      await within(grid).findAllByRole("gridcell", { name: "Synthetic control, lot CTL-001" }),
    ).toHaveLength(3);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens a scope by click or Enter, steps to the next, and Escape closes it", async () => {
    const { user, router } = renderPage();
    await openRow(user, "Whole run");
    expect(router.state.location.search).toBe("?open=run");
    expect(within(panel("Whole run")).getByText("Not assessed yet.")).toBeTruthy();
    await user.click(within(panel("Whole run")).getByRole("button", { name: "Next" }));
    const batch = `Preparation batch ${PREP_BATCH_ID}`;
    expect(await screen.findByRole("region", { name: batch })).toBeTruthy();
    await closePanelWithEscape(user, batch, batch);
    await openRowWithEnter(user, "SYN-AMP");
    expect(within(panel("QC for SYN-AMP")).getByText(/citing 1 observation/)).toBeTruthy();
  });

  it("opens the scope a link names", async () => {
    renderPage({}, `/bench/runs/${RUN_ID}/qc?open=${ANALYTE_ID}`);
    expect(await screen.findByRole("region", { name: "QC for SYN-AMP" })).toBeTruthy();
  });

  it("replaces an assessment only with a reason, citing exactly the current observations", async () => {
    const assess = vi.fn(() => ({ ...EARLIER, assessmentId: "assessment-2", outcome: "rejected" }));
    const { user } = renderPage({ assess_qc: assess });
    await screen.findByRole("grid", { name: "QC assessments" });
    expect(await rowActionLabels(user, "SYN-AMP")).toEqual(["Assess QC"]);
    await openRow(user, "SYN-AMP");
    await user.click(within(panel("QC for SYN-AMP")).getByRole("button", { name: "Assess QC" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Cites the 1 current observation shown.");
    expect(dialog.textContent).toContain("Replaces the current assessment (accepted).");
    await chooseWithKeyboard(user, /Decision$/, "Reject");
    await user.click(within(dialog).getByRole("button", { name: "Record assessment" }));
    expect(await within(dialog).findByText("Enter reason.")).toBeTruthy();
    expect(assess).not.toHaveBeenCalled();
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Control out of range",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record assessment" }));
    expect(await screen.findByText("QC assessment recorded")).toBeTruthy();
    expect(assess).toHaveBeenCalledWith({
      input: {
        scope: "run_analyte",
        runId: RUN_ID,
        analyteId: ANALYTE_ID,
        outcome: "rejected",
        evidenceObservationIds: ["observation-1"],
        supersedesAssessmentId: "assessment-1",
        reason: "Control out of range",
      },
    });
  });

  it("shows the engine's refusal to accept over a rejection", async () => {
    const { user } = renderPage({
      assess_qc: () =>
        Promise.reject(engineError("conflict", "Rejected QC cannot be replaced by an acceptance.")),
    });
    await screen.findByRole("grid", { name: "QC assessments" });
    const menu = await openRowActions(user, "SYN-AMP");
    await user.click(within(menu).getByRole("menuitem", { name: "Assess QC" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Decision$/, "Accept");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Rerun accepted");
    await user.click(within(dialog).getByRole("button", { name: "Record assessment" }));
    expect(
      await within(dialog).findByText("Rejected QC cannot be replaced by an acceptance."),
    ).toBeTruthy();
  });

  it("assesses a preparation batch, and names the gate when it is not completed", async () => {
    const assess = vi.fn(() =>
      Promise.reject(
        engineError("gate_blocked", "This action is blocked.", {
          gateReasons: [
            {
              code: "prep_not_completed",
              message: "Complete the preparation batch before accepting it.",
            },
          ],
        }),
      ),
    );
    const { user } = renderPage({ assess_qc: assess });
    const title = `Preparation batch ${PREP_BATCH_ID}`;
    await openRow(user, title);
    await user.click(within(panel(title)).getByRole("button", { name: "Assess QC" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Only a completed batch can be accepted.");
    await chooseWithKeyboard(user, /Decision$/, "Accept");
    await user.click(within(dialog).getByRole("button", { name: "Record assessment" }));
    const gates = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain(
      "Preparation batch not completed: Complete the preparation batch before accepting it.",
    );
    expect(assess).toHaveBeenCalledWith({
      input: {
        scope: "prep_batch",
        prepBatchId: PREP_BATCH_ID,
        outcome: "accepted",
        evidenceObservationIds: [],
      },
    });
  });

  it("opens a deviation against a lot, and closes an open one from its panel", async () => {
    const open = vi.fn(() => QC.deviations[0]);
    const close = vi.fn(() => QC.deviations[0]);
    const { user, container } = renderPage(
      { open_deviation: open, close_deviation: close },
      tabPath("deviations"),
    );
    expect(await screen.findByRole("gridcell", { name: "Position Q01" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Open deviation" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Applies to$/, "One lot");
    await chooseWithKeyboard(user, /Lot$/, "Synthetic control, lot CTL-001");
    await user.type(within(dialog).getByRole("textbox", { name: /Description/ }), "Lot drift");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Trend over 3 runs");
    await user.click(within(dialog).getByRole("button", { name: "Open deviation" }));
    expect(await screen.findByText("Deviation opened")).toBeTruthy();
    expect(open).toHaveBeenCalledWith({
      input: {
        scope: { kind: "lot", id: CONTROL_LOT.lotId },
        description: "Lot drift",
        reason: "Trend over 3 runs",
      },
    });

    const description = "Control vial left open";
    expect(await rowActionLabels(user, description)).toEqual(["Close deviation"]);
    await openRow(user, description);
    expect(within(panel(description)).getByText("SYN-AMP")).toBeTruthy();
    await user.click(within(panel(description)).getByRole("button", { name: "Close deviation" }));
    const closing = await screen.findByRole("dialog", { name: "Close deviation" });
    await user.type(
      within(closing).getByRole("textbox", { name: /Disposition/ }),
      "Vial discarded",
    );
    await user.type(within(closing).getByRole("textbox", { name: /Reason/ }), "Replaced the vial");
    await user.click(within(closing).getByRole("button", { name: "Close deviation" }));
    expect(await screen.findByText("Deviation closed")).toBeTruthy();
    expect(close).toHaveBeenCalledWith({
      input: {
        deviationId: "deviation-1",
        expectedRevision: 1,
        disposition: "Vial discarded",
        reason: "Replaced the vial",
      },
    });
    await closePanelWithEscape(user, description, description);
  });

  it("shows a changed QC row exactly as received, and accepts it only with a reason", async () => {
    const resolve = vi.fn(() => ({ ...CONFLICT, state: "accepted", revision: 2 }));
    const { user, container } = renderPage(
      { resolve_qc_conflict: resolve },
      tabPath("changed-rows"),
    );
    const grid = await screen.findByRole("grid", { name: "Changed QC rows" });
    expect(within(grid).getByRole("gridcell", { name: "262.50\u00A0ng/mL" })).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "250.0\u00A0ng/mL at Q01" })).toBeTruthy();
    expect(within(grid).getByText("Awaits a decision")).toBeTruthy();
    expect(screen.getByText(/Changed QC row awaits a decision/)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    const name = "SYN-AMP, 262.50\u00A0ng/mL";
    expect(await rowActionLabels(user, name)).toEqual([
      "Accept as correction",
      "Reject changed row",
    ]);
    await openRowWithEnter(user, "262.50\u00A0ng/mL");
    expect(within(panel(name)).getByText("H")).toBeTruthy();
    await user.click(within(panel(name)).getByRole("button", { name: "Accept as correction" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Accept the changed QC row?" });
    expect(dialog.textContent).toContain("262.50\u00A0ng/mL replaces the stored observation.");
    await user.click(within(dialog).getByRole("button", { name: "Accept as correction" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(resolve).not.toHaveBeenCalled();
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Analyzer recalculated",
    );
    await user.click(within(dialog).getByRole("button", { name: "Accept as correction" }));
    expect(await screen.findByText("Changed QC row accepted")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        conflictId: "conflict-1",
        expectedRevision: 1,
        resolution: "accept_as_correction",
        reason: "Analyzer recalculated",
      },
    });
  });

  it("rejects a changed QC row from its row menu, and offers no decision on a decided one", async () => {
    const resolve = vi.fn(() => Promise.reject(engineError("conflict", "Reload and try again.")));
    const decided: QcConflictView = {
      ...CONFLICT,
      conflictId: "conflict-0",
      valueText: "255.00",
      state: "rejected",
      reason: "Transmission repeat",
      resolvedBy: "quality-1",
      resolvedAt: "2026-10-12T11:55:00Z",
    };
    const { user } = renderPage(
      {
        get_run_qc: () => ({ ...QC, conflicts: [CONFLICT, decided] }),
        resolve_qc_conflict: resolve,
      },
      tabPath("changed-rows"),
    );
    await openRow(user, "255.00\u00A0ng/mL");
    const decidedPanel = panel("SYN-AMP, 255.00\u00A0ng/mL");
    expect(within(decidedPanel).getByText("Transmission repeat")).toBeTruthy();
    expect(within(decidedPanel).queryByRole("button", { name: "Reject changed row" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Actions for SYN-AMP, 255.00\u00A0ng/mL" }),
    ).toBeNull();

    const menu = await openRowActions(user, "SYN-AMP, 262.50\u00A0ng/mL");
    await user.click(within(menu).getByRole("menuitem", { name: "Reject changed row" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Reject the changed QC row?" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Not a correction");
    await user.click(within(dialog).getByRole("button", { name: "Reject changed row" }));
    expect(await within(dialog).findByText("Reload and try again.")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        conflictId: "conflict-1",
        expectedRevision: 1,
        resolution: "reject",
        reason: "Not a correction",
      },
    });
  });

  it("says when no QC, changed row or deviation has arrived", async () => {
    const empty = { ...QC, analytes: [], conflicts: [], prepBatches: [], deviations: [] };
    const { user } = renderPage({ get_run_qc: () => empty }, tabPath("observations"));
    expect(await screen.findByText("No QC observations yet")).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Changed QC rows" }));
    expect(await screen.findByText("No changed QC rows")).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Deviations" }));
    expect(await screen.findByText("No deviations recorded")).toBeTruthy();
    // The whole run can still be assessed.
    await user.click(screen.getByRole("tab", { name: "Assessments" }));
    expect(await screen.findByRole("gridcell", { name: "Whole run" })).toBeTruthy();
  });

  it("shows loading while the engine answers", async () => {
    renderPage({ get_run_qc: () => new Promise(() => undefined) });
    expect(await screen.findByText("Loading run QC…")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Run QC review" })).toBeTruthy();
  });

  it("shows the engine's error with the support reference, and no retry when forbidden", async () => {
    renderPage({
      get_run_qc: () => Promise.reject(engineError("forbidden", "QC review needs quality rights.")),
    });
    expect(await screen.findByText("QC review needs quality rights.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });
});
