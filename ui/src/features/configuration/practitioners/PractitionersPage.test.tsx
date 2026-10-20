import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PractitionerView } from "@/generated/PractitionerView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { PractitionersPage } from "./PractitionersPage";

const DOCTOR: PractitionerView = {
  active: true,
  revision: 1,
  id: "00000000-0000-7000-8000-000000000701",
  npi: "1234567893",
  familyName: "Doctor",
  givenNames: "Test",
};

const EXAMPLE: PractitionerView = {
  active: true,
  revision: 1,
  id: "00000000-0000-7000-8000-000000000702",
  npi: "1000000004",
  familyName: "Example",
  givenNames: null,
};

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}, path = "") {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_practitioners: () => [DOCTOR, EXAMPLE],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/practitioners", element: <PractitionersPage /> }],
    `/configuration/practitioners${path}`,
  );
}

describe("PractitionersPage (REQ-ORD-001 references)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists practitioners by name and NPI, and opens one in a panel by click", async () => {
    const { user, router, container } = renderPage();
    expect(await screen.findByRole("gridcell", { name: "Doctor, Test" })).toBeTruthy();
    expect(screen.getByText("2 practitioners")).toBeTruthy();
    await openRow(user, "Doctor, Test");
    const panel = within(await screen.findByRole("region", { name: "Doctor, Test" }));
    expect(panel.getByText("1234567893")).toBeTruthy();
    expect(router.state.location.search).toBe(`?open=${DOCTOR.id}`);
    await expectNoA11yViolations(container);
  });

  it("opens with Enter, and Escape closes the panel with focus back on the row", async () => {
    const { user } = renderPage();
    await screen.findByRole("gridcell", { name: "Example" });
    await openRowWithEnter(user, "Example");
    const panel = await screen.findByRole("region", { name: "Example" });
    expect(within(panel).getByText("Not provided")).toBeTruthy();
    await user.click(within(panel).getByRole("heading", { name: "Example" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "Example" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "Example" })),
    );
  });

  it("opens the practitioner named in the address", async () => {
    renderPage({}, `?open=${EXAMPLE.id}`);
    expect(await screen.findByRole("region", { name: "Example" })).toBeTruthy();
  });

  it("offers Register practitioner only with manage_configuration", async () => {
    renderPage({ current_session: () => sessionWith([]) });
    await screen.findByRole("gridcell", { name: "Doctor, Test" });
    expect(screen.queryByRole("button", { name: "Register practitioner" })).toBeNull();
  });

  it("finds practitioners by name or NPI, and says when none match", async () => {
    const { user } = renderPage();
    await screen.findByRole("gridcell", { name: "Doctor, Test" });
    const find = screen.getByRole("searchbox", { name: "Find practitioners" });
    await user.type(find, "10000{Enter}");
    expect(await screen.findByText("1 of 2 practitioners")).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "Doctor, Test" })).toBeNull();
    await user.clear(find);
    await user.type(find, "nobody{Enter}");
    expect(await screen.findByText("No practitioners match 'nobody'")).toBeTruthy();
  });

  it("registers a practitioner by keyboard, shows the NPI refusal, then opens the new record", async () => {
    const added: PractitionerView = {
      active: true,
      revision: 1,
      id: "00000000-0000-7000-8000-000000000703",
      npi: "1234567893",
      familyName: "Newcomer",
      givenNames: null,
    };
    const register = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "npi", code: "invalid_npi", message: "Enter a valid 10-digit NPI." },
          ],
        }),
      )
      .mockReturnValue(added.id);
    const list = vi.fn(() => [DOCTOR, EXAMPLE]);
    const { user, router } = renderPage({
      register_practitioner: register,
      list_practitioners: list,
    });
    await screen.findByRole("gridcell", { name: "Doctor, Test" });
    list.mockReturnValue([DOCTOR, EXAMPLE, added]);
    await tabTo(user, screen.getByRole("button", { name: "Register practitioner" }));
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog", { name: "Register practitioner" });
    await user.keyboard("1234567890");
    await typeInto(user, /^Family name/, "Newcomer");
    await user.keyboard("{Enter}");
    expect(await within(dialog).findByText("Enter a valid 10-digit NPI.")).toBeTruthy();
    const npi = within(dialog).getByRole("textbox", { name: /^NPI/ });
    await tabTo(user, npi);
    await user.keyboard("{Control>}a{/Control}1234567893{Enter}");
    expect(await screen.findByText("Newcomer registered")).toBeTruthy();
    expect(register).toHaveBeenLastCalledWith({
      input: { npi: "1234567893", familyName: "Newcomer", givenNames: null },
    });
    expect(await screen.findByRole("region", { name: "Newcomer" })).toBeTruthy();
    expect(router.state.location.search).toBe(`?open=${added.id}`);
  });

  it("shows loading under the header, then the empty state", async () => {
    renderPage({ list_practitioners: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "Practitioners" })).toBeTruthy();
    expect(screen.getByText("Loading practitioners…")).toBeTruthy();
    expect(await screen.findByText("No practitioners yet")).toBeTruthy();
  });

  it("shows the engine's error, and who can help when it refuses", async () => {
    renderPage({
      list_practitioners: () =>
        Promise.reject(engineError("forbidden", "You cannot see practitioners.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot see practitioners.");
    expect(alert.textContent).toContain("A laboratory administrator can change roles.");
  });
  it("corrects a practitioner's supplied facts without replacing the identity", async () => {
    const save = vi.fn(() => ({ ...DOCTOR, familyName: "Corrected Doctor", revision: 2 }));
    const { user } = renderPage({ update_practitioner: save }, `?open=${DOCTOR.id}`);
    await user.click(await screen.findByRole("button", { name: "Correct practitioner" }));
    const dialog = await screen.findByRole("dialog", { name: "Correct practitioner" });
    const family = within(dialog).getByRole("textbox", { name: /Family name/ });
    await user.clear(family);
    await user.type(family, "Corrected Doctor");
    await chooseWithKeyboard(user, /Availability for new orders$/, "Retired");
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Reason/ }),
      "Supplied retirement evidence",
    );
    await user.click(within(dialog).getByRole("button", { name: "Save correction" }));
    expect(await screen.findByText("Practitioner correction saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        practitionerId: DOCTOR.id,
        expectedRevision: 1,
        npi: DOCTOR.npi,
        familyName: "Corrected Doctor",
        givenNames: DOCTOR.givenNames,
        active: false,
        reason: "Supplied retirement evidence",
      },
    });
  });
});
