import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ConnectionPage } from "../connections/ConnectionPage";
import { ADMIN, CONNECTION, CONNECTION_ID, DETAIL, MAPPINGS, WRITTEN } from "../testData";
import { LogsPage } from "./LogsPage";

const HELD: PartnerMessageRow = {
  ...WRITTEN,
  transport: "not_sent",
  outcomeUnknown: false,
  waiting: "tests_out_of_date",
  writtenPath: null,
};

const ROUTES = [
  { path: "/interfaces/logs/:tab?", element: <LogsPage /> },
  { path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> },
];

function renderAt(path: string, rows: PartnerMessageRow[]) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_partner_messages: () => ({ rows, more: false }),
    get_partner_connection: () => DETAIL,
    list_partner_mappings: () => MAPPINGS,
  });
  return renderRoutes(ROUTES, path);
}

describe("Messages held by the interface contract (ticket #4726)", () => {
  beforeEach(mockGridLayout);

  it("says which check holds the message and links to the connection's contract", async () => {
    const notApproved = { ...HELD, deliveryId: "d-2", waiting: "contract_not_approved" as const };
    renderAt("/interfaces/logs", [HELD, notApproved]);
    expect(
      await screen.findByText(
        /Held: acceptance tests out of date after a change to the connection; run them again/,
      ),
    ).toBeTruthy();
    expect(screen.getByText(/Held: interface contract not approved/)).toBeTruthy();
    const links = screen.getAllByRole("link", { name: "Open interface contract" });
    expect(links).toHaveLength(2);
    expect(links[0]?.getAttribute("href")).toBe(
      `/interfaces/connections/${CONNECTION_ID}?contract=open`,
    );
  });

  it("links no contract for a message held for another reason", async () => {
    renderAt("/interfaces/logs", [{ ...HELD, waiting: "stopped" }]);
    expect(await screen.findByText("Connection stopped")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Open interface contract" })).toBeNull();
  });

  it("opens the contract when the link's address asks for it", async () => {
    renderAt(`/interfaces/connections/${CONNECTION_ID}?contract=open`, []);
    expect(await screen.findByRole("dialog", { name: "Interface contract" })).toBeTruthy();
  });
});
