import type { InvokeArgs } from "@tauri-apps/api/core";
import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { LayoutCheckRow } from "@/generated/LayoutCheckRow";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { sessionWith } from "../testData";
import { FileTemplatePage } from "./FileTemplatePage";
import {
  FENTANYL,
  FILE_INSTRUMENT,
  FILE_INSTRUMENT_ID,
  FILE_SOURCE,
  LONG_LAYOUT,
  LONG_LINES,
  LONG_READING,
  LONG_SAMPLE,
  MORPHINE,
  SAMPLE_HANDLE,
  WIDE_LINES,
  WIDE_READING,
  WIDE_SAMPLE,
} from "./testTemplates";

const PATH = `/bench/setup/instruments/${FILE_INSTRUMENT_ID}/file-template`;

/** The check of the finished long template: mapped names, words and types. */
const CHECKED: LayoutCheck = {
  ...LONG_READING,
  rows: LONG_READING.rows.map((row): LayoutCheckRow => {
    const mapped = {
      ...row,
      analyteId: row.analyteName === "Morphine" ? MORPHINE.id : null,
      notResult: row.analyteName === "Morphine-D6",
      role: row.typeWord === "Standard" ? "calibrator" : null,
    } satisfies LayoutCheckRow;
    return row.valueText === "N/F"
      ? { ...mapped, reading: "word", wordMeaning: "below_loq" }
      : mapped;
  }),
};

/** A reading of the sample to find names and words: no mapping sent yet. */
function isReading(args: InvokeArgs | undefined): boolean {
  const text = JSON.stringify(args);
  return text.includes('"testCodes":[]') && text.includes('"valueWords":[]');
}

/** Chooses the first record matching `text` in a picker, by keyboard. */
async function pick(user: User, combobox: HTMLElement, text: string) {
  await user.click(combobox);
  await user.keyboard(text);
  await screen.findByRole("listbox");
  await user.keyboard("{ArrowDown}{Enter}");
  await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
}

function renderPage(commands: Record<string, (args: InvokeArgs | undefined) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["manage_configuration"]),
    list_instrument_readiness: () => [FILE_INSTRUMENT],
    list_analytes: () => [MORPHINE, FENTANYL],
    open_layout_sample: () => LONG_SAMPLE,
    preview_layout_sample: () => LONG_LINES,
    check_file_layout: (args) => (isReading(args) ? LONG_READING : CHECKED),
    ...commands,
  });
  return renderRoutes(
    [
      {
        path: "/bench/setup/instruments/:instrumentId/file-template",
        element: <FileTemplatePage />,
      },
    ],
    PATH,
  );
}

type User = ReturnType<typeof renderPage>["user"];

/** Step 1, by keyboard. */
async function describeSoftware(user: User, final: "Yes" | "No") {
  await screen.findByRole("textbox", { name: /^Software name/ });
  await typeInto(user, /^Software name/, "Synthetic Quant");
  await typeInto(user, /^Software version/, "12.1");
  await chooseWithKeyboard(user, /Reports final \(dilution-corrected\) concentrations$/, final);
}

/** Steps 2 to 4 for the synthetic long export. */
async function describeLongFile(user: User) {
  await user.click(await screen.findByRole("button", { name: "Open a sample export" }));
  await chooseWithKeyboard(user, /Encoding$/, "UTF-8");
  await chooseWithKeyboard(user, /Delimiter$/, "Comma");
  // A click on the line in the preview makes it the header.
  await openRow(user, "Sample Name");
  await chooseWithKeyboard(user, /Header rows$/, "One line");
  await chooseWithKeyboard(user, /Shape$/, "Long: one row per sample and analyte");
  await chooseWithKeyboard(user, /Analyte column$/, "3: Component (for example Morphine)");
  await chooseWithKeyboard(user, /Value column$/, "4: Final Conc (for example 152.4)");
  await chooseWithKeyboard(user, /Sample label column$/, "1: Sample Name (for example C00000123)");
  await chooseWithKeyboard(user, /Sample type column$/, "2: Sample Type (for example Unknown)");
  await chooseWithKeyboard(user, /Spaces around values$/, "Remove spaces around each value");
  await chooseWithKeyboard(user, /End of data$/, "At the end of the file");
}

