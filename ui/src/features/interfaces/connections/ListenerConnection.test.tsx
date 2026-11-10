import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { receivedDetail } from "../intakeLabels";
import {
  ADMIN,
  CONNECTION_ID,
  DETAIL,
  LISTENER,
  MAPPINGS,
  ORDERS_LISTEN,
  UNROUTED,
  WRITTEN,
} from "../testData";
import { ConnectionPage } from "./ConnectionPage";
import { ConnectionsPage } from "./ConnectionsPage";
import { toSettings, toValues } from "./connectionForm";
import { toPartners } from "./listenerForm";

const ROUTES = [
  { path: "/interfaces/connections", element: <ConnectionsPage /> },
  { path: "/interfaces/connections/:connectionId/:tab?", element: <ConnectionPage /> },
];

const NOTHING_FOUND = { problem: null, fileName: null, presentedFingerprint: null };
const CLIENTS = [{ id: "c1", code: "CLINIC-B", name: "Clinic B", active: true }];

function renderPage(handlers: Record<string, (args: unknown) => unknown>, path: string) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [ORDERS_LISTEN],
    list_unrouted_partner_messages: () => [UNROUTED],
    list_clients: () => CLIENTS,
    get_partner_connection: () => ({ ...DETAIL, connection: ORDERS_LISTEN }),
    list_partner_mappings: () => MAPPINGS,
    ...handlers,
  });
  return renderRoutes(ROUTES, path);
}

describe("MLLP – LIS listens (ticket #4555 step 4b; REQ-INT-002, REQ-INT-006)", () => {
  beforeEach(mockGridLayout);

  it("offers the listener of mockup 2 for HL7 orders and tests the port as entered", async () => {
    const tested = vi.fn(() => NOTHING_FOUND);
    const { user } = renderPage({ test_partner_mllp: tested }, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: /HL7 Orders/ }));
    await user.click(within(dialog).getByRole("radio", { name: /MLLP – LIS listens/ }));
    expect(within(dialog).getByText("3. MLLP listener")).toBeTruthy();
    expect(within(dialog).getByText(/The LIS never listens on an internet address/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: /UTF-8/ })).toBeTruthy();
    // A listener receives each message as it comes: no folders to read.
    expect(within(dialog).queryByRole("textbox", { name: "Input folder" })).toBeNull();
    expect(within(dialog).queryByRole("textbox", { name: "Check every" })).toBeNull();
    expect(within(dialog).getAllByText("Reject with AE and log").length).toBeGreaterThan(0);
    await user.type(
      within(dialog).getByRole("textbox", { name: "Listen on address" }),
      LISTENER.address,
    );
    await user.type(within(dialog).getByRole("textbox", { name: "Port" }), "6660");
    await user.type(
      within(dialog).getByRole("textbox", { name: "Partner addresses allowed to connect" }),
      "192.168.1.50 (Hospital B interface engine)",
    );
    await user.click(within(dialog).getByRole("button", { name: "Test listener" }));
    await waitFor(() => expect(tested).toHaveBeenCalledWith({ input: { listener: LISTENER } }));
    expect(await within(dialog).findByText("✓ Port 6660 free on 192.168.1.20.")).toBeTruthy();
  });

  it("does not offer listening for a message other than HL7 orders", async () => {
    const { user } = renderPage({}, "/interfaces/connections");
    await user.click(await screen.findByRole("button", { name: "Add connection" }));
    const dialog = await screen.findByRole("dialog", { name: "Add connection" });
    await user.click(within(dialog).getByRole("button", { name: /Message/ }));
    await user.click(await screen.findByRole("option", { name: /HL7 Results/ }));
    const listen = within(dialog).getByRole("radio", { name: /MLLP – LIS listens/ });
    expect(listen.hasAttribute("disabled")).toBe(true);
  });

  it("saves the listener with its partners, and no folder", () => {
    const values = toValues(ORDERS_LISTEN);
    expect(values.mllpListener.allowedPartners).toBe("192.168.1.50 (Hospital B interface engine)");
    const settings = toSettings(values);
    expect(settings.mllpListener).toEqual(LISTENER);
    expect(settings.mllp).toBeNull();
    expect(settings.sftp).toBeNull();
    expect(toPartners("192.168.1.50 ( EMR ), 192.168.1.51,fd00::7")).toEqual([
      { address: "192.168.1.50", note: "EMR" },
      { address: "192.168.1.51", note: "" },
      { address: "fd00::7", note: "" },
    ]);
  });

  it("logs the acknowledgement each received message got, and to whom", () => {
    const received = {
      ...WRITTEN,
      transport: "received" as const,
      received: "waiting_patient" as const,
      errorCode: "patient_unknown",
      writtenPath: null,
      ackCode: "AA" as const,
      peer: "192.168.1.50",
    };
    expect(receivedDetail(received)).toBe(
      "No registered patient has the message's identifier · ACK AA returned to 192.168.1.50",
    );
    const replay = { ...received, received: "duplicate" as const, errorCode: null };
    expect(receivedDetail(replay)).toBe(
      "Exact replay; no second order · ACK AA returned to 192.168.1.50",
    );
  });

  it("shows where the LIS listens, for whom, and whether it listens now", async () => {
    const { user } = renderPage(
      { test_partner_folder: () => NOTHING_FOUND },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText(
        "Receives HL7 order messages from CLINIC-B over MLLP on 192.168.1.20:6660.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("MLLP – LIS listens – 192.168.1.20:6660")).toBeTruthy();
    expect(screen.getByText("192.168.1.50 (Hospital B interface engine)")).toBeTruthy();
    expect(screen.getByText(/^Listening since /)).toBeTruthy();
    expect(screen.getByText("Held until the patient is registered")).toBeTruthy();
    expect(screen.queryByText("Processed folder")).toBeNull();
    // It sends the ACKs itself; it awaits none (#4704).
    expect(screen.queryByText("Awaiting application ACK")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Test listener" }));
    expect(await screen.findByText("✓ The LIS can listen on 192.168.1.20:6660.")).toBeTruthy();
    // The check names what it covers: where the LIS listens and who may connect.
    await user.click(screen.getByRole("button", { name: "Record destination check" }));
    const check = await screen.findByRole("dialog", { name: "Record destination check" });
    expect(check.textContent).toContain(
      "MLLP listener: LAN 192.168.1.20:6660, from 192.168.1.50 (Hospital B interface engine)",
    );
  });

  it("says why a listener does not listen, and lists it by its address", async () => {
    const failing = {
      ...ORDERS_LISTEN,
      status: "error" as const,
      listener: { listening: false, problem: "port_in_use", since: "2026-11-06T09:00:00Z" },
    };
    renderPage(
      { get_partner_connection: () => ({ ...DETAIL, connection: failing }) },
      `/interfaces/connections/${CONNECTION_ID}`,
    );
    expect(
      await screen.findByText(/^Not listening: The port is in use by another program/),
    ).toBeTruthy();
    renderPage({}, "/interfaces/connections");
    expect(await screen.findByText("LAN 192.168.1.20:6660")).toBeTruthy();
  });
});
