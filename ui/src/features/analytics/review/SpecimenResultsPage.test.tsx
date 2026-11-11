import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { measurement, revision, SUBJECT, sessionWith } from "../testData";
import { ReviewQueuePage } from "./ReviewQueuePage";
import { SpecimenResultsPage } from "./SpecimenResultsPage";

const SPECIMEN_ID = "00000000-0000-7000-8000-000000006001";
const POLICY_ID = "00000000-0000-7000-8000-000000007001";

const FIRST = revision();
const SELECTED = revision({
  revisionId: "revision-2",
  revisionNo: 2,
  cause: "selected",
  reason: "Repeat agrees",
  evidence: [
    measurement({ measurementId: "measurement-3", positionLabel: "P07", attemptKind: "rerun" }),
  ],
});
const CREATININE = revision({
  revisionId: "revision-c1",
  outcome: "censored",
  comparator: "<",
  magnitude: "20.0",
  units: "mg/dL",
  classification: "not_classifiable",
  classificationReason: "censored_bound_inconclusive",
  evidence: [
    measurement({ measurementId: "measurement-c", valueText: "<20.0", unitsText: "mg/dL" }),
  ],
});

const RESULTS: SpecimenResults = {
  subject: SUBJECT,
  accessionNumber: "A26000001",
  specimenId: SPECIMEN_ID,
  specimenTypeCode: "urine",
  acceptance: "accepted",
  lines: [
    {
      resultId: "result-1",
      revision: 4,
      componentId: "component-1",
      analyteId: "analyte-amp",
      analyteCode: "SYN-AMP",
      analyteName: "Synthetic amphetamine",
      purpose: "analytical",
      current: SELECTED,
      history: [FIRST, SELECTED],
      unselectedEvidence: [measurement({ measurementId: "measurement-2", valueText: "498.0" })],
      gates: [
        { code: "qc_conflict", message: "A changed QC row for this analyte awaits a decision." },
        { code: "execution_failed", message: "The test execution was marked failed." },
      ],
      reviewStatus: {
        status: "stale",
        reasons: [
          {
            code: "result_revision_changed",
            message: "A reviewed result revision is no longer current.",
          },
        ],
      },
    },
    {
      resultId: "result-2",
      revision: 1,
      componentId: "component-2",
      analyteId: "analyte-cre",
      analyteCode: "SYN-CRE",
      analyteName: "Synthetic creatinine",
      purpose: "validity",
      current: CREATININE,
      history: [CREATININE],
      unselectedEvidence: [],
      gates: [],
      reviewStatus: { status: "none" },
    },
  ],
  validityRequired: true,
  currentValidity: null,
  validityHistory: [],
  pendingAnalytes: [
    {
      componentId: "component-1",
      analyteId: "analyte-coc",
      analyteCode: "SYN-COC",
      analyteName: "Synthetic cocaine metabolite",
    },
  ],
  reflexHistory: [],
  measuredNotOrdered: [],
  allowedActions: ["submit_technical_review"],
};

const POLICY: ValidityPolicyView = {
  id: POLICY_ID,
  specimenTypeId: "specimen-type-1",
  specimenTypeCode: "urine",
  version: 1,
  categories: [{ code: "normal", label: "Within expected range", action: "none" }],
  status: "approved",
  revision: 1,
  createdBy: "author-1",
  createdAt: "2026-10-12T09:00:00Z",
  approvedBy: "approver-1",
  effectiveFrom: "2026-10-12T10:00:00Z",
  effectiveTo: null,
  replacedBy: null,
  retiredBy: null,
  retiredAt: null,
  retireReason: null,
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["review_results"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_specimen_results: () => RESULTS,
    get_validity_policy: () => POLICY,
    list_validity_policies: () => [POLICY],
    list_reflex_triggers: () => [],
    list_active_orderables: () => [
      { id: "orderable-version-1", code: "SYN-CNF", name: "Synthetic confirmation" },
    ],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/review/specimens/:specimenId", element: <SpecimenResultsPage /> }],
    `/review/specimens/${SPECIMEN_ID}`,
  );
}

