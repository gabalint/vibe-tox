import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { OTHER_SUBJECT_ID, OTHER_SUMMARY, SUBJECT_ID, SUMMARY } from "../testData";
import { RegisterPatientPage } from "./RegisterPatientPage";

const NO_MATCHES = { identifierMatches: [], candidates: [] };
const REGISTERED = { subjectId: SUBJECT_ID, revision: 1, identityHoldId: null };

function renderPage(
  commands: Record<string, (args: unknown) => unknown>,
  path = "/patients/register",
) {
  mockCommands(commands);
  return renderRoutes([{ path: "/patients/register", element: <RegisterPatientPage /> }], path);
}

type User = ReturnType<typeof renderPage>["user"];

async function enterDetails(user: User) {
  await user.keyboard("Patient"); // the family name field has focus on arrival
  await typeInto(user, /^Given names/, "Test");
  await typeInto(user, /^Birth date/, "1970-01-01");
  await typeInto(user, /^Source of these details/, "Paper requisition");
  await tabTo(user, screen.getByRole("button", { name: "Add identifier" }));
  await user.keyboard("{Enter}");
  await typeInto(user, "Identifier", "MRN-00009");
  await typeInto(user, /^Assigning authority/, "SYN-CLINIC");
  await typeInto(user, /^Identifier type/, "MR");
  await tabTo(user, screen.getByRole("button", { name: "Check for existing patients" }));
  await user.keyboard("{Enter}");
}

const EXPECTED_QUERY = {
  demographics: {
    familyName: "Patient",
    givenNames: "Test",
    birthDate: "1970-01-01",
    administrativeSex: null,
  },
  identifiers: [{ assigningAuthority: "SYN-CLINIC", identifierType: "MR", value: "MRN-00009" }],
};

