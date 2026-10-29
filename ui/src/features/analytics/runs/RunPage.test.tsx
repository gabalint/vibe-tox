import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RunDetail } from "@/generated/RunDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { closePanelWithEscape, panel, rowActionLabels } from "../panelTest";
import { CONTROL_LOT, INSTRUMENT, RUN, RUN_ID, sessionWith } from "../testData";
import { RunPage } from "./RunPage";
import { RunsPage } from "./RunsPage";

function renderPage(
  run: RunDetail | (() => unknown),
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["perform_analysis"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_run: typeof run === "function" ? run : () => run,
    list_lots: () => [CONTROL_LOT],
    list_prep_batches: () => [],
    list_laboratory_profiles: () => [],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/bench/runs/:runId", element: <RunPage /> }],
    `/bench/runs/${RUN_ID}`,
  );
}

describe("RunPage (REQ-ANA-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the run, its lots and positions with their attempt kinds", async () => {
    const { container } = renderPage(RUN);
    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "Run of SYN-IA version 1 on Synthetic analyzer 1",
      }),
    ).toBeTruthy();
    expect(screen.getByText(/Synthetic control, lot CTL-001/)).toBeTruthy();
    const grid = screen.getByRole("grid", { name: "Positions" });
    expect(within(grid).getByRole("gridcell", { name: "C00000001" })).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "Initial" })).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "Control" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("keeps the run facts short, with the scan right above the positions", async () => {
    renderPage(RUN);
    const title = await screen.findByRole("heading", {
      level: 1,
      name: "Run of SYN-IA version 1 on Synthetic analyzer 1",
    });
    // The state sits beside the title; an open run has no "Closed" fact.
    expect(within(title.closest("header") ?? document.body).getByText("Open")).toBeTruthy();
    expect(screen.queryByText("Not closed")).toBeNull();
    // The scan is part of the positions, not a section of its own above them.
    expect(screen.queryByRole("region", { name: "Add patient sample" })).toBeNull();
    const scan = screen.getByRole("form", { name: "Add patient sample" });
    const grid = screen.getByRole("grid", { name: "Positions" });
    expect(scan.parentElement?.contains(grid)).toBe(true);
  });

  it("adds a patient sample by keyboard: Tab to the position, then scan and Enter", async () => {
    const add = vi.fn(() => RUN);
    const { user } = renderPage(RUN, { add_patient_position: add });
    const position = await screen.findByRole("textbox", { name: /^Position/ });
    await tabTo(user, position);
    await user.keyboard("P02");
    await user.tab();
    // A scanner types the label and presses Enter.
    await user.keyboard("C00000002{Enter}");
    expect(await screen.findByText("C00000002 placed at P02")).toBeTruthy();
    expect(add).toHaveBeenCalledWith({
      input: {
        runId: RUN_ID,
        positionLabel: "P02",
        containerLabel: "C00000002",
        attemptKind: "initial",
      },
    });
    // Ready for the next sample.
    expect(position).toHaveProperty("value", "");
    expect(screen.getByRole("textbox", { name: /^Container barcode/ })).toHaveProperty("value", "");
    expect(document.activeElement).toBe(position);
  });

  it("lists the gates that block a sample and keeps what was typed", async () => {
    const { user } = renderPage(RUN, {
      add_patient_position: () =>
        Promise.reject(
          engineError("gate_blocked", "This sample cannot be tested yet.", {
            gateReasons: [
              { code: "specimen_held", message: "The specimen is on hold." },
              { code: "order_not_accepted", message: "The order has not been accepted." },
            ],
          }),
        ),
    });
    await user.type(await screen.findByRole("textbox", { name: /^Position/ }), "P02");
    await user.type(
      screen.getByRole("textbox", { name: /^Container barcode/ }),
      "C00000009{Enter}",
    );
    const gates = await screen.findByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain("The specimen is on hold.");
    expect(gates.textContent).toContain("The order has not been accepted.");
    expect(screen.getByRole("textbox", { name: /^Container barcode/ })).toHaveProperty(
      "value",
      "C00000009",
    );
  });

  it("requires a position and a barcode before scanning", async () => {
    const add = vi.fn();
    const { user } = renderPage(RUN, { add_patient_position: add });
    await user.type(await screen.findByRole("textbox", { name: /^Container barcode/ }), "{Enter}");
    expect(await screen.findByText("Enter position.")).toBeTruthy();
    expect(screen.getByText("Enter container barcode.")).toBeTruthy();
    expect(add).not.toHaveBeenCalled();
  });

  it("adds a control position with its lot", async () => {
    const add = vi.fn(() => RUN);
    const { user } = renderPage(RUN, { add_qc_position: add });
    await user.click(
      await screen.findByRole("button", { name: "Add control, calibrator or blank" }),
    );
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: /^Position/ }), "Q02");
    await chooseWithKeyboard(user, /Role$/, "Control");
    await chooseWithKeyboard(
      user,
      /Control lot$/,
      "Synthetic control, lot CTL-001, Level 1 (QC-L1)",
    );
    await user.click(within(dialog).getByRole("button", { name: "Add position" }));
    expect(await screen.findByText("Position added")).toBeTruthy();
    expect(add).toHaveBeenCalledWith({
      input: { runId: RUN_ID, positionLabel: "Q02", role: "control", lotId: CONTROL_LOT.lotId },
    });
  });

  it("opens a position in its panel, and Escape closes it with focus back on the row", async () => {
    const { user, router } = renderPage(RUN);
    await openRow(user, "Q01");
    expect(router.state.location.search).toBe("?open=position-2");
    const position = panel("Position Q01");
    expect(within(position).getByText("Synthetic control, lot CTL-001")).toBeTruthy();
    // A control position has no patient attempt to fail.
    expect(within(position).queryByRole("button", { name: "Mark attempt failed" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for Q01" })).toBeNull();
    await closePanelWithEscape(user, "Position Q01", "Q01");
  });

  it("marks an attempt failed with a reason from its panel, as its row menu offers", async () => {
    const fail = vi.fn(() => RUN);
    const { user } = renderPage(RUN, { fail_execution: fail });
    await screen.findByRole("grid", { name: "Positions" });
    expect(await rowActionLabels(user, "P01")).toEqual(["Mark attempt failed"]);
    await openRowWithEnter(user, "P01");
    const position = panel("Position P01");
    expect(within(position).getByText("Initial")).toBeTruthy();
    await user.click(within(position).getByRole("button", { name: "Mark attempt failed" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Injection error");
    await user.click(within(confirm).getByRole("button", { name: "Mark failed" }));
    expect(await screen.findByText("Attempt at P01 marked failed")).toBeTruthy();
    expect(fail).toHaveBeenCalledWith({
      input: { executionId: "execution-1", expectedRevision: 1, reason: "Injection error" },
    });
  });

  it("offers no failure the engine does not allow", async () => {
    const [patient, control] = RUN.positions;
    if (!patient?.execution || !control) throw new Error("fixture");
    const failed = { ...patient.execution, state: "failed" as const, allowedActions: [] };
    renderPage({ ...RUN, positions: [{ ...patient, execution: failed }, control] });
    await screen.findByRole("grid", { name: "Positions" });
    expect(screen.queryByRole("button", { name: "Actions for P01" })).toBeNull();
  });

  it("keeps the filtered list in the breadcrumb, with Previous and Next in its order", async () => {
    const second: RunDetail = { ...RUN, runId: "run-2", methodVersion: 2 };
    const summary = (run: RunDetail) => ({
      runId: run.runId,
      instrumentId: run.instrumentId,
      methodVersionId: run.methodVersionId,
      state: run.state,
      createdAt: run.createdAt,
      positionCount: run.positions.length,
    });
    mockCommands({
      current_session: () => sessionWith(["perform_analysis"]),
      list_runs: () => [summary(RUN), summary(second)],
      list_instrument_readiness: () => [INSTRUMENT],
      get_run: (args) => (JSON.stringify(args).includes("run-2") ? second : RUN),
      list_lots: () => [CONTROL_LOT],
      list_prep_batches: () => [],
      list_laboratory_profiles: () => [],
    });
    const { user } = renderRoutes(
      [
        { path: "/bench/runs", element: <RunsPage /> },
        { path: "/bench/runs/:runId", element: <RunPage /> },
      ],
      "/bench/runs?state=all",
    );
    const [first] = await screen.findAllByRole("gridcell", { name: INSTRUMENT.name });
    if (!first) throw new Error("no run");
    await user.click(first);
    expect(
      await screen.findByRole("heading", { level: 1, name: /SYN-IA version 1 on/ }),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "Runs" }).getAttribute("href")).toBe(
      "/bench/runs?state=all",
    );
    await user.click(screen.getByRole("link", { name: "Next" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: /SYN-IA version 2 on/ }),
    ).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "Previous" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: /SYN-IA version 1 on/ }),
    ).toBeTruthy();
  });

  it("keeps Runs in the breadcrumb when opened from elsewhere", async () => {
    renderPage(RUN);
    await screen.findByRole("grid", { name: "Positions" });
    expect(screen.getByRole("link", { name: "Runs" }).getAttribute("href")).toBe("/bench/runs");
    // A run is about many specimens: no patient banner.
    expect(screen.queryByRole("region", { name: /identity/ })).toBeNull();
    expect(screen.queryByRole("link", { name: "Next" })).toBeNull();
  });

  it("closes the run with a reason after saying what closing means", async () => {
    const close = vi.fn(() => ({ ...RUN, state: "closed", allowedActions: [] }));
    const { user } = renderPage(RUN, { close_run: close });
    await user.click(await screen.findByRole("button", { name: "Close run" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Close run" });
    expect(confirm.textContent).toContain("No more positions can be added");
    await user.click(within(confirm).getByRole("button", { name: "Close run" }));
    expect(await within(confirm).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(close).not.toHaveBeenCalled();
    await user.type(
      within(confirm).getByRole("textbox", { name: /Reason/ }),
      "  All results received and reconciled ",
    );
    await user.click(within(confirm).getByRole("button", { name: "Close run" }));
    expect(await screen.findByText("Run closed")).toBeTruthy();
    expect(close).toHaveBeenCalledWith({
      input: {
        runId: RUN_ID,
        expectedRevision: 3,
        reason: "All results received and reconciled",
      },
    });
  });

  it("shows the reason beside the closing time", async () => {
    renderPage({
      ...RUN,
      state: "closed",
      closedAt: "2026-10-12T12:00:00Z",
      closeReason: "All results received and reconciled",
      allowedActions: [],
    });
    expect(
      await screen.findByText("2026-10-12 12:00 UTC: All results received and reconciled"),
    ).toBeTruthy();
  });

  it("offers no entry on a closed run, and QC review to quality staff", async () => {
    renderPage(
      { ...RUN, state: "closed", closedAt: "2026-10-12T12:00:00Z", allowedActions: [] },
      {},
      ["perform_analysis", "manage_quality"],
    );
    expect(await screen.findByText("2026-10-12 12:00 UTC")).toBeTruthy();
    expect(screen.queryByRole("form", { name: "Add patient sample" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Close run" })).toBeNull();
    expect(await screen.findByRole("button", { name: "QC review" })).toBeTruthy();
  });

  it("shows loading while the engine answers", async () => {
    renderPage(() => new Promise(() => undefined));
    expect(await screen.findByText("Loading run…")).toBeTruthy();
  });

  it("shows a run that cannot be loaded with the support reference", async () => {
    renderPage(() => Promise.reject(engineError("not_found", "The run was not found.")));
    expect(await screen.findByText("The run was not found.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Run" })).toBeTruthy();
  });
});
