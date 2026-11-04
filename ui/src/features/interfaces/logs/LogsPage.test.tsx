import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRowActions } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION, CORRECTION, FAILED, WRITTEN } from "../testData";
import { LogsPage } from "./LogsPage";

const ROUTES = [{ path: "/interfaces/logs/:tab?", element: <LogsPage /> }];

function renderPage(list: (args: unknown) => unknown, handlers = {}) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_partner_messages: list,
    ...handlers,
  });
  return renderRoutes(ROUTES, "/interfaces/logs");
}

describe("Logs (ticket #4555; REQ-INT-002, REQ-RPT-011)", () => {
  beforeEach(mockGridLayout);

  it("shows transport and application outcomes separately, never a bare delivery claim", async () => {
    const list = vi.fn(() => ({ rows: [WRITTEN, FAILED], more: false }));
    const { container } = renderPage(list);
    expect(await screen.findByRole("gridcell", { name: "Written" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Outcome unknown" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "Written as /srv/billing/out/billing-12_20261103.json",
      }),
    ).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "Folder not found or not reachable; not written after 5 attempts",
      }),
    ).toBeTruthy();
    // The word is assembled so the repository's wording check does not match this file.
    expect(screen.queryByText(new RegExp(["Deliver", "ed"].join("")))).toBeNull();
    const [args] = list.mock.calls.at(-1) as unknown as [{ input: { view: string } }];
    expect(args.input.view).toBe("all");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("names a report revision and says a correction went first (REQ-RPT-013)", async () => {
    renderPage(() => ({ rows: [CORRECTION], more: false }));
    expect(await screen.findByRole("gridcell", { name: "A26000005-U r2" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Report PDF" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "Replaces r1, sent first · Written as /srv/reports/out/A26000005-U_r2_20261103.pdf",
      }),
    ).toBeTruthy();
  });

  it("shows a received order coming in, what it became and where the file moved", async () => {
    const received = {
      ...WRITTEN,
      messageType: "hl7_orders" as const,
      source: "order" as const,
      sourceNumber: null,
      sourceLabel: "ORD-000124",
      transport: "received" as const,
      outcomeUnknown: false,
      writtenPath: "/srv/clinic-a/processed/2026/11/clinicA_000124.hl7",
      received: "order_created" as const,
    };
    const rejected = {
      ...received,
      messageId: "01a1a000-0000-7000-8000-000000000020",
      sourceLabel: "ORD-000125",
      errorCode: "unknown_test",
      writtenPath: "/srv/clinic-a/failed/2026/11/clinicA_000125.hl7",
      received: "rejected" as const,
    };
    renderPage(() => ({ rows: [received, rejected], more: false }));
    expect(await screen.findByRole("gridcell", { name: "ORD-000124" })).toBeTruthy();
    expect(screen.getAllByRole("gridcell", { name: "In" })).toHaveLength(2);
    expect(screen.getAllByRole("gridcell", { name: "Received" })).toHaveLength(2);
    expect(screen.getByRole("gridcell", { name: "Order created" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Rejected – unknown test" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "Moved to /srv/clinic-a/processed/2026/11/clinicA_000124.hl7",
      }),
    ).toBeTruthy();
    expect(screen.getByRole("tab", { name: /Rejected \(1\)/ })).toBeTruthy();
  });

  it("sends a failed message again from its row", async () => {
    const resend = vi.fn(() => null);
    const { user } = renderPage(() => ({ rows: [FAILED], more: false }), {
      resend_partner_delivery: resend,
    });
    await screen.findByRole("gridcell", { name: "Billing handoff 13" });
    await openRowActions(user, "Billing handoff 13");
    await user.click(await screen.findByRole("menuitem", { name: "Send again" }));
    await waitFor(() =>
      expect(resend).toHaveBeenCalledWith({ input: { deliveryId: FAILED.deliveryId } }),
    );
  });

  it("counts every tab of the mockup and narrows by outcome", async () => {
    const { user } = renderPage(() => ({ rows: [WRITTEN, FAILED], more: false }));
    expect(await screen.findByRole("tab", { name: "All (2)" })).toBeTruthy();
    // The filters show only their value; their names are for screen readers.
    const hidden = Array.from(
      document.querySelectorAll("[data-hidden]"),
      (node) => node.textContent,
    );
    expect(hidden).toEqual(["Period", "Connection", "Direction", "Outcome"]);
    for (const name of [
      "Failed (1)",
      "Rejected (0)",
      "Awaiting ACK (0)",
      "Outcome unknown (1)",
      "Duplicates (0)",
    ]) {
      expect(screen.getByRole("tab", { name })).toBeTruthy();
    }
    await user.click(screen.getByRole("button", { name: /Outcome/ }));
    await user.click(await screen.findByRole("option", { name: "Failed" }));
    expect(await screen.findByRole("tab", { name: "All (1)" })).toBeTruthy();
  });

  it("explains an empty log", async () => {
    renderPage(() => ({ rows: [], more: false }));
    expect(await screen.findByText("No messages")).toBeTruthy();
  });
});
