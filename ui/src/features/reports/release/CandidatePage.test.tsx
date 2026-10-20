import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { tabTo, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import { candidatePath } from "../paths";
import {
  CANDIDATE,
  CANDIDATE_ID,
  REVISION,
  SPECIMEN_ID,
  sessionWith,
  WORKSPACE,
} from "../testData";
import { CandidatePage } from "./CandidatePage";

const PATH = `/reports/specimens/${SPECIMEN_ID}/candidates/${CANDIDATE_ID}`;

const ROUTE = {
  path: "/reports/specimens/:specimenId/candidates/:candidateId",
  element: <CandidatePage />,
};

function mockPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["release_reports"]),
    get_report_candidate: () => CANDIDATE,
    get_report_workspace: () => WORKSPACE,
    ...commands,
  });
}

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockPage(commands);
  return renderRoutes([ROUTE], PATH);
}

async function openRelease(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Release report" }));
  return screen.findByRole("alertdialog", { name: "Release report A26000001-URN" });
}

describe("CandidatePage (REQ-RPT-001, REQ-RPT-005)", () => {
  it("names the patient and the specimen from the specimen's report workspace", async () => {
    const workspace = vi.fn(() => WORKSPACE);
    renderPage({ get_report_workspace: workspace });
    const identity = await screen.findByRole("region", { name: "Patient and specimen identity" });
    expect(identity.textContent).toContain("Testpatient, Jane");
    expect(within(identity).getByText("MRN-00000")).toBeTruthy();
    expect(within(identity).getByText("A26000001")).toBeTruthy();
    expect(workspace).toHaveBeenCalledWith({ input: { specimenId: SPECIMEN_ID } });
  });

  it("says when the patient cannot be loaded, and still shows the candidate", async () => {
    renderPage({
      get_report_workspace: () =>
        Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
    });
    expect(await screen.findByText("The patient could not be loaded")).toBeTruthy();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Candidate for report A26000001-URN" }),
    ).toBeTruthy();
  });

  it("keeps focus on Next while the next candidate loads", async () => {
    const second = "00000000-0000-7000-8000-00000000c002";
    mockPage({
      get_report_candidate: (args) =>
        JSON.stringify(args).includes(second)
          ? { ...CANDIDATE, candidateId: second, reportNumber: "A26000002-URN" }
          : CANDIDATE,
    });
    const { user } = await renderFromList({
      route: ROUTE,
      recordPath: (id: string) => candidatePath(SPECIMEN_ID, id),
      ids: [CANDIDATE_ID, second, "00000000-0000-7000-8000-00000000c003"],
    });
    await screen.findByRole("heading", { level: 1, name: "Candidate for report A26000001-URN" });
    await expectNextKeepsFocus(user, "Candidate for report A26000002-URN");
  });

  it("shows exactly the printed content, dispositions and gates", async () => {
    const get = vi.fn(() => CANDIDATE);
    const { container } = renderPage({ get_report_candidate: get });
    const content = await screen.findByRole("article", {
      name: "Printed content of report A26000001-URN",
    });
    expect(get).toHaveBeenCalledWith({ input: { candidateId: CANDIDATE_ID } });
    const amphetamine = within(content).getByRole("row", { name: /^Synthetic amphetamine 512\.4/ });
    expect(within(amphetamine).getByText("512.4")).toBeTruthy();
    expect(within(amphetamine).getByText("Cutoff 50 ng/mL; LOQ 5 ng/mL")).toBeTruthy();
    expect(within(amphetamine).getByText("Meets cutoff")).toBeTruthy();
    expect(within(content).getByText("<5.0")).toBeTruthy();
    expect(within(content).getByText("2026-10-12 09:30 CDT")).toBeTruthy();
    expect(within(content).getByText("Synthetic opiate panel: Pending")).toBeTruthy();
    const tests = screen.getByRole("list", { name: "Expected tests" });
    expect(within(tests).getByText("Reported")).toBeTruthy();
    const holding = screen.getByRole("list", { name: "Holding Synthetic opiate panel" });
    expect(holding.textContent).toContain("Technical review out of date:");
    expect(holding.textContent).toContain("QC for this run was rejected.");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("releases with the digest shown, the password and the partial acknowledgement", async () => {
    const release = vi.fn(() => REVISION);
    const { user } = renderPage({ release_report: release });
    const dialog = await openRelease(user);
    expect(dialog.textContent).toContain("as a partial report, signed by you");
    await user.click(within(dialog).getByRole("checkbox", { name: /partial report/ }));
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    expect(await screen.findByText(`Navigated to /reports/specimens/${SPECIMEN_ID}`)).toBeTruthy();
    expect(release).toHaveBeenCalledWith({
      input: {
        candidateId: CANDIDATE_ID,
        expectedRevision: 3,
        contentSha256: CANDIDATE.contentSha256,
        acknowledgePartial: true,
        password: "synthetic-secret",
        selfApproval: false,
      },
    });
  });

  it("does not release a partial report without the acknowledgement", async () => {
    const release = vi.fn(() => REVISION);
    const { user } = renderPage({ release_report: release });
    const dialog = await openRelease(user);
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    expect(await within(dialog).findByText("Acknowledge that the report is partial.")).toBeTruthy();
    expect(release).not.toHaveBeenCalled();
  });

  it("shows a wrong password on the field and does not release", async () => {
    const { user } = renderPage({
      release_report: () =>
        Promise.reject(
          engineError("validation", "The password is incorrect.", {
            fieldErrors: [
              { field: "password", code: "incorrect", message: "The password is incorrect." },
            ],
          }),
        ),
    });
    const dialog = await openRelease(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /partial report/ }));
    const password = within(dialog).getByLabelText(/^Your password/);
    await user.type(password, "wrong-password");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    expect(await within(dialog).findByText("Not released")).toBeTruthy();
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(within(dialog).getAllByText("The password is incorrect.").length).toBeGreaterThan(0);
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("refuses a stale candidate and lists the gate reasons", async () => {
    const stale =
      "The report content changed since the candidate was prepared. Prepare a new candidate.";
    const { user } = renderPage({
      release_report: () =>
        Promise.reject(
          engineError("conflict", stale, {
            gateReasons: [
              { code: "candidate_stale", message: stale },
              { code: "review_stale:qc_rejected", message: "QC for run R-1 was rejected." },
            ],
          }),
        ),
    });
    const dialog = await openRelease(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /partial report/ }));
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain(
      "Report content changed since the candidate was prepared",
    );
    expect(blocked.textContent).toContain("Technical review out of date:");
    expect(blocked.textContent).toContain("QC for run R-1 was rejected.");
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("refuses a release by the person who reviewed the results", async () => {
    const { user } = renderPage({
      release_report: () =>
        Promise.reject(
          engineError("gate_blocked", "This action is blocked by one or more gates.", {
            gateReasons: [
              {
                code: "self_release",
                message: "The technical reviewer of a reported result may not release it.",
              },
            ],
          }),
        ),
    });
    const dialog = await openRelease(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /partial report/ }));
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("You reviewed results on this report:");
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("asks the reviewer to confirm a self-release the laboratory allows (ADR 0015)", async () => {
    const message =
      "You reviewed results on this report. Your laboratory allows you to release it yourself: confirm that no second person reviewed it.";
    const release = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
        }),
      )
      .mockReturnValue(REVISION);
    const { user } = renderPage({ release_report: release });
    const dialog = await openRelease(user);
    expect(within(dialog).queryByRole("checkbox", { name: /no second person/ })).toBeNull();
    await user.click(within(dialog).getByRole("checkbox", { name: /partial report/ }));
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    const confirm = await within(dialog).findByRole("checkbox", { name: /no second person/ });
    expect(within(dialog).getByText(message)).toBeTruthy();
    expect(within(dialog).queryByText("Not released")).toBeNull();
    expect(confirm).toHaveProperty("checked", false);
    await user.click(confirm);
    await user.click(within(dialog).getByRole("button", { name: "Release report" }));
    expect(await screen.findByText(`Navigated to /reports/specimens/${SPECIMEN_ID}`)).toBeTruthy();
    expect(release).toHaveBeenLastCalledWith({
      input: expect.objectContaining({ password: "synthetic-secret", selfApproval: true }),
    });
    expect(release.mock.calls[0]?.[0]).toMatchObject({ input: { selfApproval: false } });
  });

  it("shows a stale candidate without the release action", async () => {
    renderPage({
      get_report_candidate: () => ({
        ...CANDIDATE,
        stale: true,
        staleReasons: [{ code: "component_cancelled", message: "A reported test was cancelled." }],
        allowedActions: [],
      }),
    });
    expect(
      await screen.findByText("The content changed since this candidate was prepared"),
    ).toBeTruthy();
    expect(screen.getByRole("list", { name: "What changed" }).textContent).toContain(
      "Test cancelled:",
    );
    expect(screen.queryByRole("button", { name: "Release report" })).toBeNull();
  });

  it("shows a prepared candidate with gates as blocked, in the engine's words", async () => {
    const { container } = renderPage({
      get_report_candidate: () => ({
        ...CANDIDATE,
        gates: [
          { code: "qc_pending", message: "QC for run R-1 is on hold." },
          { code: "identity_hold_open", message: "The patient record has an open identity hold." },
        ],
        allowedActions: [],
      }),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Release is blocked until these are resolved");
    const gates = within(alert).getByRole("list", { name: "Blocking release" });
    expect(gates.textContent).toContain("QC for run R-1 is on hold.");
    expect(gates.textContent).toContain("Open identity hold:");
    expect(screen.getByText("Prepared")).toBeTruthy();
    expect(screen.queryByText("Ready for release")).toBeNull();
    expect(screen.queryByRole("button", { name: "Release report" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows no blocking message when the engine returns no gate", async () => {
    renderPage();
    await screen.findByRole("button", { name: "Release report" });
    expect(screen.queryByText("Release is blocked until these are resolved")).toBeNull();
  });

  it("shows the engine's error when the candidate cannot be read", async () => {
    renderPage({
      get_report_candidate: () =>
        Promise.reject(engineError("not_found", "The report candidate was not found.")),
    });
    expect(await screen.findByText("The report candidate was not found.")).toBeTruthy();
  });

  it(
    "releases by keyboard alone (NFR-09)",
    async () => {
      const release = vi.fn(() => REVISION);
      const { user } = renderPage({ release_report: release });
      await tabTo(user, await screen.findByRole("button", { name: "Release report" }));
      await user.keyboard("{Enter}");
      const dialog = await screen.findByRole("alertdialog");
      await tabTo(user, within(dialog).getByRole("checkbox", { name: /partial report/ }));
      await user.keyboard(" ");
      await tabTo(user, within(dialog).getByLabelText(/^Your password/));
      await user.keyboard("synthetic-secret");
      await tabTo(user, within(dialog).getByRole("button", { name: "Release report" }));
      await user.keyboard("{Enter}");
      await waitFor(() => expect(release).toHaveBeenCalledTimes(1));
      expect(release).toHaveBeenCalledWith({
        input: expect.objectContaining({ acknowledgePartial: true, password: "synthetic-secret" }),
      });
    },
    WALKTHROUGH_TIMEOUT_MS,
  );
});
