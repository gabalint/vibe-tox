import { screen, waitFor } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AuConnectionSettings } from "@/generated/AuConnectionSettings";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import { listenerDetailText } from "@/lib/listenerDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { tabTo, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { connectionText } from "./connectionValues";
import { ConnectionDialog, RegisterDialog } from "./InstrumentDialogs";

const AU: AuConnectionSettings = {
  protocol: {
    framing: { start: [2], end: [3], bcc: true, use_etb: true, max_wire_len: 512, class: "b" },
    format: {
      unit_no: false,
      rack_digits: 4,
      sample_id_digits: 20,
      sex: false,
      age: false,
      month: false,
      patient_info_digits: [0, 0, 0, 0, 0, 0],
      result_digits: 9,
    },
  },
  frameTimeoutMs: 2000,
  blockTimeoutMs: 3000,
};
const INSTRUMENT: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001404",
  code: "SYN-AU",
  name: "Synthetic AU400",
  enabled: false,
  revision: 4,
  connection: {
    kind: "au_tcp",
    bindAddress: "127.0.0.1",
    port: 5000,
    allowedPeers: ["127.0.0.1"],
    au: AU,
  },
};

async function choose(user: UserEvent, label: string, option: string) {
  await user.click(screen.getByRole("button", { name: new RegExp(`${label}$`) }));
  await user.click(await screen.findByRole("option", { name: option }));
}
async function type(user: UserEvent, label: string, value: string) {
  const field = screen.getByRole("textbox", { name: label });
  await user.clear(field);
  await user.type(field, value);
}
async function enterAu(user: UserEvent) {
  await choose(user, "Transmission class", "Class B — ACK / NAK");
  await type(user, "Start code (hex)", "02");
  await type(user, "End code (hex)", "03");
  await choose(user, "BCC checksum used", "Yes");
  await choose(user, "ETB block endings used", "Yes");
  await choose(user, "Maximum block length", "512");
  await type(user, "Frame receive timeout (ms)", "2000");
  await type(user, "Next block timeout (ms)", "3000");
  await choose(user, "Instrument unit number sent", "No");
  await choose(user, "Rack number width", "4 digits");
  await type(user, "Sample ID width", "20");
  await choose(user, "Result field width", "9 characters");
  for (const label of ["Sex field sent", "Age field sent", "Month field sent"])
    await choose(user, label, "No");
  for (let i = 1; i <= 6; i += 1) await type(user, `Patient information ${i} width`, "0");
}
function setupRegister(
  register = vi.fn(() => ({ instrument: INSTRUMENT, listenersRestarted: true, message: "Saved." })),
) {
  mockCommands({ register_instrument: register });
  return {
    ...renderWithProviders(<RegisterDialog onSaved={vi.fn()} onClose={vi.fn()} />),
    register,
  };
}

