import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { closePanelWithEscape, panel, rowActionLabels } from "../panelTest";
import { INSTRUMENT, sessionWith } from "../testData";
import { ImportAttemptPage } from "./ImportAttemptPage";
import { ImportAttemptsPage } from "./ImportAttemptsPage";

const ATTEMPT_ID = "00000000-0000-7000-8000-000000008001";

const DETAIL: ImportAttemptDetail = {
  importAttemptId: ATTEMPT_ID,
  receiptId: "00000000-0000-7000-8000-000000009001",
  attemptNo: 1,
  instrumentId: INSTRUMENT.instrumentId,
  outcome: "processed_with_quarantine",
  failureCode: null,
  rowsTotal: 5,
  rowsNew: 2,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 1,
  rowsQuarantined: 2,
  rowsNotResult: 0,
  rowsSetAside: 0,
  rowsRetained: 0,
  processedAt: "2026-10-12T11:00:00Z",
  registrationId: null,
  file: null,
  setAside: [],
  quarantine: [
    {
      itemId: "item-1",
      recordPath: "R4",
      sampleLabel: "C00000001",
      testCode: "AMP",
      valueText: "530.0",
      unitsText: "ng/mL",
      reason: "conflicting_duplicate",
      state: "open",
      revision: 1,
    },
    {
      itemId: "item-2",
      recordPath: "R5",
      sampleLabel: "C99999999",
      testCode: "AMP",
      valueText: "<b>12</b>",
      unitsText: null,
      reason: "unknown_sample",
      state: "rejected",
      revision: 2,
    },
  ],
};

function renderPage(path: string, commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_instrument_readiness: () => [INSTRUMENT],
    list_import_attempts: () => [DETAIL],
    get_import_attempt: () => DETAIL,
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/review/imports", element: <ImportAttemptsPage /> },
      { path: "/review/imports/:importAttemptId", element: <ImportAttemptPage /> },
    ],
    path,
  );
}

const SECOND: ImportAttemptDetail = {
  ...DETAIL,
  importAttemptId: "00000000-0000-7000-8000-000000008002",
  attemptNo: 2,
  outcome: "processed",
  rowsQuarantined: 0,
  processedAt: "2026-10-12T12:00:00Z",
  quarantine: [],
};

