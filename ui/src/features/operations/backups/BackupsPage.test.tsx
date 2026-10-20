import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RestoreTestView } from "@/generated/RestoreTestView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  ANCHORS,
  BACKUP_STATUS,
  backupRun,
  backupSet,
  REMOVABLE_ID,
  SET_ID,
  sessionWith,
  TARGET_ID,
  target,
} from "../testData";
import { BackupsPage } from "./BackupsPage";

const TEST: RestoreTestView = {
  testId: "00000000-0000-7000-8000-0000000000b4",
  targetLabel: "Synthetic network share",
  setId: SET_ID,
  setCreatedAt: "2026-10-13T14:00:00Z",
  state: "failed",
  requestedBy: "Test Operator",
  requestedAt: "2026-10-13T14:59:00Z",
  startedAt: "2026-10-13T15:00:00Z",
  finishedAt: "2026-10-13T15:00:12Z",
  elapsedMs: 12_345,
  failureCode: null,
  anchors: ANCHORS,
  findings: [{ code: "artifact_missing", count: 2 }],
  passed: false,
};

/** A test the engine has queued: the newest set is chosen when it runs. */
const QUEUED: RestoreTestView = {
  ...TEST,
  testId: "00000000-0000-7000-8000-0000000000b5",
  setId: null,
  setCreatedAt: null,
  state: "requested",
  requestedAt: "2026-10-13T16:00:00Z",
  startedAt: null,
  finishedAt: null,
  elapsedMs: null,
  anchors: null,
  findings: [],
};

const BASE = "/operations/backups";

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  path = BASE,
  capabilities = ["backup_restore"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_backup_status: () => BACKUP_STATUS,
    list_backup_targets: () => [target()],
    list_backup_runs: () => [backupRun()],
    list_backup_sets: () => [backupSet()],
    list_restore_tests: () => [],
    ...commands,
  });
  return renderRoutes(
    [
      { path: BASE, element: <BackupsPage /> },
      { path: `${BASE}/:tab`, element: <BackupsPage /> },
    ],
    path,
  );
}

const panel = (name: string) => screen.getByRole("region", { name });

