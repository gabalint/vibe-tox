import { createMemoryRouter, RouterProvider } from "react-router";
import { TEST_SESSION } from "@/features/auth/testSession";
import { hostHealth } from "@/features/operations/testData";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { WorkQueueSummary } from "@/generated/WorkQueueSummary";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { routes } from "./routes";

/** Synthetic frame data for the shell tests. No real laboratory data. */

export const LISTENER = {
  instrumentId: "00000000-0000-7000-8000-0000000000a1",
  instrumentName: "Synthetic Analyzer 1",
  state: "receiving",
  lastActivityAt: "2026-10-12T14:03:05Z",
  detail: null,
};

export const LAB_PROFILE = {
  profileId: "00000000-0000-7000-8000-00000000f001",
  version: 1,
  name: "Synthetic Toxicology Laboratory",
  addressLines: ["1 Example Way", "Springfield, ZZ 00000"],
  cliaId: "00D0000000",
  director: "Dr. Test Director",
  phone: "555-0100",
  timeZone: "UTC",
  billingNpi: null,
  taxId: null,
  reason: null,
  setBy: "00000000-0000-7000-8000-000000000001",
  setAt: "2026-10-12T08:00:00Z",
};

export function queue(overrides: Partial<WorkQueueSummary>): WorkQueueSummary {
  return {
    queue: "review_ready",
    label: "Results ready for review",
    ownerCapability: "review_results",
    count: 0,
    countCapped: false,
    oldestSince: null,
    nextAction: "Review",
    ...overrides,
  };
}

/**
 * Renders the whole app at `path` for a session holding `capabilities`, with the frame's
 * commands answered; `commands` adds or replaces answers.
 */
export function renderApp(
  path: string,
  capabilities: readonly string[],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  const session: SessionInfo = { ...TEST_SESSION, capabilities: [...capabilities] };
  mockCommands({
    current_session: () => session,
    list_listener_status: () => [],
    list_work_queues: () => [],
    get_host_health: () => hostHealth(),
    get_recovery_reconciliation: () => null,
    get_laboratory_profile: () => LAB_PROFILE,
    app_info: () => ({ appVersion: "0.1.0", webviewVersion: null, platform: "linux-x86_64" }),
    ...commands,
  });
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  return { ...renderWithProviders(<RouterProvider router={router} />), router };
}
