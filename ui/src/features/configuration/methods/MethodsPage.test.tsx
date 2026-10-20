import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  APPROVED_VERSION,
  APPROVER,
  MANAGER,
  METHOD,
  methodVersion,
  sessionWith,
} from "../testConfiguration";
import { MethodPage } from "./MethodPage";
import { MethodsPage } from "./MethodsPage";

const SECOND = {
  id: "00000000-0000-7000-8000-000000000c02",
  code: "SYN-LC",
  name: "Synthetic chromatography",
  versionInEffect: 2,
  draftVersion: 3,
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = MANAGER,
  path = "/configuration/methods",
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_methods: () => [METHOD, SECOND],
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/configuration/methods", element: <MethodsPage /> },
      { path: "/configuration/methods/:methodId", element: <MethodPage /> },
    ],
    path,
  );
}

describe("MethodsPage (REQ-ANA-002)", () => {
  beforeEach(mockGridLayout);

  it("lists methods with the engine's version state", async () => {
    const { container } = renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "Methods" })).toBeTruthy();
    expect(await screen.findByText("v1 in effect")).toBeTruthy();
    expect(screen.getByText("v2 in effect, v3 awaiting approval")).toBeTruthy();
    expect(screen.getByRole("status", { name: "" }).textContent).toBe("2 methods");
    expect(screen.getByRole("button", { name: "Add method" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens a method's page by click, with the breadcrumb back to the found list and Next", async () => {
    const { user, router } = renderPage({
      list_method_versions: () => [APPROVED_VERSION],
      get_method_version: () => methodVersion(APPROVED_VERSION),
    });
    await user.type(await screen.findByRole("searchbox", { name: "Find methods" }), "SYN{Enter}");
    await openRow(user, "SYN-IA");
    expect(
      await screen.findByRole("heading", { level: 1, name: "SYN-IA Synthetic immunoassay" }),
    ).toBeTruthy();
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByRole("link", { name: "Methods" }).getAttribute("href")).toBe(
      "/configuration/methods?q=SYN",
    );
    await user.click(screen.getByRole("link", { name: "Next" }));
    expect(router.state.location.pathname).toBe(`/configuration/methods/${SECOND.id}`);
  });

  it("opens a method's page with Enter", async () => {
    const { user } = renderPage({ list_method_versions: () => new Promise(() => {}) });
    await openRowWithEnter(user, "SYN-LC");
    expect(
      await screen.findByRole("heading", { level: 1, name: "SYN-LC Synthetic chromatography" }),
    ).toBeTruthy();
  });

  it("adds a method and opens its page", async () => {
    const create = vi.fn(() => "00000000-0000-7000-8000-000000000c03");
    const { user, router } = renderPage({
      create_method: create,
      list_method_versions: () => new Promise(() => {}),
    });
    await user.click(await screen.findByRole("button", { name: "Add method" }));
    const dialog = await screen.findByRole("dialog", { name: "Add method" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Code/ }), "SYN-GC ");
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), "Synthetic GC");
    await user.click(within(dialog).getByRole("button", { name: "Add method" }));
    expect(await screen.findByText("Method SYN-GC added")).toBeTruthy();
    expect(router.state.location.pathname).toBe(
      "/configuration/methods/00000000-0000-7000-8000-000000000c03",
    );
    expect(create).toHaveBeenCalledWith({ input: { code: "SYN-GC", name: "Synthetic GC" } });
  });

  it("offers an approver no Add, only the methods", async () => {
    renderPage({}, APPROVER);
    expect(await screen.findByRole("gridcell", { name: "SYN-IA" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add method" })).toBeNull();
  });

  it("tells no methods yet from no methods found", async () => {
    const empty = renderPage({ list_methods: () => [] });
    expect(screen.getByText("Loading methods…")).toBeTruthy();
    expect(await screen.findByText("No methods yet")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Add method" })).toHaveLength(2);
    empty.unmount();
    const { user } = renderPage({}, MANAGER, "/configuration/methods?q=oxy");
    expect(await screen.findByText("No methods match 'oxy'")).toBeTruthy();
    await user.click(screen.getByText("Clear search"));
    expect(await screen.findByRole("gridcell", { name: "SYN-IA" })).toBeTruthy();
  });

  it("shows the engine error with its support reference", async () => {
    renderPage({
      list_methods: () =>
        Promise.reject(engineError("internal", "The app could not complete the action.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The methods could not be loaded");
    expect(alert.textContent).toContain("corr-test-0001");
    expect(screen.getByRole("heading", { level: 1, name: "Methods" })).toBeTruthy();
  });
});