describe("BackupsPage (REQ-OPS-007)", () => {
  beforeEach(mockGridLayout);

  it("shows the status band and the targets, and requests a manual backup", async () => {
    const request = vi.fn(() => backupRun({ kind: "manual", state: "requested" }));
    const { user, container } = renderPage({ request_backup: request });
    expect(await screen.findByText("Every change is in a verified copy")).toBeTruthy();
    expect(await screen.findByRole("gridcell", { name: "Synthetic network share" })).toBeTruthy();
    // The usage hint is gone; the toolbar only counts.
    expect(screen.queryByText(/Select a target/)).toBeNull();
    expect(screen.getByText("1 of 1 targets")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Targets" }).getAttribute("aria-selected")).toBe("true");
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Back up now" }));
    expect(await screen.findByText("Backup requested")).toBeTruthy();
    expect(screen.queryByText(/successful/i)).toBeNull();
    expect(request).toHaveBeenCalledWith({ input: { kind: "manual" } });
  });

  it("links the latest run and restore test in the band to their tabs", async () => {
    const { user, router } = renderPage({ list_restore_tests: () => [TEST] });
    const run = await screen.findByRole("link", { name: /Scheduled backup, verified/ });
    const restoreTest = await screen.findByRole("link", {
      name: /Synthetic network share, failed/,
    });
    expect(run.getAttribute("href")).toBe(`${BASE}/runs`);
    expect(restoreTest.getAttribute("href")).toBe(`${BASE}/restore-tests`);
    await user.click(restoreTest);
    await waitFor(() => expect(router.state.location.pathname).toBe(`${BASE}/restore-tests`));
    expect(screen.getByRole("tab", { name: "Restore tests" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(await screen.findByRole("grid", { name: "Restore tests" })).toBeTruthy();
  });

  it("says when there is no run or restore test yet", async () => {
    renderPage({ list_backup_runs: () => [] });
    expect(await screen.findByText("No backup runs yet")).toBeTruthy();
    expect(await screen.findByText("No restore tests yet")).toBeTruthy();
  });

  it("follows the tab in the address", async () => {
    const { user, router } = renderPage({}, `${BASE}/runs`);
    expect(await screen.findByRole("grid", { name: "Backup runs" })).toBeTruthy();
    expect(screen.queryByRole("grid", { name: "Backup targets" })).toBeNull();
    expect(
      await screen.findByRole("gridcell", { name: "Synthetic network share: verified" }),
    ).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Targets" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`${BASE}/targets`));
    expect(await screen.findByRole("grid", { name: "Backup targets" })).toBeTruthy();
  });

  it("shows the engine's refusal to back up", async () => {
    const { user } = renderPage({
      request_backup: () =>
        Promise.reject(engineError("conflict", "Add a backup target before backing up.")),
    });
    await user.click(await screen.findByRole("button", { name: "Back up now" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Add a backup target before backing up.");
    expect(alert.textContent).toContain("corr-test-0001");
  });

  it("explains that nothing is backed up without a target", async () => {
    const { container } = renderPage({
      list_backup_targets: () => [],
      list_backup_runs: () => [],
      get_backup_status: () => ({ ...BACKUP_STATUS, newestVerified: null, upToDate: false }),
    });
    expect(await screen.findByText("No backup targets")).toBeTruthy();
    expect(screen.getByText("Changes not yet in a verified copy")).toBeTruthy();
    expect(screen.getByText("None yet")).toBeTruthy();
    // The empty state offers the first action as well as the toolbar.
    expect(screen.getAllByRole("button", { name: "Add target" })).toHaveLength(2);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's error, loading and header", async () => {
    renderPage({
      list_backup_targets: () =>
        Promise.reject(engineError("internal", "Targets are unavailable.")),
    });
    expect(screen.getByRole("heading", { level: 1, name: "Backups" })).toBeTruthy();
    expect(await screen.findByText("Targets are unavailable.")).toBeTruthy();
  });

  it("adds a target, keeping the dialog open when the folder dialog is cancelled", async () => {
    const answers: unknown[] = [null, target({ targetId: REMOVABLE_ID, label: "Synthetic USB" })];
    const add = vi.fn(() => answers.shift());
    const { user, container } = renderPage({ add_backup_target: add });
    await user.click(await screen.findByRole("button", { name: "Add target" }));
    const dialog = await screen.findByRole("dialog", { name: "Add backup target" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), "Synthetic USB");
    await chooseWithKeyboard(user, /Kind$/, "Removable drive");
    await chooseWithKeyboard(user, /Receives$/, "Backups and audit anchors");
    await user.click(within(dialog).getByRole("button", { name: "Choose folder" }));
    expect(await within(dialog).findByText(/folder dialog was cancelled/)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(within(dialog).getByRole("button", { name: "Choose folder" }));
    expect(await screen.findByText("Synthetic USB added as a backup target")).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(add).toHaveBeenLastCalledWith({
      input: { label: "Synthetic USB", kind: "removable", purpose: "backup_and_anchors" },
    });
  });

  it("opens a target by click and by Enter, and Escape closes the panel with focus on the row", async () => {
    const { user, router } = renderPage({
      list_backup_targets: () => [
        target(),
        target({ targetId: REMOVABLE_ID, label: "Synthetic USB", kind: "removable" }),
      ],
    });
    await openRow(user, "Synthetic network share");
    expect(router.state.location.search).toBe(`?open=${TARGET_ID}`);
    const open = panel("Synthetic network share");
    expect(within(open).getByRole("heading", { level: 2 }).textContent).toBe(
      "Synthetic network share",
    );
    expect(within(open).getByText("\\\\synthetic-nas\\backups")).toBeTruthy();
    expect(
      await within(open).findByRole("list", { name: "Sets on Synthetic network share" }),
    ).toBeTruthy();
    const cell = screen.getByRole("gridcell", { name: "Synthetic network share" });
    expect(cell.closest("[role=row]")?.getAttribute("aria-current")).toBe("true");

    // Focus moves into the panel by using its Next button; Escape there closes the panel.
    await user.click(within(open).getByRole("button", { name: "Next" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "Synthetic USB" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "Synthetic USB" })),
    );

    await openRowWithEnter(user, "Synthetic network share");
    expect(panel("Synthetic network share")).toBeTruthy();
  });

  it("opens the target named in the address", async () => {
    renderPage({}, `${BASE}?open=${TARGET_ID}`);
    expect(await screen.findByRole("region", { name: "Synthetic network share" })).toBeTruthy();
  });

  it("retires a target with a reason, from the panel, offered only when the engine allows it", async () => {
    const retire = vi.fn(() => target({ retired: true, allowedActions: [] }));
    const { user } = renderPage({ retire_backup_target: retire });
    await openRow(user, "Synthetic network share");
    const open = panel("Synthetic network share");
    await user.click(within(open).getByRole("button", { name: "Retire target" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Drive replaced");
    await user.click(within(dialog).getByRole("button", { name: "Retire target" }));
    expect(await screen.findByText("Synthetic network share retired")).toBeTruthy();
    expect(retire).toHaveBeenCalledWith({
      input: { targetId: TARGET_ID, expectedRevision: 1, reason: "Drive replaced" },
    });
  });

  it("offers the same actions in the row's menu as in the panel", async () => {
    const { user } = renderPage();
    await screen.findByRole("gridcell", { name: "Synthetic network share" });
    const menu = await openRowActions(user, "Synthetic network share");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["Request restore test", "Retire target"]);
    await user.click(within(menu).getByRole("menuitem", { name: "Request restore test" }));
    expect(await screen.findByRole("dialog", { name: /Test a restore/ })).toBeTruthy();
  });

  it("hides target actions the engine does not allow and lists the target's sets", async () => {
    const { user } = renderPage({
      list_backup_targets: () => [target({ allowedActions: [], available: false })],
    });
    await openRow(user, "Synthetic network share");
    expect(
      await screen.findByRole("list", { name: "Sets on Synthetic network share" }),
    ).toBeTruthy();
    expect(screen.getAllByText("Not connected").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Retire target" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Request restore test" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Actions for/ })).toBeNull();
  });

  it("hides retired targets until asked, and keeps the choice in the address", async () => {
    const { user, router } = renderPage({
      list_backup_targets: () => [
        target(),
        target({
          targetId: REMOVABLE_ID,
          label: "Synthetic USB",
          retired: true,
          allowedActions: [],
        }),
      ],
    });
    expect(await screen.findByRole("gridcell", { name: "Synthetic network share" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "Synthetic USB" })).toBeNull();
    expect(screen.getByText("1 of 2 targets")).toBeTruthy();
    await user.click(screen.getByRole("checkbox", { name: "Show retired" }));
    expect(await screen.findByRole("gridcell", { name: "Synthetic USB" })).toBeTruthy();
    expect(router.state.location.search).toBe("?retired=show");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() =>
      expect(screen.queryByRole("gridcell", { name: "Synthetic USB" })).toBeNull(),
    );
  });

  it("says when every target is retired", async () => {
    const { user } = renderPage({
      list_backup_targets: () => [target({ retired: true, allowedActions: [] })],
    });
    expect(await screen.findByText("No active backup targets")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Show retired targets" }));
    expect(await screen.findByRole("gridcell", { name: "Synthetic network share" })).toBeTruthy();
  });

  it("requests a restore test from the panel and shows a test's findings", async () => {
    const requestTest = vi.fn(() => TEST);
    const { user, router } = renderPage({
      request_restore_test: requestTest,
      list_restore_tests: () => [TEST],
    });
    await openRow(user, "Synthetic network share");
    await user.click(
      within(panel("Synthetic network share")).getByRole("button", {
        name: "Request restore test",
      }),
    );
    const dialog = await screen.findByRole("dialog", { name: /Test a restore/ });
    await user.click(within(dialog).getByRole("button", { name: "Start restore test" }));
    expect(await screen.findByText("Restore test requested")).toBeTruthy();
    expect(requestTest).toHaveBeenCalledWith({ input: { targetId: TARGET_ID, setId: null } });

    await router.navigate(`${BASE}/restore-tests`);
    await openRow(user, "12.3 s");
    const open = await screen.findByRole("region", {
      name: /^Restore test of Synthetic network share/,
    });
    expect(within(open).getAllByText("Failed").length).toBe(1);
    expect(within(open).getByText("A stored file is missing from the backup: 2")).toBeTruthy();
    expect(within(open).getByText("Anchors compared")).toBeTruthy();
  });

  it("shows a queued restore test without a result", async () => {
    const { user, container } = renderPage(
      { list_restore_tests: () => [QUEUED] },
      `${BASE}/restore-tests`,
    );
    await openRowWithEnter(user, "Not finished");
    expect(await screen.findByText("Not started")).toBeTruthy();
    expect(screen.getByText("The newest set, chosen when the test runs")).toBeTruthy();
    expect(screen.getAllByText("Requested").length).toBeGreaterThan(0);
    expect(screen.queryByText("No findings.")).toBeNull();
    expect(screen.queryByText("Anchors compared")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens a run with each copy's outcome, and says when a run is not in the list", async () => {
    const { user, router } = renderPage(
      {
        list_backup_runs: (args: unknown) =>
          (args as { input: { runId?: string } }).input.runId ? [] : [backupRun()],
      },
      `${BASE}/runs`,
    );
    await openRow(user, "Synthetic network share: verified");
    const open = await screen.findByRole("region", { name: /^Scheduled backup,/ });
    expect(within(open).getByRole("list", { name: "Copies" })).toBeTruthy();
    expect(within(open).getByText("12")).toBeTruthy();
    await user.keyboard("{Escape}");
    await router.navigate(`${BASE}/runs?open=00000000-0000-7000-8000-0000000000ff`);
    expect(await screen.findByText("Record unavailable")).toBeTruthy();
  });

  it("explains an empty run list and an empty test list", async () => {
    const { container } = renderPage({ list_backup_runs: () => [] }, `${BASE}/runs`);
    expect(await screen.findByText("No backup runs")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("lists no sets for a retired target, which the engine no longer reads", async () => {
    const sets = vi.fn(() => [backupSet()]);
    const { user } = renderPage(
      {
        list_backup_targets: () => [
          target({ retired: true, available: false, allowedActions: [] }),
        ],
        list_backup_sets: sets,
      },
      `${BASE}?retired=show`,
    );
    await openRow(user, "Synthetic network share");
    expect(await screen.findByText(/not listed/)).toBeTruthy();
    expect(screen.queryByRole("list", { name: /Sets on/ })).toBeNull();
    expect(sets).not.toHaveBeenCalled();
  });
});

describe("Backup history continuation (mocked IPC)", () => {
  beforeEach(mockGridLayout);
  it("loads every backup page including equal request times and opens an exact old link", async () => {
    const first = Array.from({ length: 100 }, (_, index) => backupRun({ runId: `run-${index}` }));
    const old = backupRun({ runId: "old-run", failureCode: "old-retained-failure" });
    const list = vi.fn((args: unknown) => {
      const input = (args as { input: { beforeRunId?: string; runId?: string } }).input;
      if (input.runId) return [{ ...old, runId: input.runId }];
      return input.beforeRunId ? [old] : [...first, old];
    });
    const { user, router } = renderPage({ list_backup_runs: list }, `${BASE}/runs`);
    await user.click(await screen.findByRole("button", { name: "Load older backup runs" }));
    expect(await screen.findByText("101 backup runs shown")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: { limit: 100, beforeRequestedAt: null, beforeRunId: "run-99" },
    });
    await router.navigate(`${BASE}/runs?open=unloaded-exact`);
    expect(await screen.findByText("old-retained-failure")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { limit: 1, runId: "unloaded-exact" } });
  });
  it("loads older restore tests and reports an unavailable exact link", async () => {
    const first = Array.from({ length: 100 }, (_, index) => ({ ...TEST, testId: `test-${index}` }));
    const list = vi.fn((args: unknown) => {
      const input = (args as { input: { beforeTestId?: string; testId?: string } }).input;
      if (input.testId) throw engineError("not_found", "The selected restore test was not found.");
      return input.beforeTestId ? [{ ...TEST, testId: "older-test" }] : first;
    });
    const { user, router } = renderPage({ list_restore_tests: list }, `${BASE}/restore-tests`);
    await user.click(await screen.findByRole("button", { name: "Load older restore tests" }));
    expect(await screen.findByText("101 restore tests shown")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { limit: 100, beforeTestId: "test-99" } });
    await router.navigate(`${BASE}/restore-tests?open=missing`);
    expect(await screen.findByText(/The selected restore test could not be loaded/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeTruthy();
  });
});

describe("Exact background job recovery (mocked IPC)", () => {
  beforeEach(mockGridLayout);
  it("polls an exact old unfinished restore job until terminal without false missing state", async () => {
    let exactReads = 0;
    const list = vi.fn((args: unknown) => {
      const input = (args as { input: { testId?: string } }).input;
      if (!input.testId) return [];
      exactReads += 1;
      if (exactReads === 2) throw engineError("internal", "Synthetic exact job refresh failed.");
      return [
        {
          ...QUEUED,
          testId: input.testId,
          targetLabel: exactReads === 1 ? "Exact pending target" : "Exact completed target",
          state: exactReads === 1 ? "running" : "passed",
          passed: exactReads > 1,
        },
      ];
    });
    const { user } = renderPage(
      { list_restore_tests: list },
      `${BASE}/restore-tests?open=old-pending`,
    );
    expect(await screen.findByRole("region", { name: /Exact pending target/ })).toBeTruthy();
    expect(screen.queryByText("Record not found")).toBeNull();
    expect(
      await screen.findByText("Synthetic exact job refresh failed.", {}, { timeout: 6500 }),
    ).toBeTruthy();
    expect(screen.getByRole("region", { name: /Exact pending target/ })).toBeTruthy();
    vi.useFakeTimers();
    try {
      await vi.advanceTimersByTimeAsync(6000);
      expect(exactReads).toBe(2);
    } finally {
      vi.useRealTimers();
    }
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("region", { name: /Exact completed target/ })).toBeTruthy();
    expect(exactReads).toBe(3);
  }, 8000);
});

describe("Narrow backup history (mocked IPC)", () => {
  beforeEach(mockGridLayout);
  it("keeps the list visible when no record is selected", async () => {
    class FixedWidthObserver {
      constructor(private readonly callback: (entries: object[]) => void) {}
      observe(target: Element) {
        this.callback([
          {
            target,
            contentRect: { width: 600 },
            borderBoxSize: [{ inlineSize: 600, blockSize: 320 }],
          },
        ]);
      }
      unobserve() {}
      disconnect() {}
    }
    vi.stubGlobal("ResizeObserver", FixedWidthObserver);
    renderPage({}, `${BASE}/runs`);
    const grid = await screen.findByRole("grid", { name: "Backup runs" });
    expect(grid.closest("[hidden]")).toBeNull();
    expect(screen.queryByRole("region", { name: "Selected backup run" })).toBeNull();
  });
});
