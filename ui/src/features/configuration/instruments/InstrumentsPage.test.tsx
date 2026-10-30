import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { InstrumentsPage } from "./InstrumentsPage";

const TCP: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001001",
  code: "SYN-AN1",
  name: "Synthetic analyzer 1",
  connection: {
    kind: "astm_tcp",
    bindAddress: "10.0.0.5",
    port: 5000,
    allowedPeers: ["10.0.0.21"],
  },
  enabled: true,
  revision: 3,
};

const SERIAL: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001002",
  code: "SYN-AN2",
  name: "Synthetic analyzer 2",
  connection: {
    kind: "astm_serial",
    portPath: "/dev/ttyUSB0",
    baudRate: 9600,
    dataBits: "eight",
    parity: "none",
    stopBits: "one",
  },
  enabled: false,
  revision: 1,
};

function result(instrument: InstrumentConfiguration, restarted = true): InstrumentChangeResult {
  return {
    instrument,
    listenersRestarted: restarted,
    message: restarted
      ? "Saved. The analyzer listeners restarted with the new settings."
      : "Saved, but the analyzer listeners could not restart. See the listener status.",
  };
}

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  path = "/instruments",
) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_instruments: () => [TCP, SERIAL],
    list_listener_status: () => [],
    ...commands,
  });
  return renderRoutes([{ path: "/instruments", element: <InstrumentsPage /> }], path);
}

/** Opens the analyzer's panel from its row and returns the panel. */
async function openAnalyzer(user: ReturnType<typeof renderPage>["user"], code: string) {
  await openRow(user, code);
  return screen.findByRole("region", { name: code });
}

