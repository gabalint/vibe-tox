import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppRunView } from "@/generated/AppRunView";
import type { HealthEventView } from "@/generated/HealthEventView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { healthCheck, hostHealth, sessionWith } from "../testData";
import { HealthHistoryPage } from "./HealthHistoryPage";

const PATH = "/operations/health/history";
const GAP_RUN = "00000000-0000-7000-8000-0000000000f2";

const EVENTS: HealthEventView[] = [
  {
    eventId: "00000000-0000-7000-8000-000000000001",
    check: "disk_free",
    state: "warning",
    detailCode: "below_threshold",
    observedAt: "2026-10-13T12:00:00Z",
  },
  {
    eventId: "00000000-0000-7000-8000-000000000002",
    check: "backup_age",
    state: "ok",
    detailCode: null,
    observedAt: "2026-10-13T11:00:00Z",
  },
];

function run(overrides: Partial<AppRunView> = {}): AppRunView {
  return {
    runId: "00000000-0000-7000-8000-0000000000f1",
    startedAt: "2026-10-12T08:00:00Z",
    stoppedAt: "2026-10-12T18:00:00Z",
    stopKind: "clean",
    appVersion: "0.4.0",
    webview: "WebKitGTK 2.44",
    gapSeconds: null,
    previousUnclean: false,
    clockGapSeconds: null,
    needsAcknowledgement: false,
    acknowledged: null,
    ...overrides,
  };
}

const RUNS = [
  run({
    runId: GAP_RUN,
    startedAt: "2026-10-13T08:00:00Z",
    stoppedAt: null,
    stopKind: null,
    gapSeconds: 840,
    previousUnclean: true,
    clockGapSeconds: 120,
    needsAcknowledgement: true,
  }),
  run(),
];

const HEALTH = hostHealth({
  checks: [healthCheck(), healthCheck({ check: "disk_free", label: "Free disk space" })],
});

function renderPage(
  path: string,
  commands: Record<string, (args: unknown) => unknown>,
  capabilities = ["perform_analysis"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_host_health: () => HEALTH,
    ...commands,
  });
  return renderRoutes(
    [
      { path: PATH, element: <HealthHistoryPage /> },
      { path: `${PATH}/:tab`, element: <HealthHistoryPage /> },
    ],
    path,
  );
}

const panel = (name: string | RegExp) => screen.getByRole("region", { name });