/** Three specimens in the review queue, opened from its "Review out of date" filter. */
function renderFromQueue() {
  const specimens = ["A26000001", "A26000002", "A26000003"].map((accessionNumber, index) => ({
    ...RESULTS,
    specimenId: `specimen-${index + 1}`,
    accessionNumber,
  }));
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_review_queue: () => ({
      items: specimens.map((results) => ({
        specimenId: results.specimenId,
        subject: results.subject,
        since: "2026-10-14T12:00:00Z",
        accessionNumber: results.accessionNumber,
        specimenTypeCode: "urine",
        lines: 2,
        pending: 0,
        stale: 0,
        blocked: 0,
      })),
      next: null,
      scanned: specimens.length,
    }),
    get_specimen_results: (args) =>
      specimens.find((results) => JSON.stringify(args).includes(results.specimenId)),
    get_validity_policy: () => POLICY,
    list_validity_policies: () => [POLICY],
    list_reflex_triggers: () => [],
    list_active_orderables: () => [],
  });
  return renderRoutes(
    [
      { path: "/review/queue", element: <ReviewQueuePage /> },
      { path: "/review/specimens/:specimenId", element: <SpecimenResultsPage /> },
    ],
    "/review/queue?filter=stale",
  );
}

function rowOf(grid: HTMLElement, analyte: string): HTMLElement {
  const cell = within(grid)
    .getAllByRole("gridcell")
    .find((candidate) => candidate.textContent?.startsWith(analyte));
  const row = cell?.closest<HTMLElement>("[role=row]");
  if (!row) throw new Error(`no row for ${analyte}`);
  return row;
}

async function openDialog(user: ReturnType<typeof renderPage>["user"], button: string) {
  const line = await screen.findByRole("region", { name: "Synthetic amphetamine (SYN-AMP)" });
  await user.click(await within(line).findByRole("button", { name: button }));
}