describe("AU400/AU640 connection configuration", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("requires explicit parameter-sheet settings and listening choice", async () => {
    const { user, register } = setupRegister();
    await choose(user, "Connection", "AU400 / AU640 — network bridge");
    expect(screen.getByRole("textbox", { name: "Start code (hex)" })).toHaveProperty("value", "");
    expect(screen.getByText(/transparent serial-to-TCP bridge/)).toBeTruthy();
    expect(
      screen.getByText(/Automatic test requisition \(TRI\) lookup is unavailable/),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Register analyzer" }));
    expect(await screen.findByText("Choose transmission class.")).toBeTruthy();
    expect(screen.getByText("Choose listening.")).toBeTruthy();
    expect(register).not.toHaveBeenCalled();
    await expectNoA11yViolations(screen.getByRole("dialog"));
  });

  it("registers an AU bridge with exact settings and listening explicitly disabled", async () => {
    const { user, register } = setupRegister();
    await type(user, "Code", "SYN-AU");
    await type(user, "Name", "Synthetic AU400");
    await choose(user, "Connection", "AU400 / AU640 — network bridge");
    await type(user, "Listen on address", "127.0.0.1");
    await type(user, "Port", "5000");
    await type(user, "Analyzer addresses allowed to connect", "127.0.0.1");
    await enterAu(user);
    await choose(user, "Listening", "Keep disabled for now");
    await user.click(screen.getByRole("button", { name: "Register analyzer" }));
    await waitFor(() =>
      expect(register).toHaveBeenCalledWith({
        input: {
          code: "SYN-AU",
          name: "Synthetic AU400",
          connection: INSTRUMENT.connection,
          enabled: false,
        },
      }),
    );
  });

  it("edits a serial AU configuration by keyboard without losing optional fields or two-byte delimiters", async () => {
    const protocol: AuConnectionSettings = {
      ...AU,
      protocol: {
        framing: {
          ...AU.protocol.framing,
          start: [2, 1],
          end: [3, 4],
          bcc: false,
          use_etb: false,
          class: "a",
          max_wire_len: 1024,
        },
        format: {
          ...AU.protocol.format,
          unit_no: true,
          rack_digits: 5,
          sex: true,
          age: true,
          month: true,
          result_digits: 6,
          patient_info_digits: [1, 2, 3, 4, 5, 6],
        },
      },
    };
    const instrument: InstrumentConfiguration = {
      ...INSTRUMENT,
      connection: {
        kind: "au_serial",
        portPath: "COM3",
        baudRate: 9600,
        dataBits: "seven",
        parity: "even",
        stopBits: "two",
        au: protocol,
      },
    };
    const update = vi.fn(() => ({ instrument, listenersRestarted: true, message: "Saved." }));
    mockCommands({ update_instrument_connection: update });
    const { user } = renderWithProviders(
      <ConnectionDialog instrument={instrument} onSaved={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByRole("textbox", { name: "Start code (hex)" })).toHaveProperty(
      "value",
      "02 01",
    );
    await expectNoA11yViolations(screen.getByRole("dialog"));
    const port = screen.getByRole("textbox", { name: "Serial port" });
    await tabTo(user, port);
    await user.keyboard("{End}{Backspace}4");
    await tabTo(user, screen.getByRole("button", { name: "Save connection" }));
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith({
        input: {
          instrumentId: instrument.id,
          expectedRevision: 4,
          connection: { ...instrument.connection, portPath: "COM4" },
        },
      }),
    );
    expect(connectionText(instrument.connection)).toBe(
      "AU400 / AU640 · Serial COM3, 9600 baud, 7E2",
    );
  });

  it("rejects invalid hex, widths and timers before submitting and shows engine field errors", async () => {
    const update = vi.fn().mockRejectedValue(
      engineError("validation", "Check the AU settings.", {
        fieldErrors: [
          {
            field: "connection.au.protocol.format.sample_id_digits",
            code: "invalid",
            message: "Check the sample ID width.",
          },
        ],
      }),
    );
    mockCommands({ update_instrument_connection: update });
    const { user } = renderWithProviders(
      <ConnectionDialog instrument={INSTRUMENT} onSaved={vi.fn()} onClose={vi.fn()} />,
    );
    await type(user, "Start code (hex)", "0G");
    await type(user, "Sample ID width", "21");
    await type(user, "Next block timeout (ms)", "50");
    await user.click(screen.getByRole("button", { name: "Save connection" }));
    expect(await screen.findByText(/Enter one or two hexadecimal bytes/)).toBeTruthy();
    expect(screen.getByText("Enter a whole number from 1 to 20.")).toBeTruthy();
    expect(screen.getByText("Enter 100–9900 milliseconds in steps of 100.")).toBeTruthy();
    expect(update).not.toHaveBeenCalled();
    await type(user, "Start code (hex)", "02");
    await type(user, "Sample ID width", "20");
    await type(user, "Next block timeout (ms)", "3000");
    await user.click(screen.getByRole("button", { name: "Save connection" }));
    expect(await screen.findByText("Check the sample ID width.")).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: "Sample ID width" })).toHaveProperty(
        "ariaInvalid",
        "true",
      ),
    );
  });

  it("keeps unsupported requisition lookup visible as a listener reason", () => {
    expect(listenerDetailText({ detail: "au_query_unsupported" })).toContain(
      "automatic requisition lookup is unavailable",
    );
  });
});
