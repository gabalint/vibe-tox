import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { selectRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { INSTRUMENT, sessionWith } from "../testData";
import { ImportAttemptsPage } from "./ImportAttemptsPage";

const HELD: ImportAttemptSummary = {
  importAttemptId: "00000000-0000-7000-8000-000000008201",
  receiptId: "00000000-0000-7000-8000-000000009201",
  attemptNo: 1,
  instrumentId: INSTRUMENT.instrumentId,
  outcome: "quarantined",
  failureCode: "unregistered_source",
  rowsTotal: 2,
  rowsNew: 0,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 0,
  rowsQuarantined: 2,
  rowsNotResult: 0,
  rowsSetAside: 0,
  rowsRetained: 0,
  processedAt: "2026-11-04T10:00:00Z",
  receivedAt: "2026-11-04T09:58:00Z",
};
const PARTLY: ImportAttemptSummary = {
  ...HELD,
  importAttemptId: "00000000-0000-7000-8000-000000008202",
  receiptId: "00000000-0000-7000-8000-000000009202",
  outcome: "processed_with_quarantine",
  failureCode: null,
  rowsNew: 1,
  rowsQuarantined: 1,
  processedAt: "2026-11-04T11:00:00Z",
  receivedAt: "2026-11-04T10:58:00Z",
};
const DONE: ImportAttemptSummary = {
  ...PARTLY,
  importAttemptId: "00000000-0000-7000-8000-000000008203",
  receiptId: "00000000-0000-7000-8000-000000009203",
  outcome: "processed",
  rowsNew: 2,
  rowsQuarantined: 0,
  processedAt: "2026-11-04T12:00:00Z",
  receivedAt: "2026-11-04T11:58:00Z",
};
/** A second attempt of the held message: one retry covers both. */
const HELD_AGAIN: ImportAttemptSummary = {
  ...HELD,
  importAttemptId: "00000000-0000-7000-8000-000000008204",
  attemptNo: 2,
  processedAt: "2026-11-04T13:00:00Z",
};

function renderList(retry: (args: unknown) => unknown, rows = [HELD, PARTLY, DONE]) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_instrument_readiness: () => [INSTRUMENT],
    list_import_attempts: () => rows,
    retry_import: retry,
  });
  return renderRoutes(
    [{ path: "/review/imports", element: <ImportAttemptsPage /> }],
    "/review/imports",
  );
}

describe("Retry selected imports (REQ-ANA-003)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("selects every attempt with something to retry and retries each with one reason", async () => {
    const retry = vi.fn(() => 2);
    const { user, container } = renderList(retry);
    await screen.findByRole("gridcell", { name: "2026-11-04 12:00 UTC" });
    // A fully processed attempt has nothing to retry and can't be checked.
    await selectRow(user, "2026-11-04 12:00 UTC");
    expect(screen.getByText("None selected.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Select all quarantined" }));
    expect(screen.getByText("2 selected.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Retry selected" }));
    const dialog = await screen.findByRole("dialog", { name: "Retry 2 imports" });
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(within(dialog).getByRole("button", { name: "Retry selected" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(retry).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), " Source approved ");
    await user.click(within(dialog).getByRole("button", { name: "Retry selected" }));
    expect(await screen.findByText("2 retries queued")).toBeTruthy();
    expect(retry.mock.calls).toEqual([
      [{ input: { receiptId: HELD.receiptId, reason: "Source approved" } }],
      [{ input: { receiptId: PARTLY.receiptId, reason: "Source approved" } }],
    ]);
    expect(screen.getByText("None selected.")).toBeTruthy();
  });

  it("retries a message once for two of its attempts, and goes on with the rest after a refusal", async () => {
    const retry = vi
      .fn()
      .mockReturnValueOnce(3)
      .mockRejectedValueOnce(engineError("not_found", "The receipt is not complete."))
      .mockReturnValueOnce(2);
    const { user } = renderList(retry, [HELD, HELD_AGAIN, PARTLY]);
    await screen.findByRole("gridcell", { name: "2026-11-04 13:00 UTC" });
    await selectRow(user, "2026-11-04 13:00 UTC");
    await selectRow(user, "2026-11-04 10:00 UTC");
    await selectRow(user, "2026-11-04 11:00 UTC");
    await user.click(screen.getByRole("button", { name: "Retry selected" }));
    const dialog = await screen.findByRole("dialog", { name: "Retry 2 imports" });
    expect(dialog.textContent).toContain("3 attempts were selected from 2 received messages");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Run corrected");
    await user.click(within(dialog).getByRole("button", { name: "Retry selected" }));
    expect(await within(dialog).findByText("The receipt is not complete.")).toBeTruthy();
    expect(within(dialog).getByText("Not queued: 1 retry")).toBeTruthy();
    expect(dialog.textContent).toContain("Already queued: 1 retry.");
    await user.click(within(dialog).getByRole("button", { name: "Retry the rest" }));
    expect(await screen.findByText("2 retries queued")).toBeTruthy();
    await waitFor(() => expect(retry).toHaveBeenCalledTimes(3));
    expect(retry.mock.calls.map(([args]) => args.input.receiptId)).toEqual([
      HELD.receiptId,
      PARTLY.receiptId,
      PARTLY.receiptId,
    ]);
  });
});
