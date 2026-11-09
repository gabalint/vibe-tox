import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
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

describe("Logs rows (tickets #4680, #4679, #4682)", () => {
  beforeEach(mockGridLayout);

  it("links the record to the revision sent or the handoff; a test message has no link", async () => {
    renderPage(() => ({ rows: [CORRECTION, WRITTEN, TEST_MESSAGE], more: false }));
    const revision = await screen.findByRole("link", { name: "A26000005-U r2" });
    expect(revision.getAttribute("href")).toBe(`/reports/revisions/${CORRECTION.sourceId}`);
    const handoff = screen.getByRole("link", { name: "Billing handoff 12" });
    expect(handoff.getAttribute("href")).toBe(`/billing/handoffs/${WRITTEN.sourceId}`);
    expect(screen.queryByRole("link", { name: /Test/ })).toBeNull();
  });
});
