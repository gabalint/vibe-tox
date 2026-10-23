import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ReflexTriggerView } from "@/generated/ReflexTriggerView";
import type { ResultLineView } from "@/generated/ResultLineView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { revision, SUBJECT, sessionWith } from "../testData";
import { SpecimenResultsPage } from "./SpecimenResultsPage";

const SPECIMEN_ID = "00000000-0000-7000-8000-000000006101";
const MORPHINE = revision({ revisionId: "revision-mor", magnitude: "152.40" });
const OXYCODONE = revision({
  revisionId: "revision-oxy",
  outcome: "censored",
  comparator: "<",
  magnitude: "5.0",
  classification: "does_not_meet_cutoff",
});

function line(code: string, name: string, current: typeof MORPHINE): ResultLineView {
  return {
    resultId: `result-${code}`,
    revision: 1,
    componentId: "component-1",
    analyteId: `analyte-${code}`,
    analyteCode: code,
    analyteName: name,
    purpose: "analytical",
    current,
    history: [current],
    unselectedEvidence: [],
    gates: [],
    reviewStatus: { status: "current", review_id: "review-1" },
  };
}

const RESULTS: SpecimenResults = {
  subject: SUBJECT,
  accessionNumber: "A26000002",
  specimenId: SPECIMEN_ID,
  specimenTypeCode: "urine",
  acceptance: "accepted",
  lines: [
    line("SYN-MOR", "Synthetic morphine", MORPHINE),
    line("SYN-OXY", "Synthetic oxycodone", OXYCODONE),
  ],
  validityRequired: false,
  currentValidity: null,
  validityHistory: [],
  pendingAnalytes: [],
  reflexHistory: [],
  measuredNotOrdered: [],
  allowedActions: [],
};

const TRIGGER: ReflexTriggerView = {
  parentItemId: "item-1",
  orderableId: "orderable-cnf",
  orderableCode: "SYN-CNF",
  orderableName: "Synthetic confirmation",
  evidenceRevisionIds: ["revision-mor", "revision-oxy"],
  allowedActions: ["record_reflex_decision"],
};

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    get_specimen_results: () => RESULTS,
    list_reflex_triggers: () => [TRIGGER],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/review/specimens/:specimenId", element: <SpecimenResultsPage /> }],
    `/review/specimens/${SPECIMEN_ID}`,
  );
}

async function openDecision(user: ReturnType<typeof renderPage>["user"]) {
  const pending = await screen.findByRole("list", { name: "Reflex decisions pending" });
  expect(pending.textContent).toContain("Synthetic confirmation (SYN-CNF): decision pending");
  await user.click(within(pending).getByRole("button", { name: "Record reflex decision" }));
  return screen.findByRole("dialog", {
    name: "Reflex decision for Synthetic confirmation (SYN-CNF)",
  });
}

