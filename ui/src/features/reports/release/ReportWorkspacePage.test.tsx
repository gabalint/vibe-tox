import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import type { ReportWorkspace } from "../contract";
import { workspacePath } from "../paths";
import {
  CANDIDATE,
  CANDIDATE_ID,
  REPORT_ID,
  REVISION,
  SPECIMEN_ID,
  sessionWith,
  WORKSPACE,
} from "../testData";
import { ReportWorkspacePage } from "./ReportWorkspacePage";

const ROUTE = { path: "/reports/specimens/:specimenId", element: <ReportWorkspacePage /> };

function mockPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["release_reports"]),
    get_report_workspace: () => WORKSPACE,
    ...commands,
  });
}

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockPage(commands);
  return renderRoutes([ROUTE], `/reports/specimens/${SPECIMEN_ID}`);
}

describe("ReportWorkspacePage (REQ-RPT-001, REQ-RPT-005)", () => {
  it("keeps focus on Next while the next workspace loads", async () => {
    const second = "00000000-0000-7000-8000-000000006002";
    mockPage({
      get_report_workspace: (args) =>
        JSON.stringify(args).includes(second)
          ? { ...WORKSPACE, specimenId: second, accessionNumber: "A26000002" }
          : WORKSPACE,
    });
    const { user } = await renderFromList({
      route: ROUTE,
      recordPath: workspacePath,
      ids: [SPECIMEN_ID, second, "00000000-0000-7000-8000-000000006003"],
    });
    await screen.findByRole("heading", { level: 1, name: "Report for accession A26000001" });
    await expectNextKeepsFocus(user, "Report for accession A26000002");
  });

  it("shows the preview, dispositions and patient, and prepares a candidate", async () => {
    const get = vi.fn(() => WORKSPACE);
    const prepare = vi.fn(() => CANDIDATE);
    const { user, container } = renderPage({
      get_report_workspace: get,
      prepare_report_candidate: prepare,
    });
    expect(
      await screen.findByRole("heading", { level: 1, name: "Report for accession A26000001" }),
    ).toBeTruthy();
    expect(get).toHaveBeenCalledWith({ input: { specimenId: SPECIMEN_ID } });
    // The identity banner, under the page title: patient, identifiers and accession.
    const identity = screen.getByRole("region", { name: "Patient and specimen identity" });
    expect(within(identity).getByText("MRN-00000")).toBeTruthy();
    expect(within(identity).getByText("A26000001")).toBeTruthy();
    expect(identity.textContent).toContain("Testpatient, Jane");
    expect(screen.getByText("Not yet issued")).toBeTruthy();
    expect(screen.getByRole("list", { name: "Expected tests" })).toBeTruthy();
    expect(screen.getByRole("article", { name: /Printed content/ })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Prepare candidate" }));
    const dialog = await screen.findByRole("dialog", { name: "Prepare report candidate" });
    expect(within(dialog).queryByLabelText(/Reason for correction/)).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    expect(
      await screen.findByText(
        `Navigated to /reports/specimens/${SPECIMEN_ID}/candidates/${CANDIDATE_ID}`,
      ),
    ).toBeTruthy();
    expect(prepare).toHaveBeenCalledWith({ input: { specimenId: SPECIMEN_ID } });
  });

  it("requires explicit condition choices and keeps internal notes out of the preview", async () => {
    const prepare = vi.fn(() => CANDIDATE);
    const source = {
      sourceSha256: "a".repeat(64),
      latestDecisionId: "current-decision",
      conditions: [
        {
          decisionId: "older-decision",
          decision: "held" as const,
          recordedAt: "2026-10-12T14:00:00Z",
          code: "other" as const,
          description: "Synthetic internal routing note",
        },
        {
          decisionId: "current-decision",
          decision: "accepted_conditionally" as const,
          recordedAt: "2026-10-12T14:00:00Z",
          code: "other" as const,
          description: "Synthetic leakage description",
        },
      ],
    };
    const { user } = renderPage({
      get_report_workspace: () => ({ ...WORKSPACE, conditionReviewSource: source }),
      prepare_report_candidate: prepare,
    });
    const preview = await screen.findByRole("article", { name: /Printed content/ });
    expect(within(preview).queryByText("Synthetic internal routing note")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Prepare candidate" }));
    const dialog = await screen.findByRole("dialog", { name: "Prepare report candidate" });
    expect(within(dialog).getByText("Synthetic internal routing note")).toBeTruthy();
    await expectNoA11yViolations(dialog);
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    expect(prepare).not.toHaveBeenCalled();
    await chooseWithKeyboard(
      user,
      /Report choice for condition 1/,
      "Keep earlier condition internal",
    );
    await chooseWithKeyboard(user, /Report choice for condition 2/, "Include description");
    await user.type(
      within(dialog).getByLabelText(/Reason for condition selections/),
      "Retain the observed leakage; exclude internal routing.",
    );
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    expect(prepare).toHaveBeenCalledWith({
      input: {
        specimenId: SPECIMEN_ID,
        conditionReview: {
          sourceSha256: source.sourceSha256,
          reason: "Retain the observed leakage; exclude internal routing.",
          decisions: [
            { decisionId: "older-decision", code: "other", includeInReport: false },
            { decisionId: "current-decision", code: "other", includeInReport: true },
          ],
        },
      },
    });
  });

  it("keeps fresh preparation reachable when an older candidate is damaged", async () => {
    renderPage({
      get_report_workspace: () => ({
        ...WORKSPACE,
        unavailableCandidateId: CANDIDATE_ID,
        openCandidate: null,
      }),
    });
    expect((await screen.findByRole("alert")).textContent).toContain("failed its integrity check");
    expect(screen.getByRole("button", { name: "Prepare candidate" })).toBeTruthy();
    expect(screen.getByRole("article", { name: /Printed content/ })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Review candidate" })).toBeNull();
  });

  it("asks for the reason when the revision is a correction", async () => {
    const prepare = vi.fn(() => CANDIDATE);
    const { user } = renderPage({
      get_report_workspace: (): ReportWorkspace => ({
        ...WORKSPACE,
        report: {
          reportId: REPORT_ID,
          reportNumber: "A26000001-URN",
          currentRevision: REVISION,
          availability: "current",
        },
        preview: { ...WORKSPACE.preview, relationship: "correction" },
      }),
      prepare_report_candidate: prepare,
    });
    expect(await screen.findByText("A26000001-URN")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "A26000001-URN" })).toBeNull();
    expect(screen.getByText("Current")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Prepare candidate" }));
    const dialog = await screen.findByRole("dialog", { name: "Prepare report candidate" });
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    expect(await within(dialog).findByText("Enter reason for correction.")).toBeTruthy();
    expect(prepare).not.toHaveBeenCalled();
    await user.type(within(dialog).getByLabelText(/^Reason for correction/), "Wrong units");
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    expect(prepare).toHaveBeenCalledWith({
      input: { specimenId: SPECIMEN_ID, amendmentReason: "Wrong units" },
    });
  });

  it("shows what blocks release, without the prepare action", async () => {
    renderPage({
      get_report_workspace: (): ReportWorkspace => ({
        ...WORKSPACE,
        preview: {
          ...WORKSPACE.preview,
          content: null,
          gates: [
            {
              code: "laboratory_profile_missing",
              message: "Set the laboratory profile before preparing reports.",
            },
          ],
        },
        allowedActions: [],
      }),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Release is blocked until these are resolved");
    const gates = within(alert).getByRole("list", { name: "Blocking release" });
    expect(gates.textContent).toContain("Laboratory profile not set:");
    expect(screen.getByText(/Nothing can be shown yet/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Prepare candidate" })).toBeNull();
  });

  it("shows an up-to-date report as current, not blocked, with links to the issued revision", async () => {
    mockCommands({
      current_session: () => sessionWith(["release_reports", "deliver_reports"]),
      get_report_workspace: (): ReportWorkspace => ({
        ...WORKSPACE,
        report: {
          reportId: REPORT_ID,
          reportNumber: "A26000001-URN",
          currentRevision: REVISION,
          availability: "current",
        },
        preview: {
          ...WORKSPACE.preview,
          content: null,
          relationship: null,
          gates: [
            { code: "nothing_changed", message: "Nothing changed since the current revision." },
          ],
        },
        allowedActions: [],
      }),
    });
    const { container } = renderRoutes([ROUTE], `/reports/specimens/${SPECIMEN_ID}`);
    const status = (await screen.findByText("Revision 1 is current")).closest("[role=status]");
    if (!(status instanceof HTMLElement)) throw new Error("Revision 1 is current is not a status");
    expect(status.textContent).toContain("Nothing new to release");
    expect(
      await within(status).findByRole("link", { name: "View report and delivery" }),
    ).toBeTruthy();
    expect(screen.queryByText("Release is blocked until these are resolved")).toBeNull();
    expect(screen.queryByText(/Nothing can be shown yet/)).toBeNull();
    expect(screen.getByText(/Nothing new to print: revision 1 is the issued content/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "View revision 1 as issued" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("still shows the block when another gate stands beside nothing changed", async () => {
    renderPage({
      get_report_workspace: (): ReportWorkspace => ({
        ...WORKSPACE,
        report: {
          reportId: REPORT_ID,
          reportNumber: "A26000001-URN",
          currentRevision: REVISION,
          availability: "current",
        },
        preview: {
          ...WORKSPACE.preview,
          content: null,
          relationship: null,
          gates: [
            { code: "nothing_changed", message: "Nothing changed since the current revision." },
            { code: "identity_hold_open", message: "An identity hold is open for this patient." },
          ],
        },
        allowedActions: [],
      }),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Release is blocked until these are resolved");
    expect(screen.queryByText(/is current$/)).toBeNull();
    expect(screen.getByText(/Nothing can be shown yet/)).toBeTruthy();
  });

  it("shows the engine's refusal to prepare when a hold appeared since the preview", async () => {
    const { user } = renderPage({
      prepare_report_candidate: () =>
        Promise.reject(
          engineError("gate_blocked", "This action is blocked by one or more gates.", {
            gateReasons: [
              {
                code: "identity_hold_open",
                message: "An identity hold is open for this patient.",
              },
            ],
          }),
        ),
    });
    await user.click(await screen.findByRole("button", { name: "Prepare candidate" }));
    const dialog = await screen.findByRole("dialog", { name: "Prepare report candidate" });
    await user.click(within(dialog).getByRole("button", { name: "Prepare candidate" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("Open identity hold:");
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("links the open candidate and hides preparing when the engine does not allow it", async () => {
    renderPage({
      get_report_workspace: () => ({
        ...WORKSPACE,
        openCandidate: CANDIDATE,
        allowedActions: [],
      }),
    });
    const link = await screen.findByRole("link", { name: "Review candidate" });
    expect(link.getAttribute("href")).toBe(
      `/reports/specimens/${SPECIMEN_ID}/candidates/${CANDIDATE_ID}`,
    );
    expect(screen.queryByRole("button", { name: "Prepare candidate" })).toBeNull();
  });

  it("shows the engine's error", async () => {
    renderPage({
      get_report_workspace: () =>
        Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
    });
    expect(await screen.findByText("You do not have permission for this action.")).toBeTruthy();
    expect(screen.getByText("The report workspace could not be loaded")).toBeTruthy();
  });
});
