import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { IntakeDeferralView } from "@/generated/IntakeDeferralView";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { SUMMARY, sessionWith } from "../testData";
import { IncompleteIntakePage } from "./IncompleteIntakePage";

const CLERK = "00000000-0000-7000-8000-000000000e31";
const REVIEWER = "00000000-0000-7000-8000-000000000e32";

const INTAKE: IntakeDeferralView = {
  intakeId: "synthetic-intake",
  revision: 1,
  suppliedName: "Incomplete Person",
  birthEvidenceKind: "unknown",
  birthEvidenceText: null,
  sourceReference: "Synthetic source 42",
  reason: "Complete birth date not supplied",
  recordedBy: CLERK,
  recordedByDisplay: "Test Intake Clerk",
  recordedAt: "2026-10-14T10:00:00Z",
  disposition: null,
  subjectId: null,
  dispositionSource: null,
  dispositionReason: null,
  disposedBy: null,
  disposedAt: null,
};
function renderIntake(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["enter_orders"]),
    list_intake_deferrals: () => [INTAKE],
    search_subjects: () => [SUMMARY],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/patients/incomplete", element: <IncompleteIntakePage /> }],
    "/patients/incomplete",
  );
}
describe("Incomplete identity intake mocked IPC", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);
  it("retains unknown birth information without constructing a DOB or creating a patient", async () => {
    const record = vi.fn(() => INTAKE);
    const register = vi.fn();
    const { user } = renderIntake({ record_intake_deferral: record, register_subject: register });
    await user.click(await screen.findByRole("button", { name: "Record incomplete intake" }));
    const dialog = await screen.findByRole("dialog", { name: "Record incomplete identity intake" });
    await user.type(
      within(dialog).getByRole("textbox", { name: /Name as supplied/ }),
      "Incomplete Person",
    );
    await chooseWithKeyboard(user, /Available birth information$/, "Unknown");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Source reference/ }),
      "Synthetic source 42",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason intake is deferred/ }),
      "Complete birth date not supplied",
    );
    await user.click(within(dialog).getByRole("button", { name: "Retain incomplete intake" }));
    expect(await screen.findByText("Incomplete intake retained")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        suppliedName: "Incomplete Person",
        birthEvidenceKind: "unknown",
        birthEvidenceText: null,
        sourceReference: "Synthetic source 42",
        reason: "Complete birth date not supplied",
      },
    });
    expect(register).not.toHaveBeenCalled();
  });
  it("resolves only after an explicit existing-patient choice and source-backed disposition", async () => {
    const resolve = vi.fn(() => ({
      ...INTAKE,
      revision: 2,
      disposition: "resolved",
      subjectId: SUMMARY.subjectId,
    }));
    const { user } = renderIntake({ resolve_intake_deferral: resolve });
    await openRow(user, "Incomplete Person");
    await user.click(await screen.findByRole("button", { name: "Resolve with verified patient" }));
    expect(resolve).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "Resolve incomplete identity intake" })).toBeNull();
    await openRow(user, "Patient, Test");
    const dialog = await screen.findByRole("dialog", {
      name: "Resolve incomplete identity intake",
    });
    await user.type(
      within(dialog).getByRole("textbox", { name: /Verification or decline source/ }),
      "Verified synthetic identity source",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Disposition reason/ }),
      "Patient identity confirmed from source",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record disposition" }));
    expect(await screen.findByText("Intake disposition recorded")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        intakeId: INTAKE.intakeId,
        expectedRevision: 1,
        disposition: "resolved",
        subjectId: SUMMARY.subjectId,
        sourceReference: "Verified synthetic identity source",
        reason: "Patient identity confirmed from source",
      },
    });
  });
  it("refreshes away from a stale open selection before reviewing a concurrent disposition", async () => {
    const list = vi
      .fn()
      .mockReturnValueOnce([INTAKE])
      .mockReturnValue([
        {
          ...INTAKE,
          revision: 2,
          disposition: "declined",
          dispositionReason: "Source withdrew intake",
          dispositionSource: "Synthetic source",
          disposedBy: REVIEWER,
          disposedByDisplay: "Test Intake Reviewer",
          disposedAt: "2026-10-14T11:00:00Z",
        },
      ]);
    const { user } = renderIntake({ list_intake_deferrals: list });
    await openRow(user, "Incomplete Person");
    expect(
      await screen.findByRole("button", { name: "Resolve with verified patient" }),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("button", { name: "Resolve with verified patient" })).toBeNull();
    await openRow(user, "Incomplete Person");
    expect(await screen.findByText(/Source withdrew intake/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record declined intake" })).toBeNull();
    // Both actors show by account name, never by account ID.
    expect(screen.getByText(/Recorded by Test Intake Clerk/)).toBeTruthy();
    expect(screen.getByText(/Recorded by Test Intake Reviewer/)).toBeTruthy();
    expect(screen.queryByText(new RegExp(`${CLERK}|${REVIEWER}`))).toBeNull();
  });
});