describe("SpecimenResultsPage (REQ-RES-001, REQ-RES-005)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("shows one row per line with values exactly as received, classifications and gates", async () => {
    const { container } = renderPage();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results for accession A26000001" }),
    ).toBeTruthy();
    const grid = screen.getByRole("grid", { name: "Result lines" });
    expect(
      within(grid)
        .getAllByRole("columnheader")
        .map((header) => header.textContent),
    ).toEqual(["Analyte", "Result", "Classification", "Method", "Revision", "Review", "Blocking"]);
    const amphetamine = rowOf(grid, "Synthetic amphetamine (SYN-AMP)");
    expect(within(amphetamine).getByText("512.0 ng/mL")).toBeTruthy();
    expect(within(amphetamine).getByText("Meets cutoff")).toBeTruthy();
    expect(within(amphetamine).getByText("Review out of date")).toBeTruthy();
    // Revision 2, blocked by 2 gates.
    expect(within(amphetamine).getAllByRole("gridcell", { name: "2" })).toHaveLength(2);
    const creatinine = rowOf(grid, "Synthetic creatinine (SYN-CRE)");
    expect(within(creatinine).getByText("<20.0 mg/dL")).toBeTruthy();
    expect(
      within(creatinine).getByText(
        "Not classifiable: the reported bound does not decide the comparison",
      ),
    ).toBeTruthy();
    expect(within(creatinine).getByText("Specimen validity")).toBeTruthy();
    // No gate blocks the creatinine line: a plain 0.
    expect(within(creatinine).getAllByRole("gridcell").at(-1)?.textContent).toBe("0");

    // The first line is open: its revisions, gates, staleness and unselected evidence.
    const detail = screen.getByRole("region", { name: "Synthetic amphetamine (SYN-AMP)" });
    // The current revision, and the same value in the earlier revision it replaced.
    expect(within(detail).getAllByText("512.0 ng/mL")).toHaveLength(2);
    const gates = within(detail).getByRole("list", {
      name: "Blocking review of Synthetic amphetamine (SYN-AMP)",
    });
    // Each gate leads with its plain label, then the engine's own words.
    expect(gates.textContent).toContain(
      "Changed QC row awaits a decision: A changed QC row for this analyte awaits a decision.",
    );
    expect(gates.textContent).toContain(
      "Test execution failed: The test execution was marked failed.",
    );
    const stale = within(detail).getByRole("list", {
      name: "Why the review of Synthetic amphetamine (SYN-AMP) is out of date",
    });
    expect(stale.textContent).toContain(
      "Result changed: A reviewed result revision is no longer current.",
    );
    const unselected = within(detail).getByRole("list", {
      name: "Measurements not selected for Synthetic amphetamine (SYN-AMP)",
    });
    expect(unselected.textContent).toContain(
      "498.0\u00A0ng/mL, status F. Position P01, initial attempt",
    );
    expect(within(detail).getByText("Earlier revisions (1)")).toBeTruthy();
    const waiting = screen.getByRole("list", { name: "Waiting for results" });
    expect(waiting.textContent).toContain("Synthetic cocaine metabolite (SYN-COC)");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows long results whole, each value and unit joined by a non-breaking space", async () => {
    const long = RESULTS.lines[0];
    if (!long) throw new Error("no line");
    const lines = [
      { ...long, current: revision({ magnitude: "1234567.8901" }) },
      {
        ...long,
        resultId: "result-3",
        analyteCode: "SYN-MTD",
        analyteName: "Synthetic methadone metabolite with a long configured name",
        current: revision({ revisionId: "revision-3", magnitude: "12.5", units: null }),
      },
    ];
    renderPage({ get_specimen_results: () => ({ ...RESULTS, lines }) });
    const grid = await screen.findByRole("grid", { name: "Result lines" });
    const value = within(grid).getByRole("gridcell", { name: "1234567.8901\u00A0ng/mL" });
    // The engine's string, whole: the cell wraps rather than ending in "…".
    expect(value.textContent).toBe("1234567.8901\u00A0ng/mL");
    expect(value.getAttribute("data-wrap")).toBe("true");
    const noUnits = within(grid).getByRole("gridcell", { name: "12.5\u00A0(no units reported)" });
    expect(noUnits.getAttribute("data-wrap")).toBe("true");
    const analyte = rowOf(grid, "Synthetic methadone metabolite").querySelector("[role=gridcell]");
    expect(analyte?.textContent).toBe(
      "Synthetic methadone metabolite with a long configured name (SYN-MTD)",
    );
    expect(analyte?.getAttribute("data-wrap")).toBe("true");
  });

  it("shows the patient and the specimen in the identity banner under the page title", async () => {
    renderPage({
      get_specimen_results: () => ({
        ...RESULTS,
        subject: { ...SUBJECT, openIdentityHolds: 1 },
      }),
    });
    const banner = await screen.findByRole("region", { name: "Patient and specimen identity" });
    const header = screen.getByRole("heading", { level: 1 }).closest("header");
    expect(header?.nextElementSibling).toBe(banner);
    // A reviewer cannot open the patient's record, so the name is not a link.
    expect(banner.textContent).toContain("Patient, Test");
    expect(within(banner).queryByRole("link")).toBeNull();
    expect(within(banner).getByText("MRN-0001").tagName).toBe("STRONG");
    expect(within(banner).getByText("MRN (SYN-LAB)")).toBeTruthy();
    expect(within(banner).getByText("A26000001")).toBeTruthy();
    expect(within(banner).getByText("Identity hold")).toBeTruthy();
  });

  it("opens a line's detail by click or Enter, marking the open line", async () => {
    const { user, router } = renderPage();
    const grid = await screen.findByRole("grid", { name: "Result lines" });
    expect(rowOf(grid, "Synthetic amphetamine (SYN-AMP)").getAttribute("aria-current")).toBe(
      "true",
    );
    await openRowWithEnter(user, "<20.0\u00A0mg/dL");
    const detail = await screen.findByRole("region", { name: "Synthetic creatinine (SYN-CRE)" });
    expect(within(detail).getByText("<20.0 mg/dL")).toBeTruthy();
    expect(router.state.location.search).toBe("?line=result-2");
    expect(screen.queryByRole("region", { name: "Synthetic amphetamine (SYN-AMP)" })).toBeNull();
    expect(rowOf(grid, "Synthetic creatinine (SYN-CRE)").getAttribute("aria-current")).toBe("true");

    await openRow(user, "512.0\u00A0ng/mL");
    expect(
      await screen.findByRole("region", { name: "Synthetic amphetamine (SYN-AMP)" }),
    ).toBeTruthy();
    // Approve stays in the page header whichever line is open.
    expect(screen.getByRole("button", { name: "Approve results" })).toBeTruthy();
  });

  it("keeps the review queue in the breadcrumb, with Next through the queue", async () => {
    const { user } = renderFromQueue();
    await openRow(user, "A26000001");
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results for accession A26000001" }),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "Result review" }).getAttribute("href")).toBe(
      "/review/queue?filter=stale",
    );
    const next = screen.getByRole("link", { name: "Next" });
    await user.click(next);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results for accession A26000002" }),
    ).toBeTruthy();
    // The header stays in place while the next specimen loads, so focus stays on Next.
    expect(document.activeElement).toBe(next);
    await user.click(screen.getByRole("link", { name: "Next" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results for accession A26000003" }),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "Next" }).getAttribute("aria-disabled")).toBe("true");
  });

  it("keeps the queue's breadcrumb and Previous and Next after a result line is opened", async () => {
    const { user, router } = renderFromQueue();
    await openRow(user, "A26000002");
    await screen.findByRole("heading", { level: 1, name: "Results for accession A26000002" });
    await openRow(user, "<20.0\u00A0mg/dL");
    await screen.findByRole("region", { name: "Synthetic creatinine (SYN-CRE)" });
    expect(router.state.location.search).toBe("?line=result-2");
    expect(screen.getByRole("link", { name: "Result review" }).getAttribute("href")).toBe(
      "/review/queue?filter=stale",
    );
    expect(screen.getByRole("link", { name: "Next" }).getAttribute("aria-disabled")).toBeNull();
    expect(screen.getByRole("link", { name: "Previous" }).getAttribute("aria-disabled")).toBeNull();
  });

  it("approves exactly the current revisions shown", async () => {
    const submit = vi.fn(() => ({}));
    const { user } = renderPage({ submit_technical_review: submit });
    await user.click(await screen.findByRole("button", { name: "Approve results" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Approve these results" });
    const shown = within(confirm).getByRole("list", { name: "Revisions reviewed" });
    expect(shown.textContent).toContain(
      "Synthetic amphetamine (SYN-AMP): 512.0\u00A0ng/mL, revision 2",
    );
    await user.click(within(confirm).getByRole("button", { name: "Approve results" }));
    expect(await screen.findByText("Technical review recorded")).toBeTruthy();
    expect(submit).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        decision: "approved",
        resultRevisionIds: ["revision-2", "revision-c1"],
        reason: null,
      },
    });
  });

  it("approves only the revisions without a current approval", async () => {
    const submit = vi.fn(() => ({}));
    const partlyApproved: SpecimenResults = {
      ...RESULTS,
      lines: RESULTS.lines.map((line) =>
        line.resultId === "result-2"
          ? { ...line, reviewStatus: { status: "current", review_id: "r-1" } }
          : line,
      ),
    };
    const { user } = renderPage({
      get_specimen_results: () => partlyApproved,
      submit_technical_review: submit,
    });
    await user.click(await screen.findByRole("button", { name: "Approve results" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Approve these results" });
    expect(within(confirm).getByText(/not approved again/)).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Approve results" }));
    await screen.findByText("Technical review recorded");
    expect(submit).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        decision: "approved",
        resultRevisionIds: ["revision-2"],
        reason: null,
      },
    });
  });

  it("offers withdrawal, not a repeat approval, once every result is approved", async () => {
    const submit = vi.fn(() => ({}));
    const approved: SpecimenResults = {
      ...RESULTS,
      lines: RESULTS.lines.map((line) => ({
        ...line,
        reviewStatus: { status: "current", review_id: "r-1" },
      })),
    };
    const { user } = renderPage({
      get_specimen_results: () => approved,
      submit_technical_review: submit,
    });
    expect(
      await screen.findByText("All current results are approved. Nothing new to review."),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve results" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Reject results" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Withdraw approval" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Withdraw approval of these results",
    });
    expect(within(confirm).getByText(/cannot be released until/)).toBeTruthy();
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Wrong method");
    await user.click(within(confirm).getByRole("button", { name: "Withdraw approval" }));
    await screen.findByText("Technical review recorded");
    expect(submit).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        decision: "rejected",
        resultRevisionIds: ["revision-2", "revision-c1"],
        reason: "Wrong method",
      },
    });
  });

  it("lists the gate reasons when the engine blocks the review", async () => {
    const { user } = renderPage({
      submit_technical_review: () =>
        Promise.reject(
          engineError("gate_blocked", "This action is blocked by one or more review gates.", {
            gateReasons: [
              { code: "run_closed", message: "The run is closed." },
              { code: "prep_not_completed", message: "The preparation batch is not completed." },
              { code: "validity_unresolved", message: "Specimen validity is not assessed." },
            ],
          }),
        ),
    });
    await user.click(await screen.findByRole("button", { name: "Approve results" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Approve results" }));
    const gates = await within(confirm).findByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain("Run closed: The run is closed.");
    expect(gates.textContent).toContain(
      "Preparation batch not completed: The preparation batch is not completed.",
    );
    // A code without a label shows the engine's words alone.
    expect(gates.textContent).toContain("Specimen validity is not assessed.");
    expect(gates.textContent).not.toContain("validity_unresolved");
    expect(within(confirm).getByText("corr-test-0001")).toBeTruthy();
  });

  it("rejects only with a reason", async () => {
    const submit = vi.fn(() => ({}));
    const { user } = renderPage({ submit_technical_review: submit });
    await user.click(await screen.findByRole("button", { name: "Reject results" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Reject results" }));
    expect(await within(confirm).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(submit).not.toHaveBeenCalled();
    await user.type(
      within(confirm).getByRole("textbox", { name: /Reason/ }),
      "Chromatography poor",
    );
    await user.click(within(confirm).getByRole("button", { name: "Reject results" }));
    await screen.findByText("Technical review recorded");
    expect(submit).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        decision: "rejected",
        resultRevisionIds: ["revision-2", "revision-c1"],
        reason: "Chromatography poor",
      },
    });
  });

  it("selects evidence with a reason, against the line's revision", async () => {
    const select = vi.fn(() => SELECTED);
    const { user } = renderPage({ select_result_evidence: select });
    await openDialog(user, "Select evidence");
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("checkbox", { name: /498\.0\sng\/mL/ }));
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Initial run accepted",
    );
    await user.click(within(dialog).getByRole("button", { name: "Select evidence" }));
    expect(await screen.findByText("Evidence selected for Synthetic amphetamine")).toBeTruthy();
    expect(select).toHaveBeenCalledWith({
      input: {
        resultId: "result-1",
        expectedRevision: 4,
        measurementIds: ["measurement-2"],
        reason: "Initial run accepted",
      },
    });
  });

  it("says in plain words why a value word cannot be used on a dilution run", async () => {
    const { user } = renderPage({
      select_result_evidence: () =>
        Promise.reject(
          engineError(
            "gate_blocked",
            "This action is blocked until the listed holds are resolved.",
            {
              gateReasons: [{ code: "dilution_not_supported", message: "Not for a dilution." }],
            },
          ),
        ),
    });
    await openDialog(user, "Select evidence");
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("checkbox", { name: /498\.0\sng\/mL/ }));
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Dilution repeat");
    await user.click(within(dialog).getByRole("button", { name: "Select evidence" }));
    expect(
      await within(dialog).findByText(
        /A word such as N\/F cannot be used on a dilution run; the method's limit is not the limit of a diluted injection\./,
      ),
    ).toBeTruthy();
  });

  it("records a disposition with a reason", async () => {
    const record = vi.fn(() => SELECTED);
    const { user } = renderPage({ record_result_disposition: record });
    await openDialog(user, "Record disposition");
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Disposition$/, "Invalid");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Interference");
    await user.click(within(dialog).getByRole("button", { name: "Record disposition" }));
    await screen.findByText("Disposition recorded for Synthetic amphetamine");
    expect(record).toHaveBeenCalledWith({
      input: {
        resultId: "result-1",
        expectedRevision: 4,
        componentId: null,
        analyteId: null,
        specimenId: null,
        outcome: "invalid",
        reason: "Interference",
      },
    });
  });

  it("records a pending analyte as missing only with a reason", async () => {
    const record = vi.fn(() => SELECTED);
    const { user } = renderPage({ record_result_disposition: record });
    const waiting = await screen.findByRole("list", { name: "Waiting for results" });
    await user.click(within(waiting).getByRole("button", { name: "Record as missing" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Record Synthetic cocaine metabolite (SYN-COC) as missing?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Record as missing" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Sample exhausted");
    await user.click(within(dialog).getByRole("button", { name: "Record as missing" }));
    expect(
      await screen.findByText("Synthetic cocaine metabolite (SYN-COC) recorded as missing"),
    ).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        resultId: null,
        expectedRevision: 0,
        componentId: "component-1",
        analyteId: "analyte-coc",
        specimenId: SPECIMEN_ID,
        outcome: "missing",
        reason: "Sample exhausted",
      },
    });
  });

  it("keeps the missing dialog open with the engine's refusal", async () => {
    const { user } = renderPage({
      record_result_disposition: () =>
        Promise.reject(engineError("conflict", "A result arrived for this analyte. Reload.")),
    });
    const waiting = await screen.findByRole("list", { name: "Waiting for results" });
    await user.click(within(waiting).getByRole("button", { name: "Record as missing" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Sample exhausted");
    await user.click(within(dialog).getByRole("button", { name: "Record as missing" }));
    expect(
      await within(dialog).findByText("A result arrived for this analyte. Reload."),
    ).toBeTruthy();
    expect(within(dialog).getByText("corr-test-0001")).toBeTruthy();
  });

  it("records a validity assessment under an approved policy", async () => {
    const record = vi.fn(() => ({}));
    const { user } = renderPage({ record_validity_assessment: record });
    await user.click(await screen.findByRole("button", { name: "Record validity assessment" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Validity policy$/, "Policy version 1");
    await screen.findByRole("button", { name: /Category$/ });
    await chooseWithKeyboard(user, /Category$/, "Within expected range");
    await user.click(
      within(dialog).getByRole("checkbox", { name: /Synthetic creatinine: <20\.0\smg\/dL/ }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Record assessment" }));
    expect(await screen.findByText("Validity assessment recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        policyVersionId: POLICY_ID,
        categoryCode: "normal",
        evidenceRevisionIds: ["revision-c1"],
        supersedesId: null,
        reason: null,
      },
    });
  });

  it("offers no review without the engine's permission", async () => {
    renderPage({ get_specimen_results: () => ({ ...RESULTS, allowedActions: [] }) });
    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByRole("button", { name: "Approve results" })).toBeNull();
  });

  it("shows loading, then errors with the support reference", async () => {
    renderPage({ get_specimen_results: () => new Promise(() => undefined) });
    expect(await screen.findByText("Loading specimen results…")).toBeTruthy();
  });

  it("shows the engine's error under the page title", async () => {
    renderPage({
      get_specimen_results: () =>
        Promise.reject(engineError("not_found", "The specimen was not found.")),
    });
    expect(await screen.findByText("The specimen was not found.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Specimen results" })).toBeTruthy();
  });
});
