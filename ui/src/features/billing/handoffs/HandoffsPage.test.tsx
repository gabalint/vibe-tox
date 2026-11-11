import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BillingHandoffSummary } from "@/generated/BillingHandoffSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { HANDOFF, HANDOFF_ID, sessionWith } from "../testData";
import { HandoffPage } from "./HandoffPage";
import { HandoffsPage } from "./HandoffsPage";

const ACKNOWLEDGED: BillingHandoffSummary = {
  handoffId: HANDOFF_ID,
  handoffNo: 7,
  createdAt: "2026-10-23T10:39:00Z",
  itemCount: 2,
  outstandingCount: 0,
  rejectedCount: 0,
  acknowledgedCount: 2,
  accessionNumbers: ["A26000004", "A26000005"],
  datesOfService: ["2026-10-20"],
  hasMore: false,
};

const ROUTES = [
  { path: "/billing/handoffs", element: <HandoffsPage /> },
  { path: "/billing/handoffs/:handoffId", element: <HandoffPage /> },
];

type Input = { input: { search: string | null; beforeHandoffNo: number | null } };

function renderPage(list: (args: unknown) => unknown, path = "/billing/handoffs") {
  mockCommands({
    current_session: () => sessionWith(["billing"]),
    list_billing_handoffs: list,
    get_billing_handoff: () => HANDOFF,
  });
  return renderRoutes(ROUTES, path);
}

describe("HandoffsPage (REQ-BIL-002)", () => {
  beforeEach(mockGridLayout);

  it("lists an acknowledged handoff and opens it with Billing handoffs as its home", async () => {
    const list = vi.fn(() => [ACKNOWLEDGED]);
    const { user, container, router } = renderPage(list);
    expect(await screen.findByRole("gridcell", { name: "A26000004, A26000005" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Acknowledged" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "2026-10-20" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: { limit: 200, search: null, beforeHandoffNo: null },
    });
    await expectNoA11yViolations(container, { wholePage: true });
    await openRow(user, "7");
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/billing/handoffs/${HANDOFF_ID}`),
    );
    expect(await screen.findByRole("link", { name: "Billing handoffs" })).toBeTruthy();
  });

  it("asks the engine for the find text and offers to show all when nothing matches", async () => {
    const list = vi.fn((args: unknown) => ((args as Input).input.search ? [] : [ACKNOWLEDGED]));
    const { user } = renderPage(list);
    await user.type(
      await screen.findByRole("searchbox", { name: /Find by handoff number/ }),
      "A26000004{Enter}",
    );
    expect(await screen.findByText("No handoff matches")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: { limit: 200, search: "A26000004", beforeHandoffNo: null },
    });
    await user.click(screen.getByRole("button", { name: "Show all handoffs" }));
    expect(await screen.findByRole("gridcell", { name: "A26000004, A26000005" })).toBeTruthy();
  });

  it("loads older handoffs from the last handoff number", async () => {
    const newer = { ...ACKNOWLEDGED, handoffId: "handoff-8", handoffNo: 8, hasMore: true };
    const list = vi.fn((args: unknown) =>
      (args as Input).input.beforeHandoffNo === 8 ? [ACKNOWLEDGED] : [newer],
    );
    const { user } = renderPage(list);
    await user.click(await screen.findByRole("button", { name: "Load older handoffs" }));
    expect(await screen.findByText(/2 handoffs/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load older handoffs" })).toBeNull();
  });

  it("shows the item needing attention first as the handoff's status", async () => {
    renderPage(() => [{ ...ACKNOWLEDGED, rejectedCount: 1, acknowledgedCount: 1 }]);
    expect(await screen.findByRole("gridcell", { name: "Rejected" })).toBeTruthy();
  });
});
