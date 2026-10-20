import { act, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { OTHER_SUMMARY, SUBJECT_ID, SUMMARY, sessionWith } from "../testData";
import { PatientsPage } from "./PatientsPage";

function renderPage(searchSubjects: (args: unknown) => unknown) {
  mockCommands({
    search_subjects: searchSubjects,
    current_session: () => sessionWith(["enter_orders"]),
  });
  return renderRoutes([{ path: "/patients", element: <PatientsPage /> }], "/patients");
}

const find = () => screen.getByRole("searchbox", { name: "Find by name or identifier" });

describe("PatientsPage (REQ-IDN-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows patients on arrival, with identity details and keyboard opening", async () => {
    const search = vi.fn(() => [SUMMARY]);
    const { user, container } = renderPage(search);
    expect(await screen.findByRole("gridcell", { name: "Patient, Test" })).toBeTruthy();
    expect(search).toHaveBeenCalledWith({ input: { limit: 26, offset: 0 } });
    expect(screen.getByRole("gridcell", { name: /^Jan 1, 1970 \(\d+ y\)$/ })).toBeTruthy();
    expect(screen.getByText("MRN-00001 (SYN-CLINIC MR)")).toBeTruthy();
    expect(screen.getByRole("button", { name: "More filters" }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(screen.getByText("1 patient")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await tabTo(user, screen.getByRole("columnheader", { name: /^Name/ }));
    await user.keyboard("{ArrowDown}{Enter}");
    expect(await screen.findByText(`Navigated to /patients/${SUBJECT_ID}`)).toBeTruthy();
  });

  it("searches from one field and clears back to all patients without putting names in the URL", async () => {
    const search = vi.fn(() => [SUMMARY]);
    const { user, router } = renderPage(search);
    await screen.findByText("1 patient");
    await user.type(find(), "Pat{Enter}");
    await waitFor(() =>
      expect(search).toHaveBeenLastCalledWith({
        input: { query: "Pat", offset: 0, limit: 26 },
      }),
    );
    expect(await screen.findByText("1 matching patient")).toBeTruthy();
    expect(router.state.location.search).toBe("");
    await user.click(screen.getByRole("button", { name: "Show all patients" }));
    expect(await screen.findByText("1 patient")).toBeTruthy();
    expect(find()).toHaveProperty("value", "");
    expect(search).toHaveBeenLastCalledWith({ input: { offset: 0, limit: 26 } });
  });

  it("keeps detailed identifier search available with the keyboard", async () => {
    const search = vi.fn(() => [SUMMARY]);
    const { user } = renderPage(search);
    await tabTo(user, screen.getByRole("button", { name: "More filters" }));
    await user.keyboard("{Enter}");
    await typeInto(user, /^Identifier$/, "MRN-00001");
    await typeInto(user, /^Assigning authority/, "SYN-CLINIC");
    await typeInto(user, /^Identifier type/, "MR{Enter}");
    await waitFor(() =>
      expect(search).toHaveBeenLastCalledWith({
        input: {
          identifier: {
            assigningAuthority: "SYN-CLINIC",
            identifierType: "MR",
            value: "MRN-00001",
          },
          limit: 26,
          offset: 0,
        },
      }),
    );
    expect(screen.getByText("Detailed search applied")).toBeTruthy();
  });

  it("preserves a space typed between names when typing pauses", async () => {
    const search = vi.fn(() => [SUMMARY]);
    const { user } = renderPage(search);
    await user.type(find(), "Test ");
    await waitFor(() =>
      expect(search).toHaveBeenLastCalledWith({
        input: { query: "Test ", offset: 0, limit: 26 },
      }),
    );
    expect(find()).toHaveProperty("value", "Test ");
    await user.type(find(), "Patient{Enter}");
    await waitFor(() =>
      expect(search).toHaveBeenLastCalledWith({
        input: { query: "Test Patient", offset: 0, limit: 26 },
      }),
    );
  });

  it("cancels pending quick search when a detailed search is submitted", async () => {
    const search = vi.fn(() => [SUMMARY]);
    const { user } = renderPage(search);
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.type(screen.getByRole("textbox", { name: /^Family name/ }), "Pat");
    await user.type(find(), "X");
    await user.click(screen.getByRole("button", { name: "Find by name" }));
    // Allow the previous field's 250ms debounce to expire, if it was not cancelled.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 350));
    });
    expect(search).toHaveBeenLastCalledWith({
      input: { familyName: "Pat", givenNames: null, birthDate: null, limit: 26, offset: 0 },
    });
    expect(find()).toHaveProperty("value", "");
    expect(screen.getByText("Detailed search applied")).toBeTruthy();
  });

  it("finds by name and starts a new order from a row's actions", async () => {
    const search = vi.fn(() => [SUMMARY, OTHER_SUMMARY]);
    const { user } = renderPage(search);
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.type(screen.getByRole("textbox", { name: /^Family name/ }), "Pat{Enter}");
    await waitFor(() =>
      expect(search).toHaveBeenCalledWith({
        input: { familyName: "Pat", givenNames: null, birthDate: null, limit: 26, offset: 0 },
      }),
    );
    const grid = await screen.findByRole("grid", { name: "Patients" });
    expect(within(grid).getAllByText("Patient, Test")).toHaveLength(2);
    expect(within(grid).getByText("None recorded")).toBeTruthy();
    expect(screen.queryByRole("checkbox")).toBeNull();
    const [, otherActions] = within(grid).getAllByRole("button", {
      name: "Actions for Patient, Test",
    });
    if (!otherActions) throw new Error("no actions for the second patient");
    await user.click(otherActions);
    await user.click(await screen.findByRole("menuitem", { name: "New order" }));
    expect(
      await screen.findByText(`Navigated to /orders/new?subjectId=${OTHER_SUMMARY.subjectId}`),
    ).toBeTruthy();
  });

  it("checks detailed search presence and date format before sending criteria", async () => {
    const search = vi.fn(() => []);
    const { user } = renderPage(search);
    await screen.findByText("No patients yet");
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.type(screen.getByRole("textbox", { name: /^Birth date/ }), "01/01/1970");
    await user.click(screen.getByRole("button", { name: "Find by name" }));
    expect(await screen.findByText("Enter family name.")).toBeTruthy();
    expect(screen.getByText("Enter the date as YYYY-MM-DD.")).toBeTruthy();
    expect(search).toHaveBeenCalledTimes(1);
  });

  it("distinguishes an empty directory from no search matches and offers recovery", async () => {
    const { user } = renderPage(() => []);
    expect(await screen.findByText("No patients yet")).toBeTruthy();
    await user.type(find(), "Nobody{Enter}");
    expect(await screen.findByText("No matching patients")).toBeTruthy();
    await user.click(screen.getByText("Clear search"));
    expect(await screen.findByText("No patients yet")).toBeTruthy();
    await user.click(screen.getAllByRole("button", { name: "Register patient" })[0] as HTMLElement);
    expect(await screen.findByText("Navigated to /patients/register")).toBeTruthy();
  });

  it("shows loading, then keeps previous rows while the search updates", async () => {
    let finish: (rows: SubjectSummary[]) => void = () => {};
    const search = vi
      .fn()
      .mockReturnValueOnce(
        new Promise<SubjectSummary[]>((resolve) => {
          finish = resolve;
        }),
      )
      .mockImplementationOnce(
        () =>
          new Promise<SubjectSummary[]>((resolve) => {
            finish = resolve;
          }),
      );
    const { user } = renderPage(search);
    expect(screen.getByText("Loading patients…")).toBeTruthy();
    finish([SUMMARY]);
    await screen.findByRole("gridcell", { name: "Patient, Test" });
    await user.type(find(), "Nobody{Enter}");
    expect(await screen.findByText("Updating patients…")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Patient, Test" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Next" })).toHaveProperty("disabled", true);
    finish([]);
    expect(await screen.findByText("No matching patients")).toBeTruthy();
  });

  it("shows a permission refusal with its support reference and no retry", async () => {
    renderPage(() => Promise.reject(engineError("forbidden", "You cannot search patients.")));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot search patients.");
    expect(alert.textContent).toContain("A laboratory administrator can change roles.");
    expect(alert.textContent).toContain("Support reference: corr-test-0001");
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("retries a directory read the engine could not run", async () => {
    const search = vi
      .fn()
      .mockRejectedValueOnce(engineError("unavailable", "The database is busy."))
      .mockReturnValue([SUMMARY]);
    const { user } = renderPage(search);
    await user.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("gridcell", { name: "Patient, Test" })).toBeTruthy();
  });

  it("pages through all patients, hides the lookahead row, and resets paging on search", async () => {
    const full = Array.from({ length: 26 }, (_, index) => ({
      ...SUMMARY,
      familyName: `Patient${index}`,
      subjectId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`,
    }));
    const search = vi
      .fn()
      .mockReturnValueOnce(full)
      .mockReturnValueOnce([full[25]])
      .mockReturnValueOnce(full)
      .mockReturnValueOnce([])
      .mockReturnValue([SUMMARY]);
    const { user } = renderPage(search);
    expect(await screen.findByText("Patients 1–25")).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "Patient25, Test" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("Patients 26–26")).toBeTruthy();
    expect(search).toHaveBeenLastCalledWith({ input: { offset: 25, limit: 26 } });
    expect(screen.getByRole("button", { name: "Next" })).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("button", { name: "Previous" }));
    expect(await screen.findByText("Patients 1–25")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(await screen.findByText("0 patients on this page")).toBeTruthy();
    expect(screen.queryByText("0 patients", { exact: true })).toBeNull();
    expect(screen.getByText("No more patients")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Previous" })).toHaveProperty("disabled", false);
    await user.type(find(), "Pat{Enter}");
    await waitFor(() =>
      expect(search).toHaveBeenLastCalledWith({ input: { query: "Pat", offset: 0, limit: 26 } }),
    );
  });

  it("opens a patient with a single click on any cell", async () => {
    const { user } = renderPage(() => [SUMMARY]);
    await screen.findByRole("gridcell", { name: "Patient, Test" });
    await openRow(user, "MRN-00001 (SYN-CLINIC MR)");
    expect(await screen.findByText(`Navigated to /patients/${SUBJECT_ID}`)).toBeTruthy();
  });
});