/** Steps 5 to 7: the names and words the sample holds, and the unit. */
async function mapLongFile(user: User) {
  const morphine = await screen.findByRole("group", { name: "Morphine" });
  await pick(user, within(morphine).getByRole("combobox"), "Synthetic morph");
  const standard = screen.getByRole("group", { name: "Morphine-D6" });
  await user.click(within(standard).getByRole("checkbox", { name: "Not a result" }));
  await chooseWithKeyboard(user, /Standard$/, "Calibrator");
  await chooseWithKeyboard(user, /N\/F$/, "Below the limit of quantitation");
  await typeInto(user, /^Declared unit/, "ng/mL");
}

describe("FileTemplatePage (result files brief §7)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("drafts a long template step by step, checks it on the sample and saves it", async () => {
    const register = vi.fn(() => FILE_SOURCE);
    const check = vi.fn((args: InvokeArgs | undefined) =>
      isReading(args) ? LONG_READING : CHECKED,
    );
    const { user, router, container } = renderPage({
      register_file_source: register,
      check_file_layout: check,
    });
    expect(await screen.findByText(/For Synthetic LC-MS\/MS 1\./)).toBeTruthy();
    await describeSoftware(user, "Yes");
    await describeLongFile(user);
    expect(
      await screen.findByText(
        "3 analyte names found in the sample. Rows of a name left unmapped are quarantined.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("Empty cells are always quarantined.")).toBeTruthy();
    await mapLongFile(user);
    await user.click(screen.getByRole("button", { name: "Check the sample" }));
    const rows = await screen.findByRole("grid", { name: "Rows read from the file" });
    expect(
      within(rows).getByRole("gridcell", { name: "Mapped word: below the limit of quantitation" }),
    ).toBeTruthy();
    expect(within(rows).getByRole("gridcell", { name: "Standard: Calibrator" })).toBeTruthy();
    expect(check).toHaveBeenLastCalledWith({
      input: {
        sampleHandle: SAMPLE_HANDLE,
        layout: LONG_LAYOUT,
        testCodes: [{ sourceTestCode: "Morphine", analyteId: MORPHINE.id }],
      },
    });
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(await screen.findByText("File template saved; it needs approval")).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: {
        instrumentId: FILE_INSTRUMENT_ID,
        softwareName: "Synthetic Quant",
        softwareVersion: "12.1",
        reportsFinalConcentration: true,
        layout: LONG_LAYOUT,
        testCodes: [{ sourceTestCode: "Morphine", analyteId: MORPHINE.id }],
      },
    });
    await waitFor(() => expect(router.state.location.search).toBe(`?open=${FILE_INSTRUMENT_ID}`));
  });

  it("says when the template changed after its check", async () => {
    const { user } = renderPage();
    await describeSoftware(user, "Yes");
    await describeLongFile(user);
    await mapLongFile(user);
    await user.click(screen.getByRole("button", { name: "Check the sample" }));
    await screen.findByRole("grid", { name: "Rows read from the file" });
    await user.click(screen.getByRole("button", { name: /N\/F$/ }));
    await user.click(await screen.findByRole("option", { name: "Not mapped" }));
    expect(await screen.findByText(/The template changed after this check/)).toBeTruthy();
  });

  it("lists a two-row header's groups, keeps those with the value field and maps each", async () => {
    const register = vi.fn(() => FILE_SOURCE);
    const { user } = renderPage({
      open_layout_sample: () => WIDE_SAMPLE,
      preview_layout_sample: () => WIDE_LINES,
      check_file_layout: () => WIDE_READING,
      register_file_source: register,
    });
    await describeSoftware(user, "No");
    await user.click(screen.getByRole("button", { name: "Open a sample export" }));
    // A byte-order mark decides the encoding.
    expect(await screen.findByText("UTF-8, set by the mark at the start of the file")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Encoding$/ })).toBeNull();
    await chooseWithKeyboard(user, /Delimiter$/, "Comma");
    await chooseWithKeyboard(
      user,
      /Header line$/,
      "Line 1: Sample, Morphine Results, Fentanyl Results, Qualifier (338.2 -> 105.1) Results",
    );
    await chooseWithKeyboard(user, /Header rows$/, "Two lines: group labels, then field names");
    await chooseWithKeyboard(
      user,
      /Shape$/,
      "Wide: one row per sample, a column or column group per analyte",
    );
    await chooseWithKeyboard(user, /Value field$/, "Final Conc.");
    const kept = screen.getByRole("group", { name: "Analyte groups read" });
    expect(within(kept).getByRole("checkbox", { name: /Morphine Results/ })).toHaveProperty(
      "checked",
      true,
    );
    expect(
      screen.getByText(
        "Groups without that field give no rows: Sample, Qualifier (338.2 -> 105.1) Results.",
      ),
    ).toBeTruthy();
    await chooseWithKeyboard(
      user,
      /Sample label column$/,
      "1: Sample / Name (for example C00000123)",
    );
    await chooseWithKeyboard(user, /Spaces around values$/, "Keep values exactly as written");
    await chooseWithKeyboard(
      user,
      /End of data$/,
      "At the first blank line (the export has a footer)",
    );
    expect(await screen.findByText("2 analyte columns. Map each one.")).toBeTruthy();
    expect(screen.queryByRole("checkbox", { name: "Not a result" })).toBeNull();
    const morphine = screen.getByRole("group", { name: "Morphine Results" });
    await pick(user, within(morphine).getByRole("combobox"), "Synthetic morph");
    const fentanyl = screen.getByRole("group", { name: "Fentanyl Results" });
    await pick(user, within(fentanyl).getByRole("combobox"), "Synthetic fent");
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    await waitFor(() => expect(register).toHaveBeenCalled());
    expect(register).toHaveBeenCalledWith({
      input: expect.objectContaining({
        reportsFinalConcentration: false,
        layout: expect.objectContaining({
          encoding: "utf8",
          header: WIDE_LINES.slice(0, 2).map((line) => line.cells),
          shape: {
            kind: "wide",
            valueColumns: [
              { column: 4, analyteName: "Morphine Results" },
              { column: 6, analyteName: "Fentanyl Results" },
            ],
          },
          endOfData: "first_blank_line",
          trimFields: false,
        }),
        testCodes: [
          { sourceTestCode: "Morphine Results", analyteId: MORPHINE.id },
          { sourceTestCode: "Fentanyl Results", analyteId: FENTANYL.id },
        ],
      }),
    });
  });

  it("warns when the sample is not UTF-8, and says when the file dialog was cancelled", async () => {
    const open = vi
      .fn()
      .mockReturnValueOnce(null)
      .mockReturnValue({ ...LONG_SAMPLE, validUtf8: false });
    const { user } = renderPage({ open_layout_sample: open });
    await user.click(await screen.findByRole("button", { name: "Open a sample export" }));
    expect(
      await screen.findByText("The file dialog was cancelled. No file was opened."),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Open a sample export" }));
    expect(await screen.findByText("The sample is not valid UTF-8")).toBeTruthy();
    const raw = screen.getByText("First lines as written").closest("figure");
    expect(raw?.textContent).toContain("Exported batch SYN-01");
  });

  it("shows the engine's answer when the sample can no longer be read", async () => {
    const { user } = renderPage({
      preview_layout_sample: () =>
        Promise.reject(
          engineError("not_found", "The sample file is no longer open. Open it again."),
        ),
    });
    await user.click(await screen.findByRole("button", { name: "Open a sample export" }));
    await chooseWithKeyboard(user, /Encoding$/, "UTF-8");
    await chooseWithKeyboard(user, /Delimiter$/, "Comma");
    expect(
      await screen.findByText("The sample file is no longer open. Open it again."),
    ).toBeTruthy();
  });

  it("asks for a sample before saving, and shows the engine's refusal", async () => {
    const { user } = renderPage({
      register_file_source: () =>
        Promise.reject(engineError("validation", "Map every analyte column to a catalog analyte.")),
    });
    await describeSoftware(user, "Yes");
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(await screen.findByText("The template is not complete")).toBeTruthy();
    await describeLongFile(user);
    await screen.findByRole("group", { name: "Morphine" });
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(await screen.findByText("Map every analyte column to a catalog analyte.")).toBeTruthy();
    expect(screen.queryByText("The template is not complete")).toBeNull();
  });

  it("says when the sample cannot be read with the choices made", async () => {
    const { user } = renderPage({
      check_file_layout: () => ({ ...LONG_READING, failure: "limits_exceeded", rows: [] }),
    });
    await describeLongFile(user);
    expect(await screen.findByText("The sample cannot be read with these choices")).toBeTruthy();
    expect(screen.getByText(/50,000 rows, 512 columns/)).toBeTruthy();
  });

  it("tells people without manage_configuration that they cannot draft templates", async () => {
    renderPage({ current_session: () => sessionWith(["approve_configuration"]) });
    expect(await screen.findByText("You cannot draft file templates")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open a sample export" })).toBeNull();
  });

  it("shows loading, then says when the instrument is unknown", async () => {
    renderPage({ list_instrument_readiness: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "New file template" })).toBeTruthy();
    expect(screen.getByText("Loading the instrument…")).toBeTruthy();
    expect(await screen.findByText("Instrument not found")).toBeTruthy();
  });
});
