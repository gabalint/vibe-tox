import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerContract } from "@/generated/PartnerContract";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  ADMIN,
  CONNECTION,
  CONNECTION_ID,
  DETAIL,
  MAPPINGS,
  NO_CONTRACT,
  TERMS,
} from "../../testData";
import { ConnectionPage } from "../ConnectionPage";

const ME = ADMIN.userId;
const AT = "2026-11-06T09:00:00.000000000Z";

const APPROVED: PartnerContract = {
  ...NO_CONTRACT,
  current: {
    version: 1,
    contract: TERMS,
    drafted: { by: ME, at: AT },
    approved: { by: ME, at: AT },
    reason: "Synthetic: terms confirmed",
    selfApproval: false,
  },
  draft: { contract: { ...TERMS, deviations: "No PV1" }, drafted: { by: ME, at: AT } },
  acceptance: "failed",
  lastRun: {
    ran: { by: ME, at: AT },
    passed: false,
    cases: [
      { code: "message_header", passed: true, problem: null },
      { code: "mappings_complete", passed: false, problem: "analyte_without_unit" },
    ],
  },
  allowedActions: [
    "draft_partner_contract",
    "approve_partner_contract",
    "run_partner_acceptance_tests",
  ],
};

function renderPage(
  contract: PartnerContract,
  handlers: Record<string, (args: unknown) => unknown>,
) {
  const detail: PartnerConnectionDetail = {
    ...DETAIL,
    connection: {
      ...CONNECTION,
      contract: {
        version: contract.current?.version ?? null,
        acceptance: contract.acceptance,
        testedAt: contract.lastRun?.ran.at ?? null,
      },
    },
    contract,
  };
  mockCommands({
    current_session: () => ADMIN,
    get_partner_connection: () => detail,
    list_partner_mappings: () => MAPPINGS,
    ...handlers,
  });
  return renderRoutes(
    [{ path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> }],
    `/interfaces/connections/${CONNECTION_ID}`,
  );
}

describe("Interface contract and acceptance tests (ticket #4555 step 5a; REQ-INT-001)", () => {
  beforeEach(mockGridLayout);

  it("says no patient data goes until a contract is approved, and drafts the first one", async () => {
    const drafted = vi.fn(() => DETAIL);
    const { user } = renderPage(NO_CONTRACT, { draft_partner_contract: drafted });
    const line = await screen.findByRole("button", {
      name: /^Not approved: no patient data goes or comes/,
    });
    await user.click(line);
    const dialog = await screen.findByRole("dialog", { name: "Interface contract" });
    expect(within(dialog).getByText("No version approved yet")).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: "Run acceptance tests" })).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Draft v1" }));
    const form = await screen.findByRole("dialog", { name: "Draft interface contract v1" });
    await user.type(
      within(form).getByRole("textbox", { name: "Message and profile" }),
      TERMS.profile,
    );
    await user.type(
      within(form).getByRole("textbox", { name: "Acknowledgements" }),
      TERMS.acknowledgements,
    );
    await user.type(
      within(form).getByRole("textbox", { name: "Corrections and withdrawals" }),
      TERMS.corrections,
    );
    await user.click(within(form).getByRole("button", { name: /Between the LIS and the partner/ }));
    await user.click(await screen.findByRole("option", { name: "The LIS itself" }));
    await user.type(
      within(form).getByRole("textbox", { name: "Who answers for it" }),
      TERMS.owners,
    );
    await user.click(within(form).getByRole("button", { name: "Save draft" }));
    await waitFor(() =>
      expect(drafted).toHaveBeenCalledWith({
        input: { connectionId: CONNECTION_ID, contract: TERMS },
      }),
    );
  });

  it("shows the version in force, the draft and why a test failed; runs the tests", async () => {
    const ran = vi.fn(() => DETAIL);
    const { user } = renderPage(APPROVED, { run_partner_acceptance_tests: ran });
    await user.click(await screen.findByRole("button", { name: /^v1 · acceptance tests failed/ }));
    const dialog = await screen.findByRole("dialog", { name: "Interface contract" });
    expect(within(dialog).getByText("In force: v1")).toBeTruthy();
    expect(within(dialog).getByText("Draft v2, waiting for approval")).toBeTruthy();
    expect(within(dialog).getByText("No PV1")).toBeTruthy();
    expect(
      within(dialog).getByText(
        "✗ Code mappings approved, each analyte with a UCUM unit: an analyte mapping has no UCUM unit",
      ),
    ).toBeTruthy();
    expect(within(dialog).getByText(/✓ Message header/)).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Run acceptance tests" }));
    await waitFor(() =>
      expect(ran).toHaveBeenCalledWith({ input: { connectionId: CONNECTION_ID } }),
    );
  });

  it("approves the draft as the next version with a reason", async () => {
    const approved = vi.fn(() => DETAIL);
    const { user } = renderPage(APPROVED, { approve_partner_contract: approved });
    await user.click(await screen.findByRole("button", { name: /^v1 · acceptance tests/ }));
    const dialog = await screen.findByRole("dialog", { name: "Interface contract" });
    await user.click(within(dialog).getByRole("button", { name: "Approve v2" }));
    const form = await screen.findByRole("dialog", { name: "Approve interface contract v2" });
    await user.type(within(form).getByRole("textbox", { name: "Reason" }), "Synthetic");
    await user.click(within(form).getByRole("button", { name: "Approve" }));
    await waitFor(() =>
      expect(approved).toHaveBeenCalledWith({
        input: {
          connectionId: CONNECTION_ID,
          expectedRevision: CONNECTION.revision,
          reason: "Synthetic",
          selfApproval: false,
        },
      }),
    );
  });
});
