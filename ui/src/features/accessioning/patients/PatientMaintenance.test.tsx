import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { OTHER_SUMMARY, SUBJECT_VIEW, SUMMARY, sessionWith } from "../testData";
import { PatientMaintenance } from "./PatientMaintenance";

function renderMaintenance(
  capabilities = ["enter_orders", "manage_quality"],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({ current_session: () => sessionWith(capabilities), ...commands });
  return renderRoutes([{ path: "/", element: <PatientMaintenance subject={SUBJECT_VIEW} /> }], "/");
}
async function correctedDetails(user: ReturnType<typeof renderRoutes>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Correct demographics" }));
  const form = screen.getByRole("form", { name: "Correct demographics" });
  const name = within(form).getByRole("textbox", { name: /^Family name/ });
  await user.clear(name);
  await user.type(name, "Corrected Patient");
  await user.type(
    within(form).getByRole("textbox", { name: /^Source of corrected details/ }),
    "Synthetic verified requisition",
  );
  await user.type(
    within(form).getByRole("textbox", { name: /^Reason for correction/ }),
    "Source spelling corrected",
  );
  await user.click(within(form).getByRole("button", { name: "Check corrected identity" }));
}

describe("Patient maintenance mocked IPC", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);
  it("keeps demographic correction authority separate from ordinary identifier entry", async () => {
    renderMaintenance(["enter_orders"]);
    expect(await screen.findByRole("button", { name: "Add identifier" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Correct demographics" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Deactivate MR" })).toBeNull();
  });
  it("checks candidates and requires explicit acknowledgement before sending the exact revision", async () => {
    const save = vi.fn(() => SUBJECT_VIEW);
    const match = vi.fn(() => ({ identifierMatches: [], candidates: [SUMMARY, OTHER_SUMMARY] }));
    const { user } = renderMaintenance(undefined, {
      match_subject_candidates: match,
      assert_demographics: save,
    });
    await correctedDetails(user);
    const submit = await screen.findByRole("button", { name: "Save corrected demographics" });
    expect(submit).toHaveProperty("disabled", true);
    expect(save).not.toHaveBeenCalled();
    await user.click(screen.getByRole("checkbox", { name: /I reviewed these possible matches/ }));
    await user.click(submit);
    await vi.waitFor(() =>
      expect(save).toHaveBeenCalledWith({
        input: {
          subjectId: SUBJECT_VIEW.id,
          expectedRevision: SUBJECT_VIEW.revision,
          demographics: {
            familyName: "Corrected Patient",
            givenNames: "Test",
            birthDate: "1970-01-01",
            administrativeSex: "U",
          },
          source: "Synthetic verified requisition",
          reason: "Source spelling corrected",
          acknowledgedCandidates: [OTHER_SUMMARY.subjectId],
        },
      }),
    );
    expect(match).toHaveBeenCalledWith({
      input: {
        demographics: {
          familyName: "Corrected Patient",
          givenNames: "Test",
          birthDate: "1970-01-01",
          administrativeSex: "U",
        },
        identifiers: [],
      },
    });
  });
  it("shows stale refusal and rechecks candidates without silently retrying the correction", async () => {
    const save = vi.fn(() =>
      Promise.reject(engineError("conflict", "The patient changed; refresh the record.")),
    );
    const match = vi.fn(() => ({ identifierMatches: [], candidates: [OTHER_SUMMARY] }));
    const { user } = renderMaintenance(undefined, {
      match_subject_candidates: match,
      assert_demographics: save,
    });
    await correctedDetails(user);
    const acknowledge = await screen.findByRole("checkbox", {
      name: /I reviewed these possible matches/,
    });
    await user.click(acknowledge);
    await user.click(screen.getByRole("button", { name: "Save corrected demographics" }));
    expect(await screen.findByText("The patient changed; refresh the record.")).toBeTruthy();
    await vi.waitFor(() => expect(match).toHaveBeenCalledTimes(2));
    expect(acknowledge).toHaveProperty("checked", false);
    expect(save).toHaveBeenCalledTimes(1);
  });
});
