import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { HELD_SUBJECT_VIEW, SUBJECT_ID, SUMMARY, sessionWith } from "../testData";
import { PatientDetailPage } from "./PatientDetailPage";
import { PatientsPage } from "./PatientsPage";

function renderQuality(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["manage_quality"]),
    search_subjects: () => [SUMMARY],
    get_subject: () => HELD_SUBJECT_VIEW,
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/patients", element: <PatientsPage /> },
      { path: "/patients/:subjectId", element: <PatientDetailPage /> },
    ],
    "/patients",
  );
}

describe("Pure quality authority patient pages, mocked IPC", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);
  it("opens identity review without order entry, standing order or financial reads", async () => {
    const standing = vi.fn(() => []);
    const billing = vi.fn();
    const { user } = renderQuality({
      list_standing_orders: standing,
      get_patient_billing_profile: billing,
    });
    await screen.findByRole("gridcell", { name: "Patient, Test" });
    expect(screen.queryByRole("button", { name: "Register patient" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Actions for Patient/ })).toBeNull();
    await openRow(user, "Patient, Test");
    expect(await screen.findByRole("button", { name: "Correct demographics" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Confirm different people" })).toBeTruthy();
    for (const name of ["Orders", "New order", "Add identifier", "Open patient billing profile"]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
    expect(screen.queryByRole("heading", { name: "Standing orders" })).toBeNull();
    expect(standing).not.toHaveBeenCalled();
    expect(billing).not.toHaveBeenCalled();
  });
  it("gives an honest empty identity directory without offering registration", async () => {
    renderQuality({ search_subjects: () => [] });
    expect(await screen.findByText("No patients yet")).toBeTruthy();
    expect(screen.getByText("No patient records are available for identity review.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Register patient" })).toBeNull();
  });
  it("checks corrected identity and saves reviewed evidence using only quality authority", async () => {
    const match = vi.fn(() => ({ identifierMatches: [], candidates: [] }));
    const save = vi.fn(() => HELD_SUBJECT_VIEW);
    const { user } = renderQuality({ match_subject_candidates: match, assert_demographics: save });
    await openRow(user, "Patient, Test");
    await user.click(await screen.findByRole("button", { name: "Correct demographics" }));
    const form = within(screen.getByRole("form", { name: "Correct demographics" }));
    await user.type(
      form.getByRole("textbox", { name: /^Source of corrected details/ }),
      "Synthetic corrected requisition",
    );
    await user.type(
      form.getByRole("textbox", { name: /^Reason for correction/ }),
      "Verified spelling evidence",
    );
    await user.click(form.getByRole("button", { name: "Check corrected identity" }));
    await user.click(await screen.findByRole("button", { name: "Save corrected demographics" }));
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith({
        input: expect.objectContaining({
          subjectId: SUBJECT_ID,
          expectedRevision: HELD_SUBJECT_VIEW.revision,
          source: "Synthetic corrected requisition",
          reason: "Verified spelling evidence",
          acknowledgedCandidates: [],
        }),
      }),
    );
    expect(match).toHaveBeenCalledTimes(1);
  });
});