describe("HealthHistoryPage (REQ-OPS-008, REQ-OPS-013)", () => {
  beforeEach(mockGridLayout);

  it("starts on the app runs, and a tab shows the state changes, filtered by a check kept in the address", async () => {
    const history = vi.fn(() => EVENTS);
    const { user, container, router } = renderPage(PATH, {
      list_health_history: history,
      list_app_runs: () => RUNS,
    });
    expect(await screen.findByRole("gridcell", { name: "840 s" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Closed normally" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Yes" })).toBeTruthy();
    expect(screen.queryByRole("grid", { name: "Health history" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("tab", { name: "State changes" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PATH}/changes`));
    expect(await screen.findByRole("gridcell", { name: "Free disk space" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "below_threshold" })).toBeTruthy();
    expect(history).toHaveBeenCalledWith({
      input: { check: null, limit: 100, beforeEventId: undefined },
    });

    await chooseWithKeyboard(user, /Check$/, "Free disk space");
    await waitFor(() =>
      expect(history).toHaveBeenLastCalledWith({
        input: { check: "disk_free", limit: 100, beforeEventId: undefined },
      }),
    );
    expect(router.state.location.search).toBe("?check=disk_free");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(router.state.location.search).toBe(""));
  });

  it("starts from the check in the address", async () => {
    const history = vi.fn(() => []);
    const { container } = renderPage(`${PATH}/changes?check=backup_age`, {
      list_health_history: history,
      list_app_runs: () => [],
    });
    expect(await screen.findByText("No health history")).toBeTruthy();
    expect(history).toHaveBeenCalledWith({
      input: { check: "backup_age", limit: 100, beforeEventId: undefined },
    });
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens an older queue target through an exact lookup independently of loaded pages", async () => {
    const gap = run({ runId: "00000000-0000-7000-8000-0000000000f9", needsAcknowledgement: true });
    const runs = vi.fn((args: unknown) => {
      const input = (args as { input: { runId?: string } }).input;
      return input.runId ? [gap] : RUNS;
    });
    renderPage(`${PATH}?open=${gap.runId}`, { list_health_history: () => [], list_app_runs: runs });
    expect(await screen.findByRole("region", { name: /^Run started/ })).toBeTruthy();
    expect(runs).toHaveBeenCalledWith({ input: { limit: 200, beforeRunId: undefined } });
    expect(runs).toHaveBeenCalledWith({ input: { limit: 1, runId: gap.runId } });
  });

  it("loads every state change beyond 100, including repeated equal-time observations", async () => {
    const events = Array.from({ length: 103 }, (_, index) => ({
      ...EVENTS[0],
      eventId: `event-${index}`,
      detailCode: `observation-${index}`,
    }));
    const history = vi.fn((args: unknown) => {
      const input = (args as { input: { beforeEventId?: string } }).input;
      return input.beforeEventId === "event-99" ? events.slice(100) : events.slice(0, 101);
    });
    const { user } = renderPage(`${PATH}/changes`, {
      list_health_history: history,
      list_app_runs: () => [],
    });
    await user.click(await screen.findByRole("button", { name: "Load older state changes" }));
    await waitFor(() =>
      expect(history).toHaveBeenLastCalledWith({
        input: { check: null, limit: 100, beforeEventId: "event-99" },
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Load older state changes" })).toBeNull(),
    );
    expect(screen.getByText(/103 state changes/)).toBeTruthy();
  });

  it("shows all checks for a check code the engine does not know in the address", async () => {
    const history = vi.fn(() => []);
    renderPage(`${PATH}/changes?check=not_a_check`, {
      list_health_history: history,
      list_app_runs: () => [],
    });
    expect(await screen.findByText("No health history")).toBeTruthy();
    expect(history).toHaveBeenCalledWith({
      input: { check: null, limit: 100, beforeEventId: undefined },
    });
  });

  it("explains an empty list of runs", async () => {
    renderPage(PATH, { list_health_history: () => [], list_app_runs: () => [] });
    expect(await screen.findByText("No app runs")).toBeTruthy();
  });

  it("opens a run by click and by Enter, and Escape closes the panel with focus on the row", async () => {
    const { user, router } = renderPage(PATH, {
      list_health_history: () => EVENTS,
      list_app_runs: () => RUNS,
    });
    await openRow(user, "840 s");
    expect(router.state.location.search).toBe(`?open=${GAP_RUN}`);
    const open = panel(/^Run started/);
    expect(within(open).getByText("Gap not acknowledged")).toBeTruthy();
    expect(screen.queryByText(/Select a run/)).toBeNull();
    await user.click(within(open).getByRole("button", { name: "Next" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: /^Run started/ })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement?.closest("[role=row]")?.textContent).toContain(
        "Closed normally",
      ),
    );
    await openRowWithEnter(user, "840 s");
    expect(panel(/^Run started/)).toBeTruthy();
  });

  it("opens the run named in the address, and says when the list does not hold it", async () => {
    const { router } = renderPage(`${PATH}?open=${GAP_RUN}`, {
      list_health_history: () => EVENTS,
      list_app_runs: () => RUNS,
    });
    const open = await screen.findByRole("region", { name: /^Run started/ });
    expect(within(open).getByText("120 s")).toBeTruthy();
    await router.navigate(`${PATH}?open=00000000-0000-7000-8000-0000000000ff`);
    expect(await screen.findByRole("region", { name: "Record not found" })).toBeTruthy();
  });

  it("acknowledges a listening gap with a note, from the panel", async () => {
    const acknowledge = vi.fn(() => ({
      ...RUNS[0],
      needsAcknowledgement: false,
      acknowledged: { by: "Test Operator", at: "2026-10-13T09:00:00Z" },
    }));
    const { user } = renderPage(`${PATH}?open=${GAP_RUN}`, {
      list_health_history: () => EVENTS,
      list_app_runs: () => RUNS,
      acknowledge_listening_gap: acknowledge,
    });
    const open = await screen.findByRole("region", { name: /^Run started/ });
    await user.click(within(open).getByRole("button", { name: "Acknowledge listening gap" }));
    const dialog = await screen.findByRole("dialog", { name: "Acknowledge listening gap" });
    expect(
      within(dialog).getByText(/for 840 seconds before this start and 120 seconds/),
    ).toBeTruthy();
    await typeInto(user, /^Note/, "Analyzer results resent");
    await user.click(within(dialog).getByRole("button", { name: "Acknowledge" }));
    expect(await screen.findByText("Listening gap acknowledged")).toBeTruthy();
    expect(acknowledge).toHaveBeenCalledWith({
      input: { runId: GAP_RUN, note: "Analyzer results resent" },
    });
  });

  it("offers the acknowledgement in the row's menu only for a run that needs it", async () => {
    const { user } = renderPage(PATH, {
      list_health_history: () => EVENTS,
      list_app_runs: () => RUNS,
    });
    await screen.findByRole("gridcell", { name: "840 s" });
    const buttons = screen.getAllByRole("button", { name: /^Actions for run started/ });
    expect(buttons).toHaveLength(1);
    const [button] = buttons;
    if (!button) throw new Error("no row menu button");
    await user.click(button);
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["Acknowledge listening gap"]);
    await user.click(within(menu).getByRole("menuitem"));
    expect(await screen.findByRole("dialog", { name: "Acknowledge listening gap" })).toBeTruthy();
  });

  it("shows the engine's errors and offers no acknowledgement without perform analysis", async () => {
    renderPage(
      `${PATH}/changes`,
      {
        list_health_history: () => Promise.reject(engineError("internal", "History unavailable.")),
        list_app_runs: () => RUNS,
      },
      [],
    );
    expect(await screen.findByText("History unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("offers no acknowledgement without perform analysis, in the panel or the row", async () => {
    const { user } = renderPage(
      `${PATH}?open=${GAP_RUN}`,
      { list_health_history: () => EVENTS, list_app_runs: () => RUNS },
      [],
    );
    await screen.findByRole("region", { name: /^Run started/ });
    expect(screen.queryByRole("button", { name: "Acknowledge listening gap" })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Actions for/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Close" }));
  });
});
