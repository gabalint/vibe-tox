import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { SetInstrumentEnabledInput } from "@/generated/SetInstrumentEnabledInput";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { InstrumentsPage } from "./InstrumentsPage";

function analyzer(
  n: number,
  enabled: boolean,
  portPath = `/dev/ttyUSB${n}`,
): InstrumentConfiguration {
  return {
    id: `00000000-0000-7000-8000-00000000100${n}`,
    code: `SYN-AN${n}`,
    name: `Synthetic analyzer ${n}`,
    connection: {
      kind: "astm_serial",
      portPath,
      baudRate: 9600,
      dataBits: "eight",
      parity: "none",
      stopBits: "one",
    },
    enabled,
    revision: n,
  };
}

const ON = analyzer(1, true);
const OFF_A = analyzer(2, false);
const OFF_B = analyzer(3, false, "COM10");
const FILE: InstrumentConfiguration = {
  ...analyzer(4, false),
  connection: { kind: "result_file" },
};

/** `set_instrument_enabled` answering as the engine does: the analyzer at its next revision. */
function setEnabled(args: unknown) {
  const { input } = args as { input: SetInstrumentEnabledInput };
  const before = [ON, OFF_A, OFF_B, FILE].find((each) => each.id === input.instrumentId);
  return {
    instrument: { ...before, enabled: input.enabled, revision: input.expectedRevision + 1 },
    listenersRestarted: true,
    message: "Saved. The analyzer listeners restarted with the new settings.",
  };
}

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_instruments: () => [ON, OFF_A, OFF_B, FILE],
    list_listener_status: () => [],
    set_instrument_enabled: setEnabled,
    ...commands,
  });
  return renderRoutes([{ path: "/instruments", element: <InstrumentsPage /> }], "/instruments");
}

describe("Enable all and Disable all (REQ-INT-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("enables only the disabled analyzers, one command each, and reports each refusal", async () => {
    const change = vi.fn((args: unknown) => {
      const { input } = args as { input: SetInstrumentEnabledInput };
      if (input.instrumentId === OFF_A.id) {
        return Promise.reject(engineError("conflict", "Someone else changed this record."));
      }
      return setEnabled(args);
    });
    const { user, container } = renderPage({ set_instrument_enabled: change });
    await user.click(await screen.findByRole("button", { name: "Enable all" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Enable all: 3 instruments?" });
    // The enabled analyzer is not listed; each shows what it turns on.
    expect(within(dialog).queryByText(/Synthetic analyzer 1/)).toBeNull();
    expect(
      within(dialog).getByText(/Synthetic analyzer 3 \(Serial COM10.*\) — listening/),
    ).toBeTruthy();
    expect(within(dialog).getByText(/Synthetic analyzer 4 .* — folder pickup/)).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(within(dialog).getByRole("button", { name: "Enable all" }));

    const outcome = await screen.findByRole("alert");
    expect(within(outcome).getByText("2 instruments enabled")).toBeTruthy();
    expect(outcome.textContent).toContain("Not enabled: 1 instrument.");
    expect(outcome.textContent).toMatch(/Synthetic analyzer 2 \(Serial \/dev\/ttyUSB2.*\): /);
    expect(change).toHaveBeenCalledTimes(3);
    expect(change.mock.calls.map(([args]) => (args as { input: unknown }).input)).toEqual([
      { instrumentId: OFF_A.id, expectedRevision: 2, enabled: true },
      { instrumentId: OFF_B.id, expectedRevision: 3, enabled: true },
      { instrumentId: FILE.id, expectedRevision: 4, enabled: true },
    ]);
  });

  it("names an enabled analyzer whose listener could not start", async () => {
    const { user } = renderPage({
      list_listener_status: () => [
        {
          instrumentId: OFF_B.id,
          instrumentName: OFF_B.name,
          state: "error",
          lastActivityAt: null,
          detail: "open_failed",
        },
      ],
    });
    await user.click(await screen.findByRole("button", { name: "Enable all" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Enable all" }));
    const outcome = await screen.findByRole("alert");
    expect(within(outcome).getByText("3 instruments enabled")).toBeTruthy();
    await waitFor(() =>
      expect(outcome.textContent).toContain(
        "Synthetic analyzer 3 (Serial COM10, 9600 baud, 8N1): Could not open the connection",
      ),
    );
  });

  it("disables the one enabled analyzer and confirms with a summary", async () => {
    const change = vi.fn(setEnabled);
    const { user } = renderPage({ set_instrument_enabled: change });
    await user.click(await screen.findByRole("button", { name: "Disable all" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Disable all: 1 instrument?" });
    await user.click(within(dialog).getByRole("button", { name: "Disable all" }));
    const title = await screen.findByText("1 instrument disabled");
    expect(title.closest("[role=status]")).toBeTruthy();
    expect(change).toHaveBeenCalledOnce();
    expect(change).toHaveBeenCalledWith({
      input: { instrumentId: ON.id, expectedRevision: 1, enabled: false },
    });
  });

  it("offers only the button that would change something, and neither without the capability", async () => {
    mockCommands({
      current_session: () => sessionWith(MANAGER),
      list_instruments: () => [ON],
      list_listener_status: () => [],
    });
    const { unmount } = renderRoutes(
      [{ path: "/instruments", element: <InstrumentsPage /> }],
      "/instruments",
    );
    expect(await screen.findByRole("button", { name: "Disable all" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Enable all" })).toBeNull();
    unmount();
    mockCommands({
      current_session: () => sessionWith([]),
      list_instruments: () => [ON, OFF_A],
      list_listener_status: () => [],
    });
    renderRoutes([{ path: "/instruments", element: <InstrumentsPage /> }], "/instruments");
    expect(await screen.findByText("Synthetic analyzer 2")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /able all/ })).toBeNull();
  });
});