describe("InstrumentsPage (REQ-INT-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists analyzers with their connection and listening state", async () => {
    const { container } = renderPage();
    expect(await screen.findByText("TCP 10.0.0.5:5000, from 10.0.0.21")).toBeTruthy();
    expect(screen.getByText("Serial /dev/ttyUSB0, 9600 baud, 8N1")).toBeTruthy();
    expect(screen.getByText("Enabled")).toBeTruthy();
    expect(screen.getByText("Disabled")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("registers a network analyzer by keyboard, then opens it with the restart's outcome", async () => {
    const added: InstrumentConfiguration = {
      ...TCP,
      id: "00000000-0000-7000-8000-000000001003",
      code: "SYN-AN3",
      name: "Synthetic analyzer 3",
    };
    const register = vi.fn(() => result(added));
    const list = vi.fn(() => [TCP, SERIAL]);
    const { user, router } = renderPage({ register_instrument: register, list_instruments: list });
    await tabTo(user, await screen.findByRole("button", { name: "Register analyzer" }));
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog", { name: "Register analyzer" });
    await user.keyboard("SYN-AN3");
    await typeInto(user, /^Name/, "Synthetic analyzer 3");
    await chooseWithKeyboard(user, /Connection$/, "Network (ASTM over TCP/IP)");
    await typeInto(user, /^Listen on address/, "10.0.0.5");
    await typeInto(user, /^Port/, "5001");
    await typeInto(user, /^Analyzer addresses allowed/, "10.0.0.22, 10.0.0.23");
    await chooseWithKeyboard(user, /Listening$/, "Listen for this analyzer");
    list.mockReturnValue([TCP, SERIAL, added]);
    await tabTo(user, within(dialog).getByRole("button", { name: "Register analyzer" }));
    await user.keyboard("{Enter}");
    const panel = await screen.findByRole("region", { name: "SYN-AN3" });
    expect(router.state.location.search).toBe(`?open=${added.id}`);
    const notice = await within(panel).findByText(/The analyzer listeners restarted/);
    expect(notice.closest("[role=status]")?.textContent).toContain("Synthetic analyzer 3 saved");
    expect(register).toHaveBeenCalledWith({
      input: {
        code: "SYN-AN3",
        name: "Synthetic analyzer 3",
        connection: {
          kind: "astm_tcp",
          bindAddress: "10.0.0.5",
          port: 5001,
          allowedPeers: ["10.0.0.22", "10.0.0.23"],
        },
        enabled: true,
      },
    });
  });

  it("offers Register analyzer only with manage_configuration", async () => {
    renderPage({ current_session: () => sessionWith([]) });
    await screen.findByRole("gridcell", { name: "SYN-AN1" });
    expect(screen.queryByRole("button", { name: "Register analyzer" })).toBeNull();
  });

  it("requires the allowed addresses of a network analyzer", async () => {
    const register = vi.fn();
    const { user } = renderPage({ register_instrument: register });
    await user.click(await screen.findByRole("button", { name: "Register analyzer" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Connection$/, "Network (ASTM over TCP/IP)");
    await user.click(within(dialog).getByRole("button", { name: "Register analyzer" }));
    expect(
      await within(dialog).findByText("Enter analyzer addresses allowed to connect."),
    ).toBeTruthy();
    expect(register).not.toHaveBeenCalled();
  });

  it("edits a connection with its revision, and warns when the listeners did not restart", async () => {
    const update = vi.fn(() => result(SERIAL, false));
    const { user } = renderPage({ update_instrument_connection: update });
    const panel = await openAnalyzer(user, "SYN-AN2");
    await user.click(within(panel).getByRole("button", { name: "Edit connection" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Edit connection of Synthetic analyzer 2",
    });
    const baud = within(dialog).getByRole("textbox", { name: /^Baud rate/ });
    expect(baud).toHaveProperty("value", "9600");
    await user.clear(baud);
    await user.type(baud, "19200");
    await user.click(within(dialog).getByRole("button", { name: "Save connection" }));
    const warning = await within(panel).findByText(/could not restart/);
    expect(warning.closest("[role=alert]")).toBeTruthy();
    expect(update).toHaveBeenCalledWith({
      input: {
        instrumentId: SERIAL.id,
        expectedRevision: 1,
        connection: { ...SERIAL.connection, baudRate: 19200 },
      },
    });
  });

  it("disables listening after confirmation, and shows a refused change", async () => {
    const setEnabled = vi
      .fn()
      .mockRejectedValueOnce(engineError("conflict", "Someone else changed this record."))
      .mockReturnValue(result({ ...TCP, enabled: false }));
    const { user } = renderPage({ set_instrument_enabled: setEnabled });
    const panel = await openAnalyzer(user, "SYN-AN1");
    await user.click(within(panel).getByRole("button", { name: "Disable listening" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Disable listening for Synthetic analyzer 1?",
    });
    await user.click(within(confirm).getByRole("button", { name: "Disable listening" }));
    expect(await within(confirm).findByText("Someone else changed this record.")).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Disable listening" }));
    expect(await within(panel).findByText(/listeners restarted/)).toBeTruthy();
    expect(setEnabled).toHaveBeenLastCalledWith({
      input: { instrumentId: TCP.id, expectedRevision: 3, enabled: false },
    });
  });

  it("shows loading under the header, then an empty list", async () => {
    renderPage({ list_instruments: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "Instruments" })).toBeTruthy();
    expect(screen.getByText("Loading analyzers…")).toBeTruthy();
    expect(await screen.findByText("No analyzers yet")).toBeTruthy();
  });

  it("shows the engine's error, and says who can help when it refuses", async () => {
    renderPage({
      list_instruments: () => Promise.reject(engineError("forbidden", "You cannot see analyzers.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot see analyzers.");
    expect(alert.textContent).toContain("A laboratory administrator can change roles.");
  });

  it("opens an analyzer with Enter, and Escape closes it with focus back on the row", async () => {
    const { user, router, container } = renderPage();
    await screen.findByRole("gridcell", { name: "SYN-AN2" });
    await openRowWithEnter(user, "SYN-AN2");
    const panel = await screen.findByRole("region", { name: "SYN-AN2" });
    expect(router.state.location.search).toBe(`?open=${SERIAL.id}`);
    expect(within(panel).getByText("/dev/ttyUSB0")).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "Enable listening" })).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(within(panel).getByRole("heading", { name: "SYN-AN2" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-AN2" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "SYN-AN2" })),
    );
  });

  it("opens the analyzer named in the address, and offers the same actions on its row", async () => {
    const { user } = renderPage({}, `/instruments?open=${TCP.id}`);
    expect(await screen.findByRole("region", { name: "SYN-AN1" })).toBeTruthy();
    const menu = within(await openRowActions(user, "SYN-AN1"));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Edit connection",
      "Disable listening",
    ]);
  });

  it("finds analyzers by code or name", async () => {
    const { user } = renderPage();
    await screen.findByRole("gridcell", { name: "SYN-AN1" });
    await user.type(screen.getByRole("searchbox", { name: "Find analyzers" }), "analyzer 2{Enter}");
    expect(await screen.findByText("1 of 2 analyzers")).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "SYN-AN1" })).toBeNull();
    await user.type(screen.getByRole("searchbox", { name: "Find analyzers" }), "x{Enter}");
    expect(await screen.findByText("No analyzers match 'analyzer 2x'")).toBeTruthy();
  });
});
