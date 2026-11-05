import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  ADMIN,
  CONNECTION,
  CONNECTION_ID,
  DETAIL,
  HL7_MLLP,
  MAPPINGS,
  MLLP,
  UNROUTED,
} from "../testData";
import { ConnectionPage } from "./ConnectionPage";
import { ConnectionsPage } from "./ConnectionsPage";
import { toSettings, toValues } from "./connectionForm";

const ROUTES = [
  { path: "/interfaces/connections", element: <ConnectionsPage /> },
  { path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> },
];

const NOTHING_FOUND = { problem: null, fileName: null, presentedFingerprint: null };

function renderPage(handlers: Record<string, (args: unknown) => unknown>, path: string) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_unrouted_partner_messages: () => [UNROUTED],
    get_partner_connection: () => ({ ...DETAIL, connection: HL7_MLLP }),
    list_partner_mappings: () => MAPPINGS,
    ...handlers,
  });
  return renderRoutes(ROUTES, path);
}

describe("MLLP – LIS connects (ticket #4555 step 4; REQ-INT-002, REQ-INT-006)", () => {
  beforeEach(mockGridLayout);

  it("offers MLLP for HL7 results: partner address, acknowledgement, the lab-network note and a test", async () => {
    const tested = vi.fn(() => NOTHING_FOUND);
    const { user } = renderPage({ test_partner_mllp: tested }, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: /HL7 Results/ }));
    await user.click(within(dialog).getByRole("radio", { name: /MLLP – LIS connects/ }));
    expect(within(dialog).getByText("3. MLLP partner")).toBeTruthy();
    expect(within(dialog).getByText(/Lab network only/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: /Application ACK/ })).toBeTruthy();
    // An MLLP message travels as it is: no file name to set.
    expect(within(dialog).queryByRole("textbox", { name: "File name" })).toBeNull();
    await user.type(within(dialog).getByRole("textbox", { name: "Partner address" }), MLLP.address);
    await user.type(within(dialog).getByRole("textbox", { name: "Port" }), String(MLLP.port));
    await user.click(within(dialog).getByRole("button", { name: "Test connection" }));
    await waitFor(() => expect(tested).toHaveBeenCalledWith({ input: { partner: MLLP } }));
    expect(
      await within(dialog).findByText("✓ 10.20.0.15:6661 takes a connection; nothing was sent."),
    ).toBeTruthy();
  });

  it("does not offer MLLP for a message other than HL7 results", async () => {
    const { user } = renderPage({}, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: "Report PDF" }));
    const mllp = within(dialog).getByRole("radio", { name: /MLLP – LIS connects/ });
    expect(mllp.hasAttribute("disabled")).toBe(true);
  });

  it("saves the partner and no file name", () => {
    const values = toValues(HL7_MLLP);
    expect(values.mllp).toEqual({
      address: "10.20.0.15",
      port: "6661",
      acknowledgement: "application",
      ackTimeoutSeconds: "30",
    });
    const settings = toSettings({ ...values, fileNamePattern: "left-over.hl7" });
    expect(settings.mllp).toEqual(MLLP);
    expect(settings.fileNamePattern).toBe("");
    expect(settings.sftp).toBeNull();
  });

  it("shows the partner, the acknowledgements awaited and what the tests found", async () => {
    const sent = vi.fn(() => ({
      ...NOTHING_FOUND,
      ackCode: "AE",
      ackText: "Unknown provider NPI 1234567893",
    }));
    const { user } = renderPage(
      { test_partner_folder: () => NOTHING_FOUND, send_partner_test_message: sent },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText(
        "Sends an HL7 ORU result message with the partner's codes to CLINIC-B's interface over MLLP when results are released.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("MLLP – LIS connects – 10.20.0.15:6661")).toBeTruthy();
    expect(screen.getByText("Application ACK (AA / AE / AR) · timeout 30 seconds")).toBeTruthy();
    expect(screen.getByText("2 messages")).toBeTruthy();
    expect(screen.queryByText("File name")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Test connection" }));
    expect(
      await screen.findByText("✓ The partner's interface takes a connection; nothing was sent."),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Send test message" }));
    expect(
      await screen.findByText(
        "✗ Synthetic test message sent; the partner found an error (AE): Unknown provider NPI 1234567893",
      ),
    ).toBeTruthy();
  });
});