describe("Reflex decisions (REQ-ORD-003, REQ-RPT-005)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  it("records 'met' on a chosen result shown read-only, with the role entered", async () => {
    const record = vi.fn(() => ({}));
    const { user, container } = renderPage({ record_reflex_decision: record });
    const dialog = await openDecision(user);
    // The engine's evidence is offered; nothing is preselected and no role is asked yet.
    expect(within(dialog).queryByText("Classification")).toBeNull();
    expect(within(dialog).queryByRole("button", { name: /Testing sequence role/ })).toBeNull();
    await chooseWithKeyboard(user, /Result the decision rests on$/, "Synthetic morphine (SYN-MOR)");
    expect(within(dialog).getByText("152.40 ng/mL")).toBeTruthy();
    expect(within(dialog).getByText("Meets cutoff")).toBeTruthy();
    await chooseWithKeyboard(user, /Reflex condition$/, "Condition met: request the reflex test");
    await chooseWithKeyboard(user, /Testing sequence role$/, "Confirmation");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Screen positive");
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(within(dialog).getByRole("button", { name: "Record reflex decision" }));
    expect(await screen.findByText("Reflex decision recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        parentItemId: "item-1",
        orderableId: "orderable-cnf",
        resultRevisionId: "revision-mor",
        decision: "condition_met",
        sequenceRole: "confirmation",
        reason: "Screen positive",
      },
    });
  });

  it("records 'not met' without a role, and only with a reason", async () => {
    const record = vi.fn(() => ({}));
    const { user } = renderPage({ record_reflex_decision: record });
    const dialog = await openDecision(user);
    await chooseWithKeyboard(
      user,
      /Result the decision rests on$/,
      "Synthetic oxycodone (SYN-OXY)",
    );
    expect(within(dialog).getByText("<5.0 ng/mL")).toBeTruthy();
    await chooseWithKeyboard(user, /Reflex condition$/, "Condition not met: no reflex test");
    expect(within(dialog).queryByRole("button", { name: /Testing sequence role/ })).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Record reflex decision" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Screen negative");
    await user.click(within(dialog).getByRole("button", { name: "Record reflex decision" }));
    await screen.findByText("Reflex decision recorded");
    expect(record).toHaveBeenCalledWith({
      input: {
        parentItemId: "item-1",
        orderableId: "orderable-cnf",
        resultRevisionId: "revision-oxy",
        decision: "condition_not_met",
        sequenceRole: null,
        reason: "Screen negative",
      },
    });
  });

  it("keeps the dialog open with the engine's refusal", async () => {
    const { user } = renderPage({
      record_reflex_decision: () =>
        Promise.reject(engineError("conflict", "A different reflex decision is recorded.")),
    });
    const dialog = await openDecision(user);
    await chooseWithKeyboard(user, /Result the decision rests on$/, "Synthetic morphine (SYN-MOR)");
    await chooseWithKeyboard(user, /Reflex condition$/, "Condition not met: no reflex test");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Screen negative");
    await user.click(within(dialog).getByRole("button", { name: "Record reflex decision" }));
    expect(
      await within(dialog).findByText("A different reflex decision is recorded."),
    ).toBeTruthy();
  });

  it("offers no decision until the engine allows one", async () => {
    const { container } = renderPage({
      list_reflex_triggers: () => [{ ...TRIGGER, evidenceRevisionIds: [], allowedActions: [] }],
    });
    const pending = await screen.findByRole("list", { name: "Reflex decisions pending" });
    expect(within(pending).queryByRole("button")).toBeNull();
    expect(
      within(pending).getByText("Approve a result in technical review before deciding."),
    ).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("says when no decision is pending", async () => {
    renderPage({ list_reflex_triggers: () => [] });
    expect(await screen.findByText("No reflex decision is pending.")).toBeTruthy();
  });
  it("retains completed decisions with evidence, actor, time and subsequent work", async () => {
    mockGridLayout();
    renderPage({
      list_reflex_triggers: () => [],
      get_specimen_results: () => ({
        ...RESULTS,
        reflexHistory: [
          {
            orderableCode: "SYN-CNF",
            orderableName: "Synthetic confirmation",
            createdItemState: "cancelled",
            assessment: {
              assessmentId: "assessment-completed",
              parentItemId: "item-1",
              orderableId: "orderable-cnf",
              outcome: "met",
              triggerSource: "result_revision",
              triggerRef: MORPHINE.revisionId,
              createdItemId: "follow-on-1",
              reason: "Synthetic approved condition met",
              decidedBy: "reviewer-1",
              decidedByDisplay: "Synthetic reviewer",
              decidedAt: "2026-10-12T10:00:00Z",
            },
          },
        ],
      }),
    });
    const grid = await screen.findByRole("grid", { name: "Completed reflex decisions" });
    expect(within(grid).getByText("Synthetic approved condition met")).toBeTruthy();
    expect(within(grid).getByText(/result revision/)).toBeTruthy();
    expect(within(grid).getByText(/by Synthetic reviewer/)).toBeTruthy();
    expect(within(grid).queryByText(/reviewer-1/)).toBeNull();
    expect(within(grid).getByText("2026-10-12 10:00 UTC")).toBeTruthy();
    expect(within(grid).getByText("cancelled")).toBeTruthy();
  });
});
