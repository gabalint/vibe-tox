import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AuConfig } from "@/generated/AuConfig";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { ANALYTE, DRAFT_SOURCE, INSTRUMENT } from "../testData";
import { RegisterSourceDialog } from "./RegisterSourceDialog";

const AU_INSTRUMENT: InstrumentReadinessView = {
  ...INSTRUMENT,
  instrumentId: "00000000-0000-7000-8000-000000001405",
  name: "Synthetic AU640",
  transport: "au_serial",
  activeRegistration: null,
};
const PROTOCOL: AuConfig = {
  framing: { start: [2], end: [3], bcc: true, use_etb: false, max_wire_len: 256, class: "b" },
  format: {
    unit_no: false,
    rack_digits: 4,
    sample_id_digits: 9,
    sex: false,
    age: false,
    month: false,
    patient_info_digits: [0, 0, 0, 0, 0, 0],
    result_digits: 6,
  },
};
const AU_CONFIGURATION: InstrumentConfiguration = {
  id: AU_INSTRUMENT.instrumentId,
  code: "SYN-AU",
  name: AU_INSTRUMENT.name,
  enabled: true,
  revision: 1,
  connection: {
    kind: "au_serial",
    portPath: "/dev/ttyUSB0",
    baudRate: 9600,
    dataBits: "eight",
    parity: "none",
    stopBits: "one",
    au: { protocol: PROTOCOL, frameTimeoutMs: 3000, blockTimeoutMs: 5000 },
  },
};

function renderDialog(register: (args: unknown) => unknown = vi.fn(() => DRAFT_SOURCE)) {
  mockCommands({
    list_instruments: () => [AU_CONFIGURATION],
    list_analytes: () => [ANALYTE],
    register_source: register,
  });
  return renderWithProviders(<RegisterSourceDialog instrument={AU_INSTRUMENT} onClose={vi.fn()} />);
}

/** Fills everything an AU source asks, with one test code row. */
async function fillAuSource(user: ReturnType<typeof renderDialog>["user"], code: string) {
  // The form appears once the analyzer's connection has loaded.
  await user.type(await screen.findByRole("textbox", { name: /^Sender name/ }), "AU640");
  const dialog = screen.getByRole("dialog");
  await user.type(within(dialog).getByRole("textbox", { name: /^Sender version/ }), "1");
  await chooseWithKeyboard(user, /Reports final concentration after dilution$/, "No");
  await user.click(within(dialog).getByRole("button", { name: "Add test code" }));
  await user.type(within(dialog).getByRole("textbox", { name: /^Source test code/ }), code);
  await chooseWithKeyboard(user, /Analyte$/, "Synthetic amphetamine (SYN-AMP)");
  return dialog;
}

describe("RegisterSourceDialog for an AU analyzer (REQ-INT-001)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  it("takes the wire settings from the connection and declares each test's unit", async () => {
    const register = vi.fn(() => DRAFT_SOURCE);
    const { user, container } = renderDialog(register);
    const dialog = await fillAuSource(user, "01");
    expect(
      within(dialog).getByText(
        "From the analyzer's connection: Class B · start 02 · end 03 · BCC · sample ID width 9 · result width 6",
      ),
    ).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: /Sample ID is read from$/ })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: /Corrected results$/ })).toBeNull();
    await user.type(within(dialog).getByRole("textbox", { name: /^Unit/ }), "ng/mL");
    await user.click(within(dialog).getByRole("button", { name: "Add test code" }));
    await user.type(
      within(dialog).getAllByRole("textbox", { name: /^Source test code/ })[1] as HTMLElement,
      "02",
    );
    await user.click(within(dialog).getAllByRole("button", { name: /Analyte$/ })[1] as HTMLElement);
    await user.click(
      await screen.findByRole("option", { name: "Synthetic amphetamine (SYN-AMP)" }),
    );
    await expectNoA11yViolations(container);
    await user.click(within(dialog).getByRole("button", { name: "Register source" }));
    expect(register).toHaveBeenCalledWith({
      input: {
        instrumentId: AU_INSTRUMENT.instrumentId,
        senderName: "AU640",
        senderVersion: "1",
        sampleIdField: "specimen_id",
        revisionIndicator: "none",
        reportsFinalConcentration: false,
        testCodes: [
          { sourceTestCode: "01", analyteId: ANALYTE.id },
          { sourceTestCode: "02", analyteId: ANALYTE.id },
        ],
        auProfile: {
          protocol: PROTOCOL,
          tests: [
            { testCode: "01", unit: "ng/mL" },
            { testCode: "02", unit: null },
          ],
        },
      },
    });
  });

  it("asks for a two-digit test code before sending anything", async () => {
    const register = vi.fn(() => DRAFT_SOURCE);
    const { user } = renderDialog(register);
    const dialog = await fillAuSource(user, "AMP");
    await user.click(within(dialog).getByRole("button", { name: "Register source" }));
    expect(await within(dialog).findByText("Use a two-digit code from 01 to 99.")).toBeTruthy();
    expect(register).not.toHaveBeenCalled();
  });

  it("shows the engine's unit error on that test code's Unit field", async () => {
    const refusal = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [
        {
          field: "auProfile.tests[0].unit",
          code: "untrimmed",
          message: "Remove surrounding spaces.",
        },
      ],
    });
    const { user } = renderDialog(() => Promise.reject(refusal));
    const dialog = await fillAuSource(user, "01");
    await user.click(within(dialog).getByRole("button", { name: "Register source" }));
    const unit = within(dialog).getByRole("textbox", { name: /^Unit/ });
    expect(await screen.findByText("Remove surrounding spaces.")).toBeTruthy();
    expect(unit.getAttribute("aria-invalid")).toBe("true");
  });

  it("shows the engine's reason in the error box when it names no field on the form", async () => {
    const refusal = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [
        {
          field: "instrumentId",
          code: "source_format_mismatch",
          message:
            "The source format must match the instrument connection and its exact AU wire settings.",
        },
      ],
    });
    const { user } = renderDialog(() => Promise.reject(refusal));
    const dialog = await fillAuSource(user, "01");
    await user.click(within(dialog).getByRole("button", { name: "Register source" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.textContent).toContain("its exact AU wire settings.");
    expect(alert.textContent).not.toContain("Check the highlighted fields.");
  });
});
