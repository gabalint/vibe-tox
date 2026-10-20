import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  AMPHETAMINE,
  CURRENT,
  CURRENT_VIEW,
  DRAFT,
  DRAFT_VIEW,
  OPIATE,
  sessionWith,
} from "../testData";
import { ClassMappingsPage } from "./ClassMappingsPage";

function renderPage(commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({
    current_session: () => sessionWith(["billing"]),
    list_analytes: () => [AMPHETAMINE, OPIATE],
    get_class_mapping_version: () => CURRENT_VIEW,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/billing/class-mappings", element: <ClassMappingsPage /> }],
    "/billing/class-mappings",
  );
}

describe("ClassMappingsPage (REQ-BIL-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("says which version is in effect and which awaits approval, and shows the draft", async () => {
    const getVersion = vi.fn((args: unknown) =>
      JSON.stringify(args).includes(DRAFT.versionId) ? DRAFT_VIEW : CURRENT_VIEW,
    );
    const { container } = renderPage({
      list_class_mapping_versions: () => [CURRENT, DRAFT],
      get_class_mapping_version: getVersion,
    });
    expect(
      await screen.findByText("Version 1 in effect, version 2 awaiting approval"),
    ).toBeTruthy();
    // Without `?version=`, the draft awaiting approval is shown.
    expect(await screen.findByRole("heading", { level: 2, name: "Version 2" })).toBeTruthy();
    expect(getVersion).toHaveBeenCalledWith({ input: { versionId: DRAFT.versionId } });
    expect(getVersion).not.toHaveBeenCalledWith({ input: { versionId: CURRENT.versionId } });
    const versions = screen.getByRole("grid", { name: "Versions" });
    const rows = within(versions).getAllByRole("row").slice(1);
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("Version 2Draft"),
      expect.stringContaining("Version 1Current"),
    ]);
    expect(rows[0]?.getAttribute("aria-current")).toBe("true");
    expect(screen.getByRole("button", { name: "Approve version" })).toBeTruthy();
    expect(screen.queryByText(/double-click/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("moves between versions with the list, keeping the version in the address", async () => {
    const { user, router } = renderPage({
      list_class_mapping_versions: () => [CURRENT, DRAFT],
      get_class_mapping_version: (args) =>
        JSON.stringify(args).includes(DRAFT.versionId) ? DRAFT_VIEW : CURRENT_VIEW,
    });
    await screen.findByRole("heading", { level: 2, name: "Version 2" });
    await openRow(user, "Version 1");
    expect(await screen.findByRole("heading", { level: 2, name: "Version 1" })).toBeTruthy();
    expect(router.state.location.search).toBe("?version=1");
    expect(router.state.historyAction).toBe("REPLACE");
    const row = screen.getByRole("gridcell", { name: "Version 1" }).closest("[role=row]");
    expect(row?.getAttribute("aria-current")).toBe("true");
    expect(row?.contains(document.activeElement)).toBe(true);
    // Only the actions the engine allows on the version shown.
    expect(screen.getByRole("button", { name: "Retire version" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve version" })).toBeNull();
  });

  it("shows the version in effect when nothing awaits", async () => {
    const { user } = renderPage({ list_class_mapping_versions: () => [CURRENT] });
    expect(await screen.findByText("Version 1 in effect")).toBeTruthy();
    expect(await screen.findByRole("heading", { level: 2, name: "Version 1" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retire version" })).toBeTruthy();
    await openRowWithEnter(user, "Version 1");
    expect(screen.getByRole("heading", { level: 2, name: "Version 1" })).toBeTruthy();
  });

  it("does not show another version for a number the mapping does not have", async () => {
    mockCommands({
      current_session: () => sessionWith(["billing"]),
      list_analytes: () => [AMPHETAMINE, OPIATE],
      list_class_mapping_versions: () => [CURRENT],
      get_class_mapping_version: () => CURRENT_VIEW,
    });
    renderRoutes(
      [{ path: "/billing/class-mappings", element: <ClassMappingsPage /> }],
      "/billing/class-mappings?version=9",
    );
    expect(await screen.findByText("No version 9")).toBeTruthy();
    expect(screen.queryByRole("heading", { level: 2, name: "Version 1" })).toBeNull();
  });

  it("starts a new draft from the current version's entries and opens it", async () => {
    const draft = vi.fn(() => DRAFT_VIEW);
    const getVersion = vi.fn(() => CURRENT_VIEW);
    const { user, router } = renderPage({
      list_class_mapping_versions: () => [CURRENT],
      get_class_mapping_version: getVersion,
      draft_class_mapping_version: draft,
    });
    await screen.findByRole("heading", { level: 2, name: "Version 1" });
    await user.click(screen.getByRole("button", { name: "New draft" }));
    const dialog = await screen.findByRole("dialog", { name: "New class mapping draft" });
    expect(getVersion).toHaveBeenCalledWith({ input: { versionId: CURRENT.versionId } });
    expect(within(dialog).getByRole("textbox", { name: /Class code/ })).toHaveProperty(
      "value",
      "SYN-C1",
    );
    await user.click(within(dialog).getByRole("button", { name: "Add entry" }));
    await chooseWithKeyboard(user, /Select.*Analyte$/, "SYN-OPI Synthetic opiate");
    const second = within(dialog).getByRole("group", { name: "Entry 2" });
    await user.type(within(second).getByRole("textbox", { name: /Class code/ }), " SYN-C2 ");
    await user.type(within(second).getByRole("textbox", { name: /Class label/ }), "Class two");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    // The new draft shows on this page, by its number.
    await vi.waitFor(() => expect(router.state.location.search).toBe("?version=2"));
    expect(draft).toHaveBeenCalledWith({
      input: {
        entries: [
          { analyteId: AMPHETAMINE.id, classCode: "SYN-C1", classLabel: "Synthetic class 1" },
          { analyteId: OPIATE.id, classCode: "SYN-C2", classLabel: "Class two" },
        ],
      },
    });
  });

  it("starts an empty draft without suggesting a class, and shows the engine's refusal", async () => {
    const { user } = renderPage({
      list_class_mapping_versions: () => [],
      draft_class_mapping_version: () =>
        Promise.reject(engineError("conflict", "A class mapping draft already exists.")),
    });
    await screen.findByText("No class mapping yet");
    await user.click(screen.getByRole("button", { name: "New draft" }));
    const dialog = await screen.findByRole("dialog", { name: "New class mapping draft" });
    expect(within(dialog).getByRole("textbox", { name: /Class code/ })).toHaveProperty("value", "");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await within(dialog).findByText("Choose analyte.")).toBeTruthy();
    await chooseWithKeyboard(user, /Analyte$/, "SYN-AMP Synthetic amphetamine");
    await user.type(within(dialog).getByRole("textbox", { name: /Class code/ }), "SYN-C1");
    await user.type(within(dialog).getByRole("textbox", { name: /Class label/ }), "Class one");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await within(dialog).findByText("A class mapping draft already exists.")).toBeTruthy();
  });

  it("shows the engine's error", async () => {
    renderPage({
      list_class_mapping_versions: () =>
        Promise.reject(engineError("forbidden", "Class mappings need the billing capability.")),
    });
    expect(await screen.findByText("Class mappings need the billing capability.")).toBeTruthy();
  });
});
