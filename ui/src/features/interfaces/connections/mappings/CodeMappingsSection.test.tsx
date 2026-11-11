import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION_ID, DETAIL, HL7, MAPPINGS, PHENTERMINE } from "../../testData";
import { ConnectionPage } from "../ConnectionPage";

const ROUTES = [
  { path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> },
];

function renderTab(handlers: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => ADMIN,
    get_partner_connection: () => ({ ...DETAIL, connection: HL7 }),
    list_partner_mappings: () => MAPPINGS,
    list_partner_messages: () => ({ rows: [], more: false }),
    list_analytes: () => [
      { id: PHENTERMINE, code: "SYN-PHEN", name: "Phentermine", active: true, revision: 1 },
    ],
    list_orderables: () => [],
    ...handlers,
  });
  return renderRoutes(ROUTES, `/interfaces/connections/${CONNECTION_ID}/mappings`);
}

describe("Code mappings of an HL7 connection (ticket #4555; REQ-INT-004)", () => {
  beforeEach(mockGridLayout);

  it("shows approved and held mappings as the mockup, with the catalog's LOINC", async () => {
    const { container } = renderTab();
    expect(await screen.findByRole("gridcell", { name: "Approved v2" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Not mapped – held" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "from catalog" })).toBeTruthy();
    expect(
      screen.getByText(/Unmapped results are held, never sent with a guessed code/),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve mappings" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("maps a held analyte as a draft", async () => {
    const draft = vi.fn(() => MAPPINGS);
    const { user } = renderTab({ draft_partner_mapping: draft });
    await screen.findByRole("gridcell", { name: "Not mapped – held" });
    await openRow(user, "Phentermine");
    const dialog = await screen.findByRole("dialog", { name: "Map Phentermine" });
    await user.type(within(dialog).getByRole("textbox", { name: /Partner code/ }), "CA-PHEN");
    await user.type(within(dialog).getByRole("textbox", { name: /Unit \(UCUM\)/ }), "ng/mL");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    await waitFor(() =>
      expect(draft).toHaveBeenCalledWith({
        input: {
          connectionId: CONNECTION_ID,
          kind: "analyte",
          lisId: PHENTERMINE,
          mapping: { partnerCode: "CA-PHEN", ucumUnit: "ng/mL" },
        },
      }),
    );
  });

  it("offers approval while drafts wait", async () => {
    renderTab({
      list_partner_mappings: () => ({
        ...MAPPINGS,
        allowedActions: ["draft_partner_mapping", "approve_partner_mappings"],
      }),
    });
    expect(await screen.findByRole("button", { name: "Approve mappings" })).toBeTruthy();
  });
});
