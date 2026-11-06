import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { INSTRUMENT, sessionWith } from "../testData";
import { ImportAttemptsPage } from "./ImportAttemptsPage";

const ATTEMPT: ImportAttemptSummary = {
  importAttemptId: "00000000-0000-7000-8000-000000008101",
  receiptId: "00000000-0000-7000-8000-000000009101",
  attemptNo: 1,
  instrumentId: INSTRUMENT.instrumentId,
  outcome: "quarantined",
  failureCode: "unregistered_source",
  rowsTotal: 3,
  rowsNew: 0,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 0,
  rowsQuarantined: 3,
  rowsNotResult: 0,
  rowsSetAside: 0,
  rowsRetained: 0,
  processedAt: "2026-11-04T10:00:00Z",
  receivedAt: "2026-11-04T09:59:00Z",
};

/** The engine filter of an unfiltered first page. */
const NONE = {
  instrumentId: null,
  outcome: null,
  receivedFrom: null,
  receivedBefore: null,
  search: null,
  limit: 200,
  before: null,
  afterImportAttemptId: undefined,
};

function renderList(path: string, list: (args: unknown) => unknown) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_instrument_readiness: () => [INSTRUMENT],
    list_import_attempts: list,
  });
  return renderRoutes([{ path: "/review/imports", element: <ImportAttemptsPage /> }], path);
}

describe("Result imports filters (REQ-ANA-003)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("sends the outcome and the search to the engine and keeps them in the address", async () => {
    const list = vi.fn(() => [ATTEMPT]);
    const { user, router, container } = renderList("/review/imports", list);
    await screen.findByRole("gridcell", { name: "Synthetic analyzer 1" });
    expect(screen.getByRole("columnheader", { name: "Received" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /Outcome/ }));
    await user.click(await screen.findByRole("option", { name: "Quarantined" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({ input: { ...NONE, outcome: "quarantined" } }),
    );
    await user.type(
      screen.getByRole("searchbox", {
        name: "Find imports by sample label, accession, container label or file name",
      }),
      " DUPTEST01 {Enter}",
    );
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { ...NONE, outcome: "quarantined", search: "DUPTEST01" },
      }),
    );
    expect(router.state.location.search).toBe("?outcome=quarantined&q=DUPTEST01");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("reads the filters from the address, received days included, and clears them at once", async () => {
    const list = vi.fn(() => [ATTEMPT]);
    const { user, router } = renderList(
      `/review/imports?instrument=${INSTRUMENT.instrumentId}&outcome=processed&from=2026-11-01&to=2026-11-04&q=C0009`,
      list,
    );
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          ...NONE,
          instrumentId: INSTRUMENT.instrumentId,
          outcome: "processed",
          receivedFrom: "2026-11-01T00:00:00.000000000Z",
          receivedBefore: "2026-11-05T00:00:00.000000000Z",
          search: "C0009",
        },
      }),
    );
    expect(screen.getByText("Received: 2026-11-01 to 2026-11-04")).toBeTruthy();
    await user.click(screen.getAllByRole("button", { name: "Clear filters" })[0] as HTMLElement);
    await waitFor(() => expect(list).toHaveBeenLastCalledWith({ input: NONE }));
    expect(router.state.location.search).toBe("");
    expect(screen.queryByRole("button", { name: "Clear filters" })).toBeNull();
  });

  it("applies a received day once it is a real day, and says what is wrong until then", async () => {
    const list = vi.fn(() => [ATTEMPT]);
    const { user, router } = renderList("/review/imports", list);
    await screen.findByRole("gridcell", { name: "Synthetic analyzer 1" });
    await user.click(screen.getByRole("button", { name: "More filters" }));
    const from = screen.getByRole("textbox", { name: "Received from" });
    await user.type(from, "2026-11-3");
    expect(screen.getByText("Enter the day as YYYY-MM-DD.")).toBeTruthy();
    await user.clear(from);
    await user.type(from, "2026-11-03");
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { ...NONE, receivedFrom: "2026-11-03T00:00:00.000000000Z" },
      }),
    );
    expect(router.state.location.search).toBe("?from=2026-11-03");
    await user.type(screen.getByRole("textbox", { name: "Received to" }), "2026-11-02");
    expect(screen.getByText("Choose a day on or after the start day.")).toBeTruthy();
  });

  it("says when no import matches the filters, and ignores a day that is not real", async () => {
    const list = vi.fn(() => []);
    renderList("/review/imports?outcome=quarantined&from=2026-02-30", list);
    expect(await screen.findByText("No import matches these filters.")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({ input: { ...NONE, outcome: "quarantined" } });
  });
});
