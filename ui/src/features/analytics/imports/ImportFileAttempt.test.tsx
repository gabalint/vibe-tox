import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { FILE_INSTRUMENT, FILE_SOURCE } from "../fileTemplates/testTemplates";
import { sessionWith } from "../testData";
import { ImportAttemptPage } from "./ImportAttemptPage";
import { ImportAttemptsPage } from "./ImportAttemptsPage";

const ATTEMPT_ID = "00000000-0000-7000-8000-000000008201";
const USER_ID = "00000000-0000-7000-8000-000000000e01";

const FILE: NonNullable<ImportAttemptDetail["file"]> = {
  origin: "upload",
  fileName: "SYN-01 batch.csv",
  byteLen: 48213,
  sha256: "ab".repeat(32),
  loadedBy: USER_ID,
};

/** An attempt that read a synthetic result file (result files brief §7). */
const FILE_ATTEMPT: ImportAttemptDetail = {
  importAttemptId: ATTEMPT_ID,
  receiptId: "00000000-0000-7000-8000-000000009201",
  attemptNo: 1,
  instrumentId: FILE_INSTRUMENT.instrumentId,
  outcome: "processed_with_quarantine",
  failureCode: null,
  rowsTotal: 9,
  rowsNew: 2,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 1,
  rowsQuarantined: 1,
  rowsNotResult: 1,
  rowsSetAside: 2,
  rowsRetained: 2,
  processedAt: "2026-10-13T10:00:02Z",
  registrationId: FILE_SOURCE.registrationId,
  file: FILE,
  quarantine: [
    {
      itemId: "item-1",
      recordPath: "L5",
      sampleLabel: "C00000123",
      testCode: "Fentanyl",
      valueText: "",
      unitsText: "ng/mL",
      reason: "malformed_value",
      state: "open",
      revision: 1,
    },
  ],
  setAside: [
    {
      rowId: "row-1",
      recordPath: "L7",
      sampleLabel: "CAL-1",
      testCode: "Morphine",
      valueText: "10.2",
      typeWord: "Standard",
      kind: "batch_row_not_placed",
      reason: null,
    },
    {
      rowId: "row-2",
      recordPath: "L8",
      sampleLabel: "C00000123",
      testCode: "Oxycodone",
      valueText: "12.0",
      typeWord: "Unknown",
      kind: "not_ordered_unusable",
      reason: "incompatible_unit",
    },
  ],
};

function renderPage(path: string, attempt: ImportAttemptDetail = FILE_ATTEMPT) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    get_import_attempt: () => attempt,
    list_source_registrations: () => [FILE_SOURCE],
  });
  return renderRoutes(
    [{ path: "/review/imports/:importAttemptId/:tab?", element: <ImportAttemptPage /> }],
    path,
  );
}

describe("Import attempt of a result file (result files brief §7)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("shows the file, the template used and the file counts", async () => {
    const { container, user, router } = renderPage(`/review/imports/${ATTEMPT_ID}`);
    expect(await screen.findByText("SYN-01 batch.csv")).toBeTruthy();
    // The signed-in person loaded it, named like anyone else.
    expect(await screen.findByText("Loaded by hand by Test Analyst")).toBeTruthy();
    expect(screen.getByText("48,213 bytes")).toBeTruthy();
    expect(screen.getByText("ab".repeat(32))).toBeTruthy();
    const facts = screen.getByRole("region", { name: "Outcome" });
    expect(within(facts).getByText("Measured, not ordered (kept)").nextSibling?.textContent).toBe(
      "2",
    );
    expect(within(facts).getByText("Set aside").nextSibling?.textContent).toBe("2");
    expect(within(facts).getByText("Not results").nextSibling?.textContent).toBe("1");
    expect(await screen.findByRole("grid", { name: "Quarantined rows" })).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(await screen.findByRole("link", { name: "Synthetic Quant 12.1" }));
    expect(router.state.location.pathname).toBe(
      `/bench/setup/instruments/${FILE_INSTRUMENT.instrumentId}/sources/${FILE_SOURCE.registrationId}`,
    );
  });

  it("lists the rows set aside on their own tab, with why", async () => {
    const { user, router } = renderPage(`/review/imports/${ATTEMPT_ID}`);
    await user.click(await screen.findByRole("tab", { name: "Set aside (2)" }));
    expect(router.state.location.pathname).toBe(`/review/imports/${ATTEMPT_ID}/set-aside`);
    const rows = await screen.findByRole("grid", { name: "Rows set aside" });
    expect(
      within(rows).getByRole("gridcell", {
        name: "Calibrator or blank row of the batch, not on a run",
      }),
    ).toBeTruthy();
    expect(
      within(rows).getByRole("gridcell", {
        name: "Measured, not ordered, not kept: The units differ from the method's units",
      }),
    ).toBeTruthy();
  });

  it("says why a whole file failed, what to do next, and that it came from the folder", async () => {
    const failed: ImportAttemptDetail = {
      ...FILE_ATTEMPT,
      outcome: "quarantined",
      failureCode: "layout_mismatch",
      file: { ...FILE, origin: "folder", loadedBy: null },
      quarantine: [],
      setAside: [],
    };
    renderPage(`/review/imports/${ATTEMPT_ID}/set-aside`, failed);
    expect(
      await screen.findByText(
        "The file's columns differ from the approved template. Fix the export or approve a new template version, then use Retry import.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("Picked up from the watched folder")).toBeTruthy();
    expect(screen.getByText("No rows were set aside.")).toBeTruthy();
  });

  it("names the person who loaded the file", async () => {
    renderPage(`/review/imports/${ATTEMPT_ID}`, {
      ...FILE_ATTEMPT,
      file: {
        ...FILE,
        loadedBy: "00000000-0000-7000-8000-000000000e99",
        loadedByDisplay: "Test Analyst 2",
      },
    });
    expect(await screen.findByText("Loaded by hand by Test Analyst 2")).toBeTruthy();
  });

  it("says on the list how each attempt's data arrived", async () => {
    mockCommands({
      current_session: () => sessionWith(["review_results"]),
      list_instrument_readiness: () => [FILE_INSTRUMENT],
      list_import_attempts: () => [
        { ...FILE_ATTEMPT, importAttemptId: "a-1", origin: "upload" },
        { ...FILE_ATTEMPT, importAttemptId: "a-2", origin: "folder" },
        { ...FILE_ATTEMPT, importAttemptId: "a-3", origin: undefined },
      ],
    });
    renderRoutes([{ path: "/review/imports", element: <ImportAttemptsPage /> }], "/review/imports");
    const grid = await screen.findByRole("grid", { name: "Imports" });
    expect(within(grid).getByRole("gridcell", { name: "File loaded by hand" })).toBeTruthy();
    expect(
      within(grid).getByRole("gridcell", { name: "File from the watched folder" }),
    ).toBeTruthy();
    expect(within(grid).getByRole("gridcell", { name: "Analyzer message" })).toBeTruthy();
  });

  it("keeps an analyzer message's attempt without file facts or tabs", async () => {
    renderPage(`/review/imports/${ATTEMPT_ID}`, {
      ...FILE_ATTEMPT,
      file: null,
      registrationId: null,
      setAside: [],
    });
    expect(await screen.findByText("Message failure")).toBeTruthy();
    expect(screen.queryByRole("tab")).toBeNull();
    expect(screen.queryByText("Set aside")).toBeNull();
  });
});
