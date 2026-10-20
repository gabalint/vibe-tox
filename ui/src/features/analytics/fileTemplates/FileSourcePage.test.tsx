import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { sessionWith } from "../testData";
import { FileSourcePage } from "./FileSourcePage";
import {
  FENTANYL,
  FILE_INSTRUMENT,
  FILE_INSTRUMENT_ID,
  FILE_SOURCE,
  LONG_READING,
  LONG_SAMPLE,
  MORPHINE,
  SAMPLE_HANDLE,
} from "./testTemplates";

const PATH = `/bench/setup/instruments/${FILE_INSTRUMENT_ID}/sources/${FILE_SOURCE.registrationId}`;

function renderPage(
  capabilities: string[],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_instrument_readiness: () => [FILE_INSTRUMENT],
    list_source_registrations: () => [FILE_SOURCE],
    list_analytes: () => [MORPHINE, FENTANYL],
    open_layout_sample: () => LONG_SAMPLE,
    check_file_layout: () => LONG_READING,
    ...commands,
  });
  return renderRoutes(
    [
      {
        path: "/bench/setup/instruments/:instrumentId/sources/:registrationId",
        element: <FileSourcePage />,
      },
    ],
    PATH,
  );
}

describe("FileSourcePage (result files brief §7)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows everything the template says, with names and words as written", async () => {
    const { container } = renderPage(["perform_analysis"]);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Synthetic Quant 12.1" }),
    ).toBeTruthy();
    expect(screen.getByText("Draft, awaiting approval")).toBeTruthy();
    expect(
      screen.getByText("Sample Name | Sample Type | Component | Final Conc | Units | Dilution"),
    ).toBeTruthy();
    expect(screen.getByText("Long: one row per sample and analyte")).toBeTruthy();
    expect(await screen.findByText("Morphine → Synthetic morphine (SYN-MOR)")).toBeTruthy();
    expect(screen.getByText("Morphine-D6")).toBeTruthy();
    expect(screen.getByText("Standard → Calibrator")).toBeTruthy();
    expect(screen.getByText("N/F → Below the limit of quantitation")).toBeTruthy();
    expect(screen.getByText("ng/mL")).toBeTruthy();
    // Only an approver may approve.
    expect(screen.queryByRole("button", { name: "Approve source" })).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("lets an approver check the template with a file, then approve it", async () => {
    const check = vi.fn(() => LONG_READING);
    const approve = vi.fn(() => ({ ...FILE_SOURCE, state: "active", allowedActions: [] }));
    const { user } = renderPage(["approve_configuration"], {
      check_file_layout: check,
      approve_source: approve,
    });
    await user.click(await screen.findByRole("button", { name: "Check with a file" }));
    const rows = await screen.findByRole("grid", { name: "Rows read from the file" });
    expect(within(rows).getByRole("gridcell", { name: "Unmapped word, quarantined" })).toBeTruthy();
    expect(check).toHaveBeenCalledWith({
      input: {
        sampleHandle: SAMPLE_HANDLE,
        layout: FILE_SOURCE.fileLayout,
        testCodes: FILE_SOURCE.testCodes,
      },
    });
    await user.click(screen.getByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Approve Synthetic Quant 12.1",
    });
    expect(within(confirm).getByText(/You checked it with a file on this page\./)).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    expect(await screen.findByText("Synthetic Quant 12.1 approved")).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: {
        registrationId: FILE_SOURCE.registrationId,
        expectedRevision: 1,
        selfApproval: false,
      },
    });
  });

  it("says when a file does not match the template, and when the approver has not checked", async () => {
    const mismatch: LayoutCheck = { ...LONG_READING, failure: "layout_mismatch", rows: [] };
    const { user } = renderPage(["approve_configuration"], { check_file_layout: () => mismatch });
    await user.click(await screen.findByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(
      within(confirm).getByText(/You have not checked it with a file on this page\./),
    ).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Check with a file" }));
    expect(await screen.findByText("No row can be read from this file")).toBeTruthy();
    expect(
      screen.getByText(/No line among the first 50 matches the template's header/),
    ).toBeTruthy();
  });

  it("shows the engine's refusal of an approval by the template's author", async () => {
    const { user } = renderPage(["approve_configuration"], {
      approve_source: () =>
        Promise.reject(engineError("gate_blocked", "The author of a source cannot approve it.")),
    });
    await user.click(await screen.findByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    expect(
      await within(confirm).findByText("The author of a source cannot approve it."),
    ).toBeTruthy();
  });

  it("says when the template is not found, and shows a failed read", async () => {
    renderPage(["perform_analysis"], { list_source_registrations: () => [] });
    expect(await screen.findByText("File template not found")).toBeTruthy();
  });

  it("shows the engine's error when the sources cannot be read", async () => {
    renderPage(["perform_analysis"], {
      list_source_registrations: () => Promise.reject(engineError("internal", "Not readable.")),
    });
    expect(await screen.findByText("Not readable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
