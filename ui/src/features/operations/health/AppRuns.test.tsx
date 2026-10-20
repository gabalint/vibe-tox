import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppRunView } from "@/generated/AppRunView";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { sessionWith } from "../testData";
import { AppRuns } from "./AppRuns";

const PATH = "/operations/health/history";
const rows: AppRunView[] = Array.from({ length: 205 }, (_, index) => ({
  runId: `synthetic-run-${index}`,
  startedAt: "2026-10-14T08:00:00Z",
  stoppedAt: null,
  stopKind: null,
  appVersion: `synthetic-version-${index}`,
  webview: "Synthetic web engine",
  gapSeconds: null,
  previousUnclean: false,
  clockGapSeconds: null,
  needsAcknowledgement: false,
  acknowledged: null,
}));

function renderPage(open?: string, missing = false) {
  const list = vi.fn((args: unknown) => {
    const { input } = args as { input: { beforeRunId?: string; runId?: string } };
    if (input.runId) {
      if (missing) throw engineError("not_found", "Selected app run was not found.");
      return rows.filter((row) => row.runId === input.runId);
    }
    const offset = input.beforeRunId
      ? rows.findIndex((row) => row.runId === input.beforeRunId) + 1
      : 0;
    return rows.slice(offset, offset + 200);
  });
  mockCommands({ current_session: () => sessionWith([]), list_app_runs: list });
  const result = renderRoutes(
    [{ path: PATH, element: <AppRuns /> }],
    open ? `${PATH}?open=${open}` : PATH,
  );
  return { ...result, list };
}

describe("Application run history (UI-07/UI-09; mocked IPC)", () => {
  beforeEach(mockGridLayout);

  it("reaches older equal-time records and ends only after the last page", async () => {
    const { user, list } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Load older runs" }));
    await waitFor(() => expect(screen.getByText("205 application runs shown")).toBeTruthy());
    expect(list).toHaveBeenCalledWith({ input: { limit: 200, beforeRunId: "synthetic-run-199" } });
    expect(screen.queryByRole("button", { name: "Load older runs" })).toBeNull();
  });

  it("opens an exact older queued record and refreshes its independent read", async () => {
    const { user, list } = renderPage("synthetic-run-204");
    expect(await screen.findByText("synthetic-version-204")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { limit: 1, runId: "synthetic-run-204" } });
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() =>
      expect(
        list.mock.calls.filter(
          ([args]) => (args as { input: { runId?: string } }).input.runId === "synthetic-run-204",
        ).length,
      ).toBeGreaterThan(1),
    );
  });

  it("shows an exact lookup failure with recovery instead of a neighboring run", async () => {
    renderPage("synthetic-missing", true);
    expect(await screen.findByText("Selected app run was not found.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    expect(screen.queryByRole("region", { name: /^Run started/ })).toBeNull();
  });
});
