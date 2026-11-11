import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ANALYTE, DRAFT_SOURCE, INSTRUMENT, sessionWith } from "../testData";
import { InstrumentSources } from "./InstrumentSources";

const AU_DRAFT: SourceRegistrationView = {
  ...DRAFT_SOURCE,
  senderName: "AU640",
  senderVersion: "1",
  revisionIndicator: "none",
  testCodes: [
    { sourceTestCode: "01", analyteId: ANALYTE.id },
    { sourceTestCode: "02", analyteId: ANALYTE.id },
  ],
  auProfile: {
    protocol: {
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
    },
    tests: [
      { testCode: "01", unit: "ng/mL" },
      { testCode: "02", unit: null },
    ],
  },
};

describe("InstrumentSources (REQ-INT-001)", () => {
  it("shows an AU source's wire settings and declared units for its approver", async () => {
    mockCommands({
      current_session: () => sessionWith(["approve_configuration"]),
      list_source_registrations: () => [AU_DRAFT],
      list_analytes: () => [ANALYTE],
    });
    renderWithProviders(<InstrumentSources instrument={INSTRUMENT} />);
    const codes = await screen.findByRole("list", { name: "Test codes of AU640 1" });
    expect(codes.textContent).toContain("01 → Synthetic amphetamine (SYN-AMP) · ng/mL");
    expect(codes.textContent).toContain("02 → Synthetic amphetamine (SYN-AMP) · no unit");
    expect(
      screen.getByText("Class B · start 02 · end 03 · BCC · sample ID width 9 · result width 6"),
    ).toBeTruthy();
  });
});
