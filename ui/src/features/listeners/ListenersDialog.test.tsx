import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import type { RunSummary } from "@/generated/RunSummary";
import type { SetInstrumentEnabledInput } from "@/generated/SetInstrumentEnabledInput";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { session } from "@/lib/test/session";
import { ListenerSummary } from "./ListenerStatus";

function analyzer(n: number, enabled: boolean): InstrumentConfiguration {
  return {
    id: `00000000-0000-7000-8000-00000000100${n}`,
    code: `SYN-AN${n}`,
    name: `Synthetic analyzer ${n}`,
    connection: {
      kind: "astm_serial",
      portPath: `/dev/ttyUSB${n}`,
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
const OFF_B = analyzer(3, false);

function status(
  instrument: InstrumentConfiguration,
  state: ListenerStatusView["state"],
): ListenerStatusView {
  return {
    instrumentId: instrument.id,
    instrumentName: instrument.name,
    state,
    lastActivityAt: null,
    detail: state === "disabled" ? "configuration_disabled" : null,
  };
}

const QUIET = [status(ON, "listening"), status(OFF_A, "disabled"), status(OFF_B, "disabled")];

const OPEN_RUN: RunSummary = {
  runId: "00000000-0000-7000-8000-000000002001",
  instrumentId: ON.id,
  methodVersionId: "00000000-0000-7000-8000-000000003001",
  state: "open",
  createdAt: "2026-11-05T10:00:00Z",
  positionCount: 4,
};

/** `set_instrument_enabled` answering as the engine does: the analyzer at its next revision. */
function setEnabled(args: unknown) {
  const { input } = args as { input: SetInstrumentEnabledInput };
  const before = [ON, OFF_A, OFF_B].find((each) => each.id === input.instrumentId);
  return {
    instrument: { ...before, enabled: input.enabled, revision: input.expectedRevision + 1 },
    listenersRestarted: true,
    message: "Saved. The analyzer listeners restarted with the new settings.",
  };
}

function renderWindow(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["manage_configuration"],
) {
  mockCommands({
    current_session: () => session(capabilities),
    list_listener_status: () => QUIET,
    list_instruments: () => [ON, OFF_A, OFF_B],
    list_runs: () => [],
    set_instrument_enabled: setEnabled,
    ...commands,
  });
  return renderRoutes([{ path: "/", element: <ListenerSummary /> }], "/");
}

async function openWindow(user: ReturnType<typeof renderWindow>["user"]) {
  await user.click(await screen.findByRole("button", { name: /^3 analyzers:/ }));
  return screen.findByRole("dialog", { name: "Analyzer listeners" });
}

describe("Analyzer listeners window actions (ticket #4746)", () => {
  it("enables one analyzer at once and says it is listening now", async () => {
    let statuses = QUIET;
    const change = vi.fn((args: unknown) => {
      statuses = [status(ON, "listening"), status(OFF_A, "listening"), status(OFF_B, "disabled")];
      return setEnabled(args);
    });
    const { user, container } = renderWindow({
      list_listener_status: () => statuses,
      set_instrument_enabled: change,
    });
    const dialog = await openWindow(user);
    await user.click(
      await within(dialog).findByRole("button", { name: "Enable Synthetic analyzer 2" }),
    );
    expect(change).toHaveBeenCalledWith({
      input: { instrumentId: OFF_A.id, expectedRevision: 2, enabled: true },
    });
    expect(await within(dialog).findByText("1 instrument enabled")).toBeTruthy();
    await waitFor(() =>
      expect(dialog.textContent).toContain("Listening now: Synthetic analyzer 2."),
    );
    expect(screen.queryByRole("alertdialog")).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("asks before disabling an analyzer with an open run, then disables it", async () => {
    const change = vi.fn(setEnabled);
    const { user } = renderWindow({ list_runs: () => [OPEN_RUN], set_instrument_enabled: change });
    const dialog = await openWindow(user);
    await user.click(
      await within(dialog).findByRole("button", { name: "Disable Synthetic analyzer 1" }),
    );
    const confirm = await screen.findByRole("alertdialog", {
      name: "Disable: Synthetic analyzer 1?",
    });
    expect(within(confirm).getByText("Synthetic analyzer 1 has an open run.")).toBeTruthy();
    expect(change).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Disable" }));
    expect(await within(dialog).findByText("1 instrument disabled")).toBeTruthy();
    expect(change).toHaveBeenCalledWith({
      input: { instrumentId: ON.id, expectedRevision: 1, enabled: false },
    });
  });

  it("disables an idle analyzer without asking", async () => {
    const change = vi.fn(setEnabled);
    const { user } = renderWindow({ set_instrument_enabled: change });
    const dialog = await openWindow(user);
    await user.click(await within(dialog).findByRole("button", { name: "Disable all" }));
    expect(await within(dialog).findByText("1 instrument disabled")).toBeTruthy();
    expect(change).toHaveBeenCalledOnce();
  });

  it("asks before Enable all while an analyzer is sending, and reports each refusal", async () => {
    const change = vi.fn((args: unknown) => {
      const { input } = args as { input: SetInstrumentEnabledInput };
      if (input.instrumentId === OFF_B.id) {
        return Promise.reject(engineError("conflict", "Someone else changed this record."));
      }
      return setEnabled(args);
    });
    const { user } = renderWindow({
      list_listener_status: () => [
        status(ON, "receiving"),
        status(OFF_A, "disabled"),
        status(OFF_B, "disabled"),
      ],
      set_instrument_enabled: change,
    });
    const dialog = await openWindow(user);
    await user.click(await within(dialog).findByRole("button", { name: "Enable all" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Enable all: 2 instruments?",
    });
    expect(within(confirm).getByText("Synthetic analyzer 1 is sending results now.")).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Enable all" }));
    expect(await within(dialog).findByText("1 instrument enabled")).toBeTruthy();
    expect(dialog.textContent).toContain("Not enabled: 1 instrument.");
    expect(dialog.textContent).toMatch(/Synthetic analyzer 3 \(Serial \/dev\/ttyUSB3.*\): /);
    expect(change).toHaveBeenCalledTimes(2);
  });

  it("asks when open runs cannot be checked", async () => {
    const { user } = renderWindow({
      list_runs: () => Promise.reject(engineError("internal", "Not available.")),
    });
    const dialog = await openWindow(user);
    await user.click(await within(dialog).findByRole("button", { name: "Disable all" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Disable all: Synthetic analyzer 1?",
    });
    expect(within(confirm).getByText("Open runs could not be checked.")).toBeTruthy();
  });

  it("opens an analyzer's settings from its name and closes the window", async () => {
    const { user } = renderWindow();
    const dialog = await openWindow(user);
    await user.click(await within(dialog).findByRole("link", { name: "Synthetic analyzer 2" }));
    expect(
      await screen.findByText(`Navigated to /configuration/instruments?open=${OFF_A.id}`),
    ).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("stays read-only without the right to manage analyzers", async () => {
    const instruments = vi.fn(() => [ON, OFF_A, OFF_B]);
    const { user } = renderWindow({ list_instruments: instruments }, ["perform_analysis"]);
    const dialog = await openWindow(user);
    expect(await within(dialog).findByText("Synthetic analyzer 2")).toBeTruthy();
    expect(within(dialog).queryByRole("link")).toBeNull();
    expect(within(dialog).queryByRole("button", { name: /able/ })).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Close" })).toBeTruthy();
    expect(instruments).not.toHaveBeenCalled();
  });
});
