import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION, CORRECTION, WRITTEN } from "../testData";
import { LogsPage } from "./LogsPage";

const ROUTES = [{ path: "/interfaces/logs/:tab?", element: <LogsPage /> }];

const TEST_MESSAGE = {
  ...WRITTEN,
  messageId: "01a1a000-0000-7000-8000-000000000050",
  deliveryId: "01a1a000-0000-7000-8000-000000000051",
  messageType: "hl7_oru" as const,
  source: "test" as const,
  sourceId: null,
  sourceNumber: null,
  reason: "test" as const,
};

function renderPage(list: (args: unknown) => unknown, handlers = {}) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_partner_messages: list,
    ...handlers,
  });
  return renderRoutes(ROUTES, "/interfaces/logs");
}

describe("Logs rows (tickets #4680, #4679, #4682, #4723)", () => {
  beforeEach(mockGridLayout);

  it("links the record to the revision sent or the handoff; a test message has no link", async () => {
    renderPage(() => ({ rows: [CORRECTION, WRITTEN, TEST_MESSAGE], more: false }));
    const revision = await screen.findByRole("link", { name: "A26000005-U r2" });
    expect(revision.getAttribute("href")).toBe(`/reports/revisions/${CORRECTION.sourceId}`);
    const handoff = screen.getByRole("link", { name: "Billing handoff 12" });
    expect(handoff.getAttribute("href")).toBe(`/billing/handoffs/${WRITTEN.sourceId}`);
    expect(screen.queryByRole("link", { name: /Test/ })).toBeNull();
  });

  it("warns only on an unrouted message still waiting; a resolved one says how", async () => {
    const unrouted = {
      ...CORRECTION,
      deliveryId: null,
      connection: null,
      reason: null,
      transport: "not_sent" as const,
      outcomeUnknown: false,
      writtenPath: null,
      unrouted: true,
    };
    const resolved = {
      ...unrouted,
      messageId: "01a1a000-0000-7000-8000-000000000060",
      resolution: "delivered_another_way" as const,
    };
    renderPage(() => ({ rows: [unrouted, resolved], more: false }));
    expect(
      await screen.findByRole("gridcell", { name: "Resolved – recorded another way" }),
    ).toBeTruthy();
    const tones = screen
      .getAllByRole("gridcell", { name: "Not sent" })
      .map((cell) => cell.querySelector("[data-tone]")?.getAttribute("data-tone"));
    expect(tones.sort()).toEqual(["neutral", "warning"]);
    expect(screen.getByRole("gridcell", { name: "Delivery recorded another way" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "No connection rule matched; waiting in Unrouted messages",
      }),
    ).toBeTruthy();
  });

  it("hides test messages in the engine until Show test messages, then badges them", async () => {
    const list = vi.fn((args: unknown) => {
      const { input } = args as { input: { hideTests?: boolean } };
      return { rows: input.hideTests ? [WRITTEN] : [WRITTEN, TEST_MESSAGE], more: false };
    });
    const { user } = renderPage(list);
    expect(await screen.findByRole("tab", { name: "All (1)" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "Test" })).toBeNull();
    await user.click(screen.getByRole("checkbox", { name: "Show test messages" }));
    expect(await screen.findByRole("tab", { name: "All (2)" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Test" })).toBeTruthy();
    const [args] = list.mock.calls.at(-1) as unknown as [{ input: { hideTests: boolean } }];
    expect(args.input.hideTests).toBe(false);
  });

  it("exports only the rows checked, and the rows shown when none is", async () => {
    const exported = vi.fn(() => ({ fileName: "interface-log.csv", folder: "downloads", rows: 2 }));
    const { user } = renderPage(() => ({ rows: [WRITTEN, CORRECTION], more: false }), {
      export_partner_messages: exported,
    });
    await user.click(await screen.findByRole("button", { name: "Export CSV" }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(1));
    const [all] = exported.mock.calls.at(-1) as unknown as [{ input: { rowIds?: string[] } }];
    expect(all.input.rowIds).toBeUndefined();
    await user.click(screen.getByRole("columnheader", { name: "Select all rows" }));
    await user.click(await screen.findByRole("button", { name: "Export 2 selected" }));
    await waitFor(() => expect(exported).toHaveBeenCalledTimes(2));
    const [checked] = exported.mock.calls.at(-1) as unknown as [{ input: { rowIds: string[] } }];
    expect(checked.input.rowIds.sort()).toEqual([WRITTEN.deliveryId, CORRECTION.deliveryId].sort());
  });

  it("passes several comma-separated values to the engine's search", async () => {
    const list = vi.fn(() => ({ rows: [CORRECTION], more: false }));
    const { user } = renderPage(list);
    await user.type(
      await screen.findByRole("searchbox", { name: /Search by report number/ }),
      "A26000005, A26000009",
    );
    await waitFor(() => {
      const [args] = list.mock.calls.at(-1) as unknown as [{ input: { search?: string } }];
      expect(args.input.search).toBe("A26000005, A26000009");
    });
  });
});