describe("Result imports (REQ-ANA-003)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists import attempts with their counts and opens one with Enter", async () => {
    const list = vi.fn(() => [DETAIL]);
    const { user, container } = renderPage("/review/imports", { list_import_attempts: list });
    await screen.findByRole("gridcell", { name: "Synthetic analyzer 1" });
    expect(screen.getByText("Processed, some rows quarantined")).toBeTruthy();
    // One more than shown, so the list knows whether more exist.
    expect(list).toHaveBeenCalledWith({
      input: { instrumentId: null, limit: 200, before: null, afterImportAttemptId: undefined },
    });
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "2026-10-12 11:00 UTC");
    expect(await screen.findByRole("heading", { level: 1, name: "Import attempt 1" })).toBeTruthy();
  });

  it("opens an attempt with a click, with the list in its breadcrumb and Next in its order", async () => {
    const { user } = renderPage("/review/imports", {
      list_import_attempts: () => [DETAIL, SECOND],
      get_import_attempt: (args) =>
        JSON.stringify(args).includes(SECOND.importAttemptId) ? SECOND : DETAIL,
    });
    await openRow(user, "2026-10-12 11:00 UTC");
    expect(await screen.findByRole("heading", { level: 1, name: "Import attempt 1" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Result imports" }).getAttribute("href")).toBe(
      "/review/imports",
    );
    expect(screen.getByRole("link", { name: "Previous" }).getAttribute("aria-disabled")).toBe(
      "true",
    );
    await user.click(screen.getByRole("link", { name: "Next" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Import attempt 2" })).toBeTruthy();
    expect(screen.getByText("No rows were quarantined.")).toBeTruthy();
  });

  it("says the whole message was held when it was refused, not that no rows were", async () => {
    const refused: ImportAttemptDetail = {
      ...DETAIL,
      outcome: "quarantined",
      failureCode: "unregistered_source",
      rowsTotal: 3,
      rowsNew: 0,
      rowsQc: 0,
      rowsQuarantined: 3,
      quarantine: [],
    };
    renderPage(`/review/imports/${ATTEMPT_ID}`, { get_import_attempt: () => refused });
    expect(
      await screen.findByText("The whole message was held: 3 rows. See Message failure above."),
    ).toBeTruthy();
    expect(screen.queryByText("No rows were quarantined.")).toBeNull();
  });

  it("says when more exist than are shown", async () => {
    const many = Array.from({ length: 201 }, (_, index) => ({
      ...DETAIL,
      importAttemptId: index === 0 ? ATTEMPT_ID : `attempt-${index}`,
    }));
    const list = vi.fn((args: unknown) =>
      JSON.stringify(args).includes("attempt-199") ? many.slice(200) : many,
    );
    const { user } = renderPage("/review/imports", { list_import_attempts: list });
    const grid = await screen.findByRole("grid", { name: "Imports" });
    expect(grid.getAttribute("aria-rowcount")).toBe("201");
    expect(screen.getByText("200 imports shown · more available")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load older imports" }));
    await vi.waitFor(() => expect(grid.getAttribute("aria-rowcount")).toBe("202"));
    expect(list).toHaveBeenLastCalledWith({
      input: { instrumentId: null, limit: 200, before: null, afterImportAttemptId: "attempt-199" },
    });
    expect(screen.queryByRole("button", { name: "Load older imports" })).toBeNull();
  });

  it("shows quarantined rows as plain text with why they were held", async () => {
    const { user, container } = renderPage(`/review/imports/${ATTEMPT_ID}`);
    const grid = await screen.findByRole("grid", { name: "Quarantined rows" });
    expect(
      within(grid).getByRole("gridcell", { name: "<b>12</b>\u00A0(no units reported)" }),
    ).toBeTruthy();
    expect(within(grid).getByText("A different value arrived for the same row")).toBeTruthy();
    expect(within(grid).getByText("Needs a decision")).toBeTruthy();
    // Without a list to return to, the breadcrumb leads to Result imports.
    expect(screen.getByRole("link", { name: "Result imports" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Next" })).toBeNull();
    await openRow(user, "R5");
    expect(within(panel("Row R5")).getByText("<b>12</b> (no units reported)")).toBeTruthy();
    expect(container.querySelector("b")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("accepts an open row as reprocessing from its panel, as its row menu offers", async () => {
    const resolve = vi.fn(() => DETAIL);
    const { user } = renderPage(`/review/imports/${ATTEMPT_ID}`, {
      resolve_quarantine_item: resolve,
    });
    await screen.findByRole("grid", { name: "Quarantined rows" });
    expect(await rowActionLabels(user, "R4")).toEqual(["Accept as reprocessing", "Reject row"]);
    await openRowWithEnter(user, "R4");
    await user.click(
      within(panel("Row R4")).getByRole("button", { name: "Accept as reprocessing" }),
    );
    const confirm = await screen.findByRole("alertdialog");
    expect(confirm.textContent).toContain("Only rows quarantined as conflicting duplicates");
    await user.type(
      within(confirm).getByRole("textbox", { name: /Reason/ }),
      "Corrected re-export",
    );
    await user.click(within(confirm).getByRole("button", { name: "Accept row" }));
    expect(await screen.findByText("Accept as reprocessing: recorded")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        itemId: "item-1",
        expectedRevision: 1,
        resolution: "accept_as_reprocessing",
        reason: "Corrected re-export",
      },
    });
    await closePanelWithEscape(user, "Row R4", "R4");
  });

  it("offers acceptance as reprocessing only on a conflicting duplicate", async () => {
    const unmapped = {
      ...DETAIL.quarantine[0],
      itemId: "item-3",
      recordPath: "R6",
      reason: "unknown_analyte" as const,
    };
    const { user } = renderPage(`/review/imports/${ATTEMPT_ID}`, {
      get_import_attempt: () => ({ ...DETAIL, quarantine: [...DETAIL.quarantine, unmapped] }),
    });
    await screen.findByRole("grid", { name: "Quarantined rows" });
    expect(await rowActionLabels(user, "R6")).toEqual(["Reject row"]);
    await openRow(user, "R6");
    const row = panel("Row R6");
    expect(within(row).queryByRole("button", { name: "Accept as reprocessing" })).toBeNull();
    expect(within(row).getByRole("button", { name: "Reject row" })).toBeTruthy();
  });

  it("rejects an open row from its row menu, and offers no decision on a decided one", async () => {
    const resolve = vi.fn(() => DETAIL);
    const { user } = renderPage(`/review/imports/${ATTEMPT_ID}`, {
      resolve_quarantine_item: resolve,
    });
    await openRow(user, "R5");
    expect(within(panel("Row R5")).queryByRole("button", { name: "Reject row" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for R5" })).toBeNull();
    const menu = await openRowActions(user, "R4");
    await user.click(within(menu).getByRole("menuitem", { name: "Reject row" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Reject row" });
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Unknown sample");
    await user.click(within(confirm).getByRole("button", { name: "Reject row" }));
    expect(await screen.findByText("Reject row: recorded")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        itemId: "item-1",
        expectedRevision: 1,
        resolution: "reject",
        reason: "Unknown sample",
      },
    });
  });

  it("retries the import with a reason", async () => {
    const retry = vi.fn(() => 2);
    const { user } = renderPage(`/review/imports/${ATTEMPT_ID}`, { retry_import: retry });
    await user.click(await screen.findByRole("button", { name: "Retry import" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Source fixed");
    await user.click(within(confirm).getByRole("button", { name: "Retry import" }));
    expect(await screen.findByText(/Retry queued/)).toBeTruthy();
    expect(retry).toHaveBeenCalledWith({
      input: { receiptId: DETAIL.receiptId, reason: "Source fixed" },
    });
  });

  it("shows one instrument's imports when the address names it", async () => {
    const list = vi.fn(() => []);
    renderPage(`/review/imports?instrument=${INSTRUMENT.instrumentId}`, {
      list_import_attempts: list,
    });
    expect(
      await screen.findByText("No message from this instrument has been processed yet."),
    ).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: {
        instrumentId: INSTRUMENT.instrumentId,
        limit: 200,
        before: null,
        afterImportAttemptId: undefined,
      },
    });
    expect(
      await screen.findByRole("button", { name: /Synthetic analyzer 1 Instrument/ }),
    ).toBeTruthy();
  });

  it("says when nothing has been imported", async () => {
    renderPage("/review/imports", { list_import_attempts: () => [] });
    expect(await screen.findByText("No imports")).toBeTruthy();
  });

  it("shows loading under the page title", async () => {
    renderPage(`/review/imports/${ATTEMPT_ID}`, {
      get_import_attempt: () => new Promise(() => undefined),
    });
    expect(await screen.findByText("Loading import…")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Import attempt" })).toBeTruthy();
  });

  it("shows an import that cannot be loaded", async () => {
    renderPage(`/review/imports/${ATTEMPT_ID}`, {
      get_import_attempt: () =>
        Promise.reject(engineError("not_found", "The import was not found.")),
    });
    expect(await screen.findByText("The import was not found.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Import attempt" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Retry import" })).toBeNull();
  });
});
