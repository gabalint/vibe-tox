import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { panel } from "../panelTest";
import { INSTRUMENT, sessionWith } from "../testData";
import { ImportAttemptPage } from "./ImportAttemptPage";

const ATTEMPT_ID = "00000000-0000-7000-8000-000000008301";
const RUN_A = "00000000-0000-7000-8000-000000007301";
const RUN_B = "00000000-0000-7000-8000-000000007302";
const SPECIMEN = "00000000-0000-7000-8000-000000006301";

const ROW: QuarantineItemView = {
  itemId: "item-1",
  recordPath: "R1",
  sampleLabel: "DUPTEST01",
  testCode: "AMP",
  valueText: "530.0",
  unitsText: "ng/mL",
  reason: "unknown_sample",
  state: "open",
  revision: 1,
  runId: null,
  specimenId: null,
  matchingRuns: [],
};

const DETAIL: ImportAttemptDetail = {
  importAttemptId: ATTEMPT_ID,
  receiptId: "00000000-0000-7000-8000-000000009301",
  attemptNo: 1,
  instrumentId: INSTRUMENT.instrumentId,
  outcome: "quarantined",
  failureCode: null,
  rowsTotal: 4,
  rowsNew: 0,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 0,
  rowsQuarantined: 4,
  rowsNotResult: 0,
  rowsSetAside: 0,
  rowsRetained: 0,
  processedAt: "2026-11-04T10:00:00Z",
  receivedAt: "2026-11-04T09:58:00Z",
  registrationId: null,
  file: null,
  setAside: [],
  quarantine: [
    ROW,
    {
      ...ROW,
      itemId: "item-2",
      recordPath: "R2",
      sampleLabel: "C00000002",
      reason: "ambiguous_sample",
      matchingRuns: [
        { runId: RUN_A, createdAt: "2026-11-03T08:00:00Z" },
        { runId: RUN_B, createdAt: "2026-11-04T08:00:00Z" },
      ],
    },
    {
      ...ROW,
      itemId: "item-3",
      recordPath: "R3",
      reason: "conflicting_duplicate",
      runId: RUN_A,
      specimenId: SPECIMEN,
    },
    { ...ROW, itemId: "item-4", recordPath: "R4", reason: "unknown_analyte", testCode: "XYZ" },
  ],
};

function renderAttempt(detail: ImportAttemptDetail, retry = vi.fn(() => 2)) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_instrument_readiness: () => [INSTRUMENT],
    get_import_attempt: () => detail,
    retry_import: retry,
  });
  const rendered = renderRoutes(
    [{ path: "/review/imports/:importAttemptId", element: <ImportAttemptPage /> }],
    `/review/imports/${ATTEMPT_ID}`,
  );
  return { ...rendered, retry };
}

/** The "Where to fix" links of the open row's panel, as label → address. */
function fixLinks(identifier: string) {
  const list = within(panel(identifier)).getByRole("list", { name: "Where to fix" });
  return within(list)
    .getAllByRole("link")
    .map((link) => [link.textContent, link.getAttribute("href")]);
}

describe("Where a quarantined row is fixed (REQ-ANA-003)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("links each reason to the screen that fixes it", async () => {
    const { user, container } = renderAttempt(DETAIL);
    await screen.findByRole("grid", { name: "Quarantined rows" });
    await openRow(user, "R1");
    expect(fixLinks("Row R1")).toEqual([
      ["Worklist", "/bench/worklist"],
      ["Open runs", "/bench/runs"],
      ["Container lookup for DUPTEST01", "/containers?labelCode=DUPTEST01"],
    ]);
    await expectNoA11yViolations(container, { wholePage: true });
    await openRow(user, "R2");
    expect(fixLinks("Row R2")).toEqual([
      ["Container lookup for C00000002", "/containers?labelCode=C00000002"],
      ["Run created 2026-11-03 08:00 UTC", `/bench/runs/${RUN_A}`],
      ["Run created 2026-11-04 08:00 UTC", `/bench/runs/${RUN_B}`],
    ]);
    await openRow(user, "R3");
    expect(fixLinks("Row R3")).toEqual([
      ["Existing result in Result review", `/review/specimens/${SPECIMEN}`],
    ]);
    await openRow(user, "R4");
    expect(fixLinks("Row R4")).toEqual([
      ["Instrument in Bench setup", `/bench/setup/instruments?open=${INSTRUMENT.instrumentId}`],
    ]);
    await user.click(
      within(panel("Row R4")).getByRole("link", { name: "Instrument in Bench setup" }),
    );
    expect(await screen.findByText(/Navigated to \/bench\/setup\/instruments/)).toBeTruthy();
  });

  it("links an unmapped code of a result file to the template that read it", async () => {
    const { user } = renderAttempt({
      ...DETAIL,
      registrationId: "registration-1",
      file: {
        origin: "upload",
        fileName: "SYN batch.csv",
        byteLen: 10,
        sha256: null,
        loadedBy: null,
      },
      quarantine: [DETAIL.quarantine[3] as QuarantineItemView],
    });
    await openRow(user, "R4");
    expect(fixLinks("Row R4")).toEqual([
      [
        "File template",
        `/bench/setup/instruments/${INSTRUMENT.instrumentId}/sources/registration-1`,
      ],
    ]);
  });

  it("links a refused message to its instrument, and offers Retry import on an open row", async () => {
    const { user, retry } = renderAttempt({
      ...DETAIL,
      failureCode: "unregistered_source",
      quarantine: [ROW],
    });
    const fix = await screen.findByRole("list", { name: "Where to fix" });
    expect(within(fix).getByRole("link", { name: "Instrument in Bench setup" })).toBeTruthy();
    const menu = await openRowActions(user, "R1");
    await user.click(within(menu).getByRole("menuitem", { name: "Retry import" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Retry import" });
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Source approved");
    await user.click(within(confirm).getByRole("button", { name: "Retry import" }));
    expect(
      await screen.findByText("Retry queued; it appears as a new import attempt"),
    ).toBeTruthy();
    expect(retry).toHaveBeenCalledWith({
      input: { receiptId: DETAIL.receiptId, reason: "Source approved" },
    });
  });
});
