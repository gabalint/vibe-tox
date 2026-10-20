import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BillingReviewRecord } from "@/generated/BillingReviewRecord";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ORDER_ID } from "../testData";
import { BillingReviewPage } from "./BillingReviewPage";

const ROWS: BillingReviewRecord[] = Array.from({ length: 50 }, (_, i) => ({
  id: `review-${i}`,
  orderId: ORDER_ID,
  orderReference: "SYN-ORDER",
  element: "other",
  note: null,
  openedAt: "2026-10-14T12:00:00Z",
  openedBy: "synthetic-user",
  version: 1,
  status: "open",
  owner: null,
  sourceRef: null,
  reason: null,
  updatedAt: null,
  updatedBy: null,
}));

describe("Billing follow-up queue (BILL-G04)", () => {
  beforeEach(mockGridLayout);
  it("keeps filters and cursor in the URL and opens scoped order billing", async () => {
    const list = vi.fn(({ input }: { input: { afterItemId: string | null; status: string } }) =>
      input.afterItemId || input.status === "resolved" ? [] : ROWS,
    );
    mockCommands({ list_billing_review_items: (args) => list(args as Parameters<typeof list>[0]) });
    const { user, router } = renderRoutes(
      [{ path: "/billing/review", element: <BillingReviewPage /> }],
      "/billing/review",
    );
    await screen.findByRole("grid", { name: "Billing follow-up" });
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(await screen.findByText("No open billing follow-up on this page")).toBeTruthy();
    expect(router.state.location.search).toBe("?status=open&after=review-49");
    expect(list).toHaveBeenLastCalledWith({
      input: { status: "open", limit: 50, afterItemId: "review-49" },
    });
    await user.click(screen.getByRole("button", { name: "Resolved follow-up" }));
    expect(await screen.findByText("No resolved billing follow-up on this page")).toBeTruthy();
    expect(router.state.location.search).toBe("?status=resolved");
    await user.click(screen.getByRole("button", { name: "Open follow-up" }));
    const links = await screen.findAllByRole("link", {
      name: "Order SYN-ORDER · Other financial evidence",
    });
    const first = links[0];
    expect(first).toBeDefined();
    if (first) await user.click(first);
    expect(await screen.findByText(`Navigated to /billing/orders/${ORDER_ID}`)).toBeTruthy();
  });
});