describe("RegisterPatientPage (REQ-IDN-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("registers a new patient with the keyboard only when nothing matches", async () => {
    const match = vi.fn(() => NO_MATCHES);
    const register = vi.fn(() => REGISTERED);
    const { user, container } = renderPage({
      match_subject_candidates: match,
      register_subject: register,
    });
    await expectNoA11yViolations(container, { wholePage: true });
    await enterDetails(user);
    expect(await screen.findByText("No existing patient matches these details")).toBeTruthy();
    expect(match).toHaveBeenCalledWith({ input: EXPECTED_QUERY });
    await expectNoA11yViolations(container, { wholePage: true });

    await tabTo(user, screen.getByRole("button", { name: "Register patient" }));
    await user.keyboard("{Enter}");
    expect(await screen.findByText(`Navigated to /patients/${SUBJECT_ID}`)).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: { ...EXPECTED_QUERY, source: "Paper requisition", acknowledgedCandidates: [] },
    });
    expect(screen.getByText("Patient registered")).toBeTruthy();
  });

  it("requires acknowledging name and birth date candidates, and sends exactly those shown", async () => {
    const register = vi.fn(() => ({ ...REGISTERED, identityHoldId: "hold-1" }));
    const { user } = renderPage({
      match_subject_candidates: () => ({ identifierMatches: [], candidates: [OTHER_SUMMARY] }),
      register_subject: register,
    });
    await enterDetails(user);
    expect(await screen.findByText("1 possible matches by name and birth date")).toBeTruthy();
    const registerButton = screen.getByRole("button", { name: "Register patient" });
    expect(registerButton.hasAttribute("disabled")).toBe(true);

    await user.click(
      screen.getByRole("checkbox", {
        name: "I compared these records; this is a different person",
      }),
    );
    await user.click(registerButton);
    expect(await screen.findByText("Patient registered on an identity hold")).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: expect.objectContaining({ acknowledgedCandidates: [OTHER_SUBJECT_ID] }),
    });
  });

  it("offers only the existing record when an identifier already belongs to a patient", async () => {
    const register = vi.fn();
    const { user } = renderPage({
      match_subject_candidates: () => ({ identifierMatches: [SUMMARY], candidates: [] }),
      register_subject: register,
    });
    await enterDetails(user);
    expect(await screen.findByText("This identifier already belongs to a patient")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Register patient" })).toBeNull();
    // The existing patient opens from the grid; the typed details are lost, so it asks first.
    expect(screen.getByRole("grid", { name: "Patients holding these identifiers" })).toBeTruthy();
    await openRow(user, "MRN-00001 (SYN-CLINIC MR)");
    const leave = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(leave).getByRole("button", { name: "Leave without saving" }));
    expect(await screen.findByText(`Navigated to /patients/${SUMMARY.subjectId}`)).toBeTruthy();
  });

  it("asks before leaving typed details, and stays when asked to", async () => {
    const { user } = renderPage({});
    await user.keyboard("Patient");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    const leave = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(leave).getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("textbox", { name: /^Family name/ })).toHaveProperty(
      "value",
      "Patient",
    );
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("leaves at once when nothing was typed", async () => {
    const { user } = renderPage({});
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("Navigated to /patients")).toBeTruthy();
  });

  it("shows the candidates again when they changed since they were shown (conflict)", async () => {
    const match = vi
      .fn()
      .mockReturnValueOnce(NO_MATCHES)
      .mockReturnValue({ identifierMatches: [], candidates: [OTHER_SUMMARY] });
    const { user } = renderPage({
      match_subject_candidates: match,
      register_subject: () =>
        Promise.reject(engineError("conflict", "The possible matches changed. Review them again.")),
    });
    await enterDetails(user);
    await user.click(await screen.findByRole("button", { name: "Register patient" }));
    expect(
      await screen.findByText("The possible matches changed. Review them again."),
    ).toBeTruthy();
    expect(await screen.findByText("1 possible matches by name and birth date")).toBeTruthy();
    expect(match).toHaveBeenCalledTimes(2);
  });

  it("returns to the details with the engine's field errors when it refuses them", async () => {
    const { user } = renderPage({
      match_subject_candidates: () =>
        Promise.reject(
          engineError("validation", "Check the highlighted fields.", {
            fieldErrors: [
              { field: "birthDate", code: "invalid_date", message: "Enter a real calendar date." },
            ],
          }),
        ),
    });
    await enterDetails(user);
    await user.click(await screen.findByRole("button", { name: "Change details" }));
    expect(await screen.findByText("Enter a real calendar date.")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /^Birth date/ })).toHaveProperty(
      "value",
      "1970-01-01",
    );
  });

  it("checks the required details before looking for matches", async () => {
    const match = vi.fn();
    const { user } = renderPage({ match_subject_candidates: match });
    await user.click(screen.getByRole("button", { name: "Check for existing patients" }));
    expect(await screen.findByText("Enter family name.")).toBeTruthy();
    expect(screen.getByText("Enter birth date.")).toBeTruthy();
    expect(screen.getByText("Enter source of these details.")).toBeTruthy();
    expect(match).not.toHaveBeenCalled();
  });
  it("returns a newly registered patient directly to the new-order task", async () => {
    const register = vi.fn(() => REGISTERED);
    const { user } = renderPage(
      { match_subject_candidates: () => NO_MATCHES, register_subject: register },
      "/patients/register?intent=new-order",
    );
    await enterDetails(user);
    await user.click(await screen.findByRole("button", { name: "Register patient" }));
    expect(
      await screen.findByText(`Navigated to /orders/new?subjectId=${SUBJECT_ID}`),
    ).toBeTruthy();
    expect(register).toHaveBeenCalledTimes(1);
  });

  it("uses an existing identifier match for the order without registering it again", async () => {
    const register = vi.fn();
    const { user } = renderPage(
      {
        match_subject_candidates: () => ({ identifierMatches: [SUMMARY], candidates: [] }),
        register_subject: register,
      },
      "/patients/register?intent=new-order",
    );
    await enterDetails(user);
    await openRow(user, "MRN-00001 (SYN-CLINIC MR)");
    expect(
      await screen.findByText(`Navigated to /orders/new?subjectId=${SUBJECT_ID}`),
    ).toBeTruthy();
    expect(register).not.toHaveBeenCalled();
  });

  it("ignores unrecognized return intents", async () => {
    const { user } = renderPage({}, "/patients/register?intent=https://example.invalid");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("Navigated to /patients")).toBeTruthy();
  });
});
