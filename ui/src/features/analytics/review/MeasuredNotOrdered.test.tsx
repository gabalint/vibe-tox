import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MeasuredNotOrderedView } from "@/generated/MeasuredNotOrderedView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { RUN_ID, SUBJECT, sessionWith } from "../testData";
import { SpecimenResultsPage } from "./SpecimenResultsPage";

const SPECIMEN_ID = "00000000-0000-7000-8000-000000006101";

/** A synthetic measurement kept outside the ordered scope (result files brief F17). */
function kept(overrides: Partial<MeasuredNotOrderedView> = {}): MeasuredNotOrderedView {
  return {
    measurementId: "measurement-fen",
    executionId: "execution-1",
    runId: RUN_ID,
    instrumentName: "Synthetic LC-MS/MS 1",
    runCreatedAt: "2026-10-13T08:30:00Z",
    analyteId: "analyte-fen",
    analyteCode: "SYN-FEN",
    analyteName: "Synthetic fentanyl",
    valueText: "1.98",
    unitsText: "ng/mL",
    form: "quantity",
    recordedAt: "2026-10-13T10:00:00Z",
    addOnComponents: [],
    ...overrides,
  };
}

const FENTANYL = kept({
  addOnComponents: [{ componentId: "component-fen", orderableCode: "SYN-FENT" }],
});
const OXYCODONE = kept({
  measurementId: "measurement-oxy",
  analyteId: "analyte-oxy",
  analyteCode: "SYN-OXY",
  analyteName: "Synthetic oxycodone",
  valueText: "N/F",
  unitsText: null,
  form: "word",
});

function results(measured: MeasuredNotOrderedView[], allowedActions: string[]): SpecimenResults {
  return {
    subject: SUBJECT,
    accessionNumber: "A26000003",
    specimenId: SPECIMEN_ID,
    specimenTypeCode: "urine",
    acceptance: "accepted",
    lines: [],
    validityRequired: false,
    currentValidity: null,
    validityHistory: [],
    pendingAnalytes: [],
    reflexHistory: [],
    measuredNotOrdered: measured,
    allowedActions,
  };
}

const USABLE = results([OXYCODONE, FENTANYL], ["submit_technical_review", "use_measured_result"]);

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    get_specimen_results: () => USABLE,
    list_reflex_triggers: () => [],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/review/specimens/:specimenId", element: <SpecimenResultsPage /> }],
    `/review/specimens/${SPECIMEN_ID}`,
  );
}

const measuredList = () => screen.findByRole("list", { name: "Measured, not ordered" });

describe("Measured, not ordered (result files brief F17)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("lists kept measurements as received, usable ones first", async () => {
    const { container } = renderPage();
    const list = await measuredList();
    const [first, second] = within(list).getAllByRole("listitem");
    if (first === undefined) throw new Error("no rows");
    // A value and its unit stay on one line, joined by a non-breaking space.
    expect(first.textContent).toContain("Synthetic fentanyl (SYN-FEN): 1.98\u00A0ng/mL");
    expect(second?.textContent).toContain(
      "Synthetic oxycodone (SYN-OXY): N/F\u00A0(no units reported)",
    );
    const run = within(first).getByRole("link", {
      name: "Synthetic LC-MS/MS 1, run of 2026-10-13 08:30 UTC",
    });
    expect(run.getAttribute("href")).toBe(`/bench/runs/${RUN_ID}`);
    // Only a measurement an accepted add-on covers can be used.
    expect(within(list).getAllByRole("button", { name: "Use measured result" })).toHaveLength(1);
    await expectNoA11yViolations(container);
  });

  it("uses a measurement for its add-on test with a reason, then refreshes the results", async () => {
    const use = vi.fn(() => results([OXYCODONE], ["submit_technical_review"]));
    const get = vi
      .fn()
      .mockReturnValueOnce(USABLE)
      .mockReturnValue(results([OXYCODONE], []));
    const { user } = renderPage({ use_measured_result: use, get_specimen_results: get });
    await user.click(await screen.findByRole("button", { name: "Use measured result" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Use the measured Synthetic fentanyl (SYN-FEN)",
    });
    expect(within(dialog).getByText(/goes through technical review/)).toBeTruthy();
    expect(within(dialog).getByText("SYN-FENT")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Use measured result" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    await typeInto(user, /^Reason/, "Add-on accepted from the clinic");
    await user.click(within(dialog).getByRole("button", { name: "Use measured result" }));
    expect(
      await screen.findByText(
        "Synthetic fentanyl (SYN-FEN) used for SYN-FENT; it needs technical review",
      ),
    ).toBeTruthy();
    expect(use).toHaveBeenCalledWith({
      input: {
        orderComponentId: "component-fen",
        executionId: "execution-1",
        reason: "Add-on accepted from the clinic",
      },
    });
    await vi.waitFor(() => expect(get).toHaveBeenCalledTimes(2));
  });

  it("asks which add-on test when several can use it, and says a refusal in plain words", async () => {
    const two = kept({
      addOnComponents: [
        { componentId: "component-a", orderableCode: "SYN-FENT" },
        { componentId: "component-b", orderableCode: "SYN-OPI" },
      ],
    });
    const use = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "orderItemId", code: "matrix_mismatch", message: "No specimen of that type." },
          ],
        }),
      ),
    );
    const { user } = renderPage({
      get_specimen_results: () => results([two], ["use_measured_result"]),
      use_measured_result: use,
    });
    await user.click(await screen.findByRole("button", { name: "Use measured result" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Add-on test$/, "SYN-OPI");
    await typeInto(user, /^Reason/, "Opiate panel added");
    await user.click(within(dialog).getByRole("button", { name: "Use measured result" }));
    expect(
      await within(dialog).findByText(
        "This accession has no specimen of the type the add-on test requires.",
      ),
    ).toBeTruthy();
    expect(use).toHaveBeenCalledWith({
      input: {
        orderComponentId: "component-b",
        executionId: "execution-1",
        reason: "Opiate panel added",
      },
    });
  });

  it("says in plain words why a value word cannot be used on a dilution run", async () => {
    const { user } = renderPage({
      use_measured_result: () =>
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
    await user.click(await screen.findByRole("button", { name: "Use measured result" }));
    const dialog = await screen.findByRole("dialog");
    await typeInto(user, /^Reason/, "Add-on accepted");
    await user.click(within(dialog).getByRole("button", { name: "Use measured result" }));
    expect(
      await within(dialog).findByText(/A word such as N\/F cannot be used on a dilution run/),
    ).toBeTruthy();
  });

  it("offers no use without the engine's allowed action, and shows the rest on request", async () => {
    const many = Array.from({ length: 12 }, (_, index) =>
      kept({ measurementId: `m-${index}`, analyteName: `Synthetic analyte ${index}` }),
    );
    const { user } = renderPage({ get_specimen_results: () => results(many, []) });
    const list = await measuredList();
    expect(within(list).getAllByRole("listitem")).toHaveLength(10);
    expect(within(list).queryByRole("button", { name: "Use measured result" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show all (12)" }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(12);
  });

  it("shows no section when nothing was measured outside the order", async () => {
    renderPage({ get_specimen_results: () => results([], []) });
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results for accession A26000003" }),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: /Measured, not ordered/ })).toBeNull();
  });
});
