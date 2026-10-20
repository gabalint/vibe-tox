import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import { patientPath } from "../subjects";
import { HELD_SUBJECT_VIEW, SUBJECT_ID, SUBJECT_VIEW, sessionWith } from "../testData";
import { PatientDetailPage } from "./PatientDetailPage";

function renderPage(commands: Record<string, (args: unknown) => unknown>) {
  mockCommands(commands);
  return renderRoutes(
    [{ path: "/patients/:subjectId", element: <PatientDetailPage /> }],
    `/patients/${SUBJECT_ID}`,
  );
}

describe("PatientDetailPage (REQ-IDN-001)", () => {
  beforeEach(mockGridLayout);

  it("shows the patient, identifiers and demographic history", async () => {
    const getSubject = vi.fn(() => SUBJECT_VIEW);
    const { container } = renderPage({
      get_subject: getSubject,
      current_session: () => sessionWith(["enter_orders"]),
    });
    // The header is in place while the patient loads.
    expect(screen.getByRole("heading", { level: 1, name: "Patient" })).toBeTruthy();
    expect(screen.getByText("Loading patient…")).toBeTruthy();
    expect(await screen.findByRole("heading", { level: 1, name: "Patient, Test" })).toBeTruthy();
    // One audited read: the banner shows the record the page already loaded.
    expect(getSubject).toHaveBeenCalledTimes(1);
    expect(getSubject).toHaveBeenCalledWith({ input: { subjectId: SUBJECT_ID } });
    const banner = screen.getByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    expect(within(banner).getByText(/^Jan 1, 1970/)).toBeTruthy();
    const breadcrumb = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(breadcrumb).getByRole("link", { name: "Patients" })).toBeTruthy();
    expect(screen.getByText("No open identity holds.")).toBeTruthy();
    const history = screen.getByRole("grid", { name: "Demographic history" });
    expect(within(history).getByText("Paper requisition")).toBeTruthy();
    expect(within(history).getByText("First record")).toBeTruthy();
    expect(within(history).getByText(/^Jan 1, 1970/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "New order" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("lets quality staff confirm different people, with a reason", async () => {
    const resolve = vi.fn(() => ({ holdId: "hold", revision: 2 }));
    const { user } = renderPage({
      get_subject: () => HELD_SUBJECT_VIEW,
      current_session: () => sessionWith(["enter_orders", "manage_quality"]),
      resolve_identity_hold: resolve,
    });
    const banner = await screen.findByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("Identity hold")).toBeTruthy();
    expect(screen.getByRole("link", { name: "patient record 1" })).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: "Confirm different people" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Confirm different people" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(resolve).not.toHaveBeenCalled();

    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Different MRN");
    await user.click(within(dialog).getByRole("button", { name: "Confirm different people" }));
    expect(await screen.findByText("Identity hold resolved")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        holdId: HELD_SUBJECT_VIEW.openHolds[0]?.id,
        expectedRevision: 1,
        reason: "Different MRN",
      },
    });
  });

  it("does not offer hold resolution without quality authority", async () => {
    renderPage({
      get_subject: () => HELD_SUBJECT_VIEW,
      current_session: () => sessionWith(["enter_orders"]),
    });
    expect(await screen.findByText("Possible duplicate")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Confirm different people" })).toBeNull();
  });

  it("shows the engine's refusal of a resolution in the dialog", async () => {
    const { user } = renderPage({
      get_subject: () => HELD_SUBJECT_VIEW,
      current_session: () => sessionWith(["manage_quality"]),
      resolve_identity_hold: () =>
        Promise.reject(engineError("conflict", "The hold changed since it was loaded.")),
    });
    await user.click(await screen.findByRole("button", { name: "Confirm different people" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Different MRN");
    await user.click(within(dialog).getByRole("button", { name: "Confirm different people" }));
    expect(await within(dialog).findByText("The hold changed since it was loaded.")).toBeTruthy();
  });

  it("keeps focus on Next while the next patient of the list loads", async () => {
    const ids = [SUBJECT_ID, "subject-2", "subject-3"];
    const names = ["One", "Two", "Three"];
    mockCommands({
      get_subject: (args) => {
        const index = ids.findIndex((id) => JSON.stringify(args).includes(id));
        const assertions = SUBJECT_VIEW.assertions.map((each) => ({
          ...each,
          givenNames: names[index] ?? null,
        }));
        return { ...SUBJECT_VIEW, id: ids[index], assertions };
      },
      current_session: () => sessionWith(["enter_orders"]),
    });
    const { user } = await renderFromList({
      route: { path: "/patients/:subjectId", element: <PatientDetailPage /> },
      recordPath: patientPath,
      ids,
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Patient, One" })).toBeTruthy();
    await expectNextKeepsFocus(user, "Patient, Two");
  });

  it("shows the engine error with a retry", async () => {
    renderPage({
      get_subject: () => Promise.reject(engineError("not_found", "The patient was not found.")),
      current_session: () => sessionWith([]),
    });
    expect(await screen.findByText("The patient was not found.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Patient" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});
