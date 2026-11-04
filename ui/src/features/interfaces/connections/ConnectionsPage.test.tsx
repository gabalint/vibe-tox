import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION, CONNECTION_ID, DETAIL, SFTP, UNROUTED } from "../testData";
import { ConnectionPage } from "./ConnectionPage";
import { ConnectionsPage } from "./ConnectionsPage";

const ROUTES = [
  { path: "/interfaces/connections", element: <ConnectionsPage /> },
  { path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> },
];

function renderPage(handlers: Record<string, (args: unknown) => unknown>, path: string) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_unrouted_partner_messages: () => [UNROUTED],
    get_partner_connection: () => DETAIL,
    ...handlers,
  });
  return renderRoutes(ROUTES, path);
}

describe("Connections (ticket #4555; REQ-RPT-012)", () => {
  beforeEach(mockGridLayout);

  it("lists a connection with its status and destination check, and opens it", async () => {
    const { user, container, router } = renderPage({}, "/interfaces/connections");
    expect(await screen.findByRole("gridcell", { name: "/srv/billing/out" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Stopped" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Not verified" })).toBeTruthy();
    expect(screen.getByText("Unrouted messages")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRow(user, "Billing - RCM");
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/interfaces/connections/${CONNECTION_ID}`),
    );
  });

  it("lays out Add connection as the mockup: fixed direction, protocol cards, note and actions", async () => {
    const { user } = renderPage({}, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    expect(
      within(dialog).getByText("Where messages go or come from, and which ones."),
    ).toBeTruthy();
    const direction = within(dialog).getByRole("textbox", { name: "Direction" });
    expect((direction as HTMLInputElement).value).toBe("Out – LIS sends");
    expect(direction.hasAttribute("disabled")).toBe(true);
    const folder = within(dialog).getByRole("radio", { name: /Folder/ });
    expect((folder as HTMLInputElement).checked).toBe(true);
    expect(within(dialog).getByRole("radio", { name: /SFTP/ }).hasAttribute("disabled")).toBe(
      false,
    );
    expect(
      within(dialog)
        .getByRole("radio", { name: /MLLP – LIS connects/ })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(within(dialog).getByText(/can't send patient data/)).toBeTruthy();
    const actions = within(dialog)
      .getAllByRole("button")
      .slice(-3)
      .map((button) => button.textContent);
    expect(actions).toEqual(["Cancel", "Save as stopped", "Save"]);
  });

  it("fills Send when and the file name from the chosen message", async () => {
    const { user } = renderPage({}, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: "Report PDF" }));
    const fileName = within(dialog).getByRole("textbox", { name: "File name" });
    await waitFor(() =>
      expect((fileName as HTMLInputElement).value).toBe("{accession}_r{revision}_{date}.pdf"),
    );
    expect(within(dialog).getByRole("button", { name: /Report released/ })).toBeTruthy();
    expect(within(dialog).getByText(/Corrections and withdrawals go first/)).toBeTruthy();
  });

  it("sets up HL7 orders coming in: client, folders to read, and what to accept", async () => {
    const clients = [{ id: "c1", code: "CLINIC-A", name: "Clinic A", active: true }];
    const { user } = renderPage({ list_clients: () => clients }, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: /HL7 Orders/ }));
    const direction = within(dialog).getByRole("textbox", { name: "Direction" });
    await waitFor(() => expect((direction as HTMLInputElement).value).toBe("In – LIS receives"));
    await user.click(within(dialog).getByRole("button", { name: /Partner \(client\)/ }));
    expect(await screen.findByRole("option", { name: "CLINIC-A – Clinic A" })).toBeTruthy();
    await user.keyboard("{Escape}");
    for (const name of ["Input folder", "Processed folder", "Failed folder", "Check every"]) {
      expect(within(dialog).getByRole("textbox", { name })).toBeTruthy();
    }
    expect(within(dialog).getByText("4. What to accept")).toBeTruthy();
    expect(within(dialog).getByRole("textbox", { name: "Accept messages from" })).toBeTruthy();
    expect(within(dialog).queryByText(/Routing rules/)).toBeNull();
    expect(within(dialog).queryByText("5. When to send")).toBeNull();
    expect(within(dialog).getByText(/reads no orders until its destination check/)).toBeTruthy();
  });

  it("shows an incoming connection's folders and what it accepts", async () => {
    const orders = {
      ...CONNECTION,
      // The engine offers no test message to a connection that only reads.
      allowedActions: CONNECTION.allowedActions.filter(
        (action) => action !== "send_partner_test_message",
      ),
      processedFolder: "/srv/clinic-a/processed",
      failedFolder: "/srv/clinic-a/failed",
      settings: {
        ...CONNECTION.settings,
        partner: "CLINIC-A",
        messageType: "hl7_orders" as const,
        rules: [],
        fileNamePattern: "",
        intake: { checkEverySeconds: 30, acceptFrom: "CLINIC-A", unknownPatient: "hold" as const },
      },
    };
    renderPage(
      { get_partner_connection: () => ({ ...DETAIL, connection: orders }) },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText("Reads HL7 order messages from CLINIC-A's folder every 30 seconds."),
    ).toBeTruthy();
    expect(screen.getByText("/srv/clinic-a/processed")).toBeTruthy();
    expect(screen.getByText("MSH-4 = CLINIC-A")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Send test message" })).toBeNull();
  });

  it("offers the SFTP server fields of mockup 6 and tests the server as entered", async () => {
    const tested = vi.fn(() => ({ problem: null, fileName: null, presentedFingerprint: null }));
    const { user } = renderPage({ test_partner_sftp: tested }, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("radio", { name: /SFTP/ }));
    expect(within(dialog).getByText("3. SFTP server")).toBeTruthy();
    const port = within(dialog).getByRole("textbox", { name: "Port" }) as HTMLInputElement;
    expect(port.value).toBe("22");
    expect(within(dialog).getByRole("button", { name: /SSH key \(recommended\)/ })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Copy LIS public key" })).toBeTruthy();
    await user.type(within(dialog).getByRole("textbox", { name: "Server" }), SFTP.host);
    await user.type(within(dialog).getByRole("textbox", { name: "Username" }), SFTP.username);
    await user.type(
      within(dialog).getByRole("textbox", { name: "Server fingerprint" }),
      SFTP.fingerprint,
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: "Remote folder" }),
      SFTP.remoteFolder,
    );
    await user.click(within(dialog).getByRole("button", { name: "Test connection" }));
    await waitFor(() => expect(tested).toHaveBeenCalledWith({ input: { server: SFTP } }));
    expect(
      await within(dialog).findByText(
        "✓ Connected, fingerprint matches, test file written and removed.",
      ),
    ).toBeTruthy();
  });

  it("shows an SFTP connection's server, remote folder and sign-in as the mockup", async () => {
    const uploads = {
      ...CONNECTION,
      settings: { ...CONNECTION.settings, protocol: "sftp" as const, sftp: SFTP },
    };
    renderPage(
      { get_partner_connection: () => ({ ...DETAIL, connection: uploads }) },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText(
        "Uploads the billing handoff file to Synthetic billing company's SFTP server when a billing handoff is created.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("SFTP – sftp.clinic-a.example:22")).toBeTruthy();
    expect(screen.getByText("/inbound/results")).toBeTruthy();
    expect(
      screen.getByText("SSH key as vibetox_lab · fingerprint SHA256:8Zq3...f1Kx"),
    ).toBeTruthy();
  });

  it("explains an empty list", async () => {
    renderPage({ list_partner_connections: () => [] }, "/interfaces/connections");
    expect(await screen.findByText("No connections")).toBeTruthy();
  });

  it("shows the engine's refusal with its reference", async () => {
    renderPage(
      {
        list_partner_connections: () =>
          Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
      },
      "/interfaces/connections",
    );
    expect(await screen.findByText(/You do not have permission/)).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
  });

  it("offers the destination check first while it is missing, and records it", async () => {
    const record = vi.fn(() => ({
      ...DETAIL,
      connection: { ...CONNECTION, verification: null, revision: 2 },
    }));
    const { user } = renderPage(
      { record_partner_destination_check: record },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    const check = await screen.findByRole("button", { name: "Record destination check" });
    expect(screen.getByRole("button", { name: "Edit" })).toBeTruthy();
    await user.click(check);
    const dialog = await screen.findByRole("dialog", { name: "Record destination check" });
    expect(within(dialog).getByText(/business associate agreement/)).toBeTruthy();
    await user.type(
      within(dialog).getByRole("textbox", { name: /How it was confirmed/ }),
      "Synthetic call-back",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record check" }));
    await waitFor(() =>
      expect(record).toHaveBeenCalledWith({
        input: {
          connectionId: CONNECTION_ID,
          expectedRevision: 1,
          method: "Synthetic call-back",
          selfApproval: false,
        },
      }),
    );
  });

  it("lays out a connection's page as the mockup: action row, five tabs, two cards", async () => {
    renderPage({}, `/interfaces/connections/${CONNECTION_ID}`);
    expect(await screen.findByRole("region", { name: "Checks and contract" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Connection" })).toBeTruthy();
    const tabs = screen.getAllByRole("tab").map((tab) => tab.textContent);
    expect(tabs).toEqual(["Overview", "Rules", "Code mappings", "Logs", "History"]);
    for (const name of [
      "Edit",
      "Start",
      "Test connection",
      "Send test message",
      "Record destination check",
    ]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
  });

  it("describes a Report PDF connection by what it writes and when", async () => {
    const pdfs = {
      ...CONNECTION,
      settings: { ...CONNECTION.settings, partner: "CLINIC-A", messageType: "report_pdf" as const },
    };
    renderPage(
      { get_partner_connection: () => ({ ...DETAIL, connection: pdfs }) },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText(
        "Writes the issued report PDF to CLINIC-A's folder when a report revision is released.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("Report PDF (the issued PDF, as released)")).toBeTruthy();
    expect(screen.getByText("Report released · immediately")).toBeTruthy();
  });

  it("starts a stopped connection with a reason", async () => {
    const setActive = vi.fn(() => DETAIL);
    const { user } = renderPage(
      { set_partner_connection_active: setActive },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    await user.click(await screen.findByRole("button", { name: "Start" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Start connection" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Partner ready");
    await user.click(within(dialog).getByRole("button", { name: "Start" }));
    await waitFor(() =>
      expect(setActive).toHaveBeenCalledWith({
        input: {
          connectionId: CONNECTION_ID,
          expectedRevision: 1,
          active: true,
          reason: "Partner ready",
        },
      }),
    );
  });
});
