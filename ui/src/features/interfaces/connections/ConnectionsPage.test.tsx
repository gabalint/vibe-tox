import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION, CONNECTION_ID, DETAIL, UNROUTED } from "../testData";
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
    expect(within(dialog).getByRole("radio", { name: /SFTP/ }).hasAttribute("disabled")).toBe(true);
    expect(within(dialog).getByText(/can't send patient data/)).toBeTruthy();
    const actions = within(dialog)
      .getAllByRole("button")
      .slice(-3)
      .map((button) => button.textContent);
    expect(actions).toEqual(["Cancel", "Save as stopped", "Save"]);
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
