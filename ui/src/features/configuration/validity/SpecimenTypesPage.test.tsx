import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { APPROVER, MANAGER, sessionWith, URINE } from "../testConfiguration";
import { SpecimenTypesPage } from "./SpecimenTypesPage";

const ORAL_FLUID = {
  id: "00000000-0000-7000-8000-000000000a02",
  code: "oral",
  name: "Oral fluid",
  versionInEffect: null,
  draftVersion: 1,
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = MANAGER,
  path = "/configuration/specimen-types",
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_specimen_types: () => [URINE, ORAL_FLUID],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/specimen-types", element: <SpecimenTypesPage /> }],
    path,
  );
}

describe("SpecimenTypesPage (REQ-SPC-008)", () => {
  beforeEach(mockGridLayout);

  it("lists specimen types with their validity policy's state, a draft as a warning", async () => {
    const { container } = renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "Specimen types" })).toBeTruthy();
    expect(await screen.findByText("No version in effect")).toBeTruthy();
    const draft = screen.getByText("v1 awaiting approval");
    expect(draft.closest("[data-tone]")?.getAttribute("data-tone")).toBe("warning");
    expect(
      screen.getByText("No version in effect").closest("[data-tone]")?.getAttribute("data-tone"),
    ).toBe("neutral");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens a specimen type's page by click and by Enter", async () => {
    const first = renderPage();
    await openRow(first.user, "Urine");
    expect(
      await screen.findByText(`Navigated to /configuration/specimen-types/${URINE.id}`),
    ).toBeTruthy();
    first.unmount();
    const { user } = renderPage();
    await openRowWithEnter(user, "oral");
    expect(
      await screen.findByText(`Navigated to /configuration/specimen-types/${ORAL_FLUID.id}`),
    ).toBeTruthy();
  });

  it("adds a specimen type and opens its page", async () => {
    const create = vi.fn(() => "00000000-0000-7000-8000-000000000a03");
    const { user } = renderPage({ create_specimen_type: create });
    await user.click(await screen.findByRole("button", { name: "Add specimen type" }));
    const dialog = await screen.findByRole("dialog", { name: "Add specimen type" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Code/ }), "hair");
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), "Hair");
    await user.click(within(dialog).getByRole("button", { name: "Add specimen type" }));
    expect(
      await screen.findByText(
        "Navigated to /configuration/specimen-types/00000000-0000-7000-8000-000000000a03",
      ),
    ).toBeTruthy();
    expect(create).toHaveBeenCalledWith({ input: { code: "hair", name: "Hair" } });
  });

  it("offers an approver no Add", async () => {
    renderPage({}, APPROVER);
    expect(await screen.findByRole("gridcell", { name: "urine" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add specimen type" })).toBeNull();
  });

  it("shows the empty, no-results and error states", async () => {
    const empty = renderPage({ list_specimen_types: () => [] }, APPROVER);
    expect(screen.getByText("Loading specimen types…")).toBeTruthy();
    expect(await screen.findByText("No specimen types yet")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add specimen type" })).toBeNull();
    empty.unmount();
    const found = renderPage({}, MANAGER, "/configuration/specimen-types?q=blood");
    expect(await screen.findByText("No specimen types match 'blood'")).toBeTruthy();
    found.unmount();
    renderPage({
      list_specimen_types: () => Promise.reject(engineError("internal", "Synthetic failure.")),
    });
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The specimen types could not be loaded",
    );
  });
});
