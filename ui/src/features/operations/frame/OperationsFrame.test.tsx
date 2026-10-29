import { QueryClientProvider } from "@tanstack/react-query";
import { act, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createQueryClient } from "@/lib/commands";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { GET_HOST_HEALTH, type RecoveryReconciliation } from "../contract";
import { ANCHORS, healthCheck, hostHealth } from "../testData";
import { HealthIndicator } from "./HealthIndicator";
import { OperationsBanners } from "./OperationsBanners";

const RECONCILIATION: RecoveryReconciliation = {
  restoreId: "00000000-0000-7000-8000-0000000000c1",
  setCreatedAt: "2026-10-13T08:00:00Z",
  appliedAt: "2026-10-13T09:30:00Z",
  lostIntervalFrom: "2026-10-13T08:00:00Z",
  items: [],
  anchors: ANCHORS,
  heldLabelJobs: 0,
  floors: { accession: "A26001000", container: "C00005000" },
  closed: null,
};

const NOT_ACCEPTED = healthCheck({
  check: "version",
  label: "App version",
  state: "warning",
  detailCode: "awaiting_acceptance",
  summary: "The running version is not yet accepted for clinical use.",
});

function renderFrame(commands: Record<string, () => unknown>) {
  mockCommands(commands);
  const cache = createQueryClient();
  const result = renderRoutes(
    [
      {
        path: "/",
        element: (
          <QueryClientProvider client={cache}>
            <main>
              <h1>Frame</h1>
              <HealthIndicator />
              <OperationsBanners />
            </main>
          </QueryClientProvider>
        ),
      },
    ],
    "/",
  );
  return { ...result, cache };
}

describe("Operations frame (REQ-OPS-002, REQ-OPS-006, REQ-OPS-008)", () => {
  it("shows the engine's overall health and both banners, with links", async () => {
    const { user, container } = renderFrame({
      get_host_health: () => hostHealth({ overall: "warning", checks: [NOT_ACCEPTED] }),
      get_recovery_reconciliation: () => RECONCILIATION,
    });
    expect(await screen.findByText("Health: Warning")).toBeTruthy();
    expect(await screen.findByText(/release, delivery and billing handoff wait/)).toBeTruthy();
    expect(screen.getByText(/not yet accepted for clinical use/)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("link", { name: "Open the recovery reconciliation" }));
    expect(await screen.findByText("Navigated to /operations/recovery")).toBeTruthy();
  });

  it("shows a version the laboratory did not approve as an alert, linking to the versions", async () => {
    const { user } = renderFrame({
      get_host_health: () =>
        hostHealth({
          overall: "alert",
          checks: [
            {
              ...NOT_ACCEPTED,
              state: "alert",
              detailCode: "started_without_approval",
              summary: "The running version started without an update approval.",
            },
          ],
        }),
      get_recovery_reconciliation: () => null,
    });
    expect(await screen.findByText("Health: Alert")).toBeTruthy();
    expect(await screen.findByText(/started without an update approval/)).toBeTruthy();
    expect(screen.queryByText(/release, delivery and billing handoff wait/)).toBeNull();
    await user.click(screen.getByRole("link", { name: "Versions and updates" }));
    expect(await screen.findByText("Navigated to /operations/versions/installed")).toBeTruthy();
  });

  it("dismisses the version notice with the keyboard while health and recovery remain visible", async () => {
    const health = hostHealth({ overall: "warning", checks: [NOT_ACCEPTED] });
    const { user, cache } = renderFrame({
      get_host_health: () => health,
      get_recovery_reconciliation: () => RECONCILIATION,
    });
    const dismiss = await screen.findByRole("button", { name: "Dismiss notification" });
    dismiss.focus();
    await user.keyboard("{Enter}");
    expect(screen.queryByText(NOT_ACCEPTED.summary)).toBeNull();
    expect(screen.getByText("Health: Warning")).toBeTruthy();
    expect(screen.getByText(/release, delivery and billing handoff wait/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Dismiss notification" })).toBeNull();
    await act(async () => {
      cache.setQueryData([GET_HOST_HEALTH], {
        ...health,
        checks: [{ ...NOT_ACCEPTED, observedAt: "2026-10-13T14:06:00Z" }],
      });
    });
    expect(screen.queryByText(NOT_ACCEPTED.summary)).toBeNull();
  });

  it.each([
    ["the running version changes", { host: { ...hostHealth().host, appVersion: "0.5.0" } }],
    [
      "the warning changes",
      { checks: [{ ...NOT_ACCEPTED, detailCode: "started_without_approval", state: "alert" }] },
    ],
  ])("shows a dismissed notice again when %s", async (_change, updated) => {
    const health = hostHealth({ overall: "warning", checks: [NOT_ACCEPTED] });
    const { user, cache } = renderFrame({
      get_host_health: () => health,
      get_recovery_reconciliation: () => null,
    });
    await user.click(await screen.findByRole("button", { name: "Dismiss notification" }));
    expect(screen.queryByText(NOT_ACCEPTED.summary)).toBeNull();
    await act(async () => {
      cache.setQueryData([GET_HOST_HEALTH], { ...health, ...updated });
    });
    expect(await screen.findByText(NOT_ACCEPTED.summary)).toBeTruthy();
  });

  it("shows a notice again if the warning clears and later returns", async () => {
    const health = hostHealth({ overall: "warning", checks: [NOT_ACCEPTED] });
    const { user, cache } = renderFrame({
      get_host_health: () => health,
      get_recovery_reconciliation: () => null,
    });
    await user.click(await screen.findByRole("button", { name: "Dismiss notification" }));
    await act(async () => {
      cache.setQueryData([GET_HOST_HEALTH], hostHealth());
    });
    await waitFor(() => expect(screen.getByText("Health: OK")).toBeTruthy());
    await act(async () => {
      cache.setQueryData([GET_HOST_HEALTH], health);
    });
    expect(await screen.findByText(NOT_ACCEPTED.summary)).toBeTruthy();
  });

  it("shows no banner when the reconciliation is closed and the version is fine", async () => {
    renderFrame({
      get_host_health: () => hostHealth({ checks: [{ ...NOT_ACCEPTED, state: "ok" }] }),
      get_recovery_reconciliation: () => ({
        ...RECONCILIATION,
        closed: { by: "Test Quality Manager", at: "2026-10-13T12:00:00Z" },
      }),
    });
    expect(await screen.findByText("Health: OK")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("says the health is unavailable when it cannot be read, and shows no banner", async () => {
    renderFrame({
      get_host_health: () => Promise.reject(engineError("internal", "No health.")),
      get_recovery_reconciliation: () => null,
    });
    expect(await screen.findByText("Health: unavailable")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
