import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { APPROVER, MANAGER, sessionWith } from "../testConfiguration";
import { OrderablesPage } from "./OrderablesPage";

const SCREEN = {
  id: "00000000-0000-7000-8000-000000000e01",
  code: "SYN-SCR",
  name: "Synthetic drug screen",
  kind: "panel" as const,
  versionInEffect: 3,
  draftVersion: 4,
};
const CONFIRMATION = {
  id: "00000000-0000-7000-8000-000000000e02",
  code: "SYN-CNF",
  name: "Synthetic confirmation",
  kind: "test" as const,
  versionInEffect: 1,
  draftVersion: null,
};
const NEW_TEST = {
  id: "00000000-0000-7000-8000-000000000e03",
  code: "SYN-NEW",
  name: "Synthetic new test",
  kind: "test" as const,
  versionInEffect: null,
  draftVersion: null,
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = MANAGER,
  path = "/configuration/orderables",
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_orderables: () => [SCREEN, CONFIRMATION, NEW_TEST],
    ...commands,
  });
  return renderRoutes([{ path: "/configuration/orderables", element: <OrderablesPage /> }], path);
}

const codes = () =>
  screen
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("gridcell")[0]?.textContent);

describe("OrderablesPage (REQ-ANA-009)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists tests and panels with their kind and the engine's version state", async () => {
    const { container } = renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "Tests and panels" })).toBeTruthy();
    const cell = (name: string) => screen.getByRole("gridcell", { name });
    expect(
      await screen.findByRole("gridcell", { name: "v3 in effect, v4 awaiting approval" }),
    ).toBeTruthy();
    expect(cell("v1 in effect")).toBeTruthy();
    expect(cell("No version in effect")).toBeTruthy();
    expect(screen.getByText("Panel (one or more components)")).toBeTruthy();
    expect(screen.getByText("3 tests and panels")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("filters by version state and kind in the URL, and clears the filters", async () => {
    const { user, router } = renderPage();
    await screen.findByRole("gridcell", { name: "SYN-SCR" });
    await chooseWithKeyboard(user, /State$/, "Awaiting approval");
    expect(codes()).toEqual(["SYN-SCR"]);
    expect(router.state.location.search).toBe("?state=awaiting-approval");
    await chooseWithKeyboard(user, /State$/, "No version in effect");
    await chooseWithKeyboard(user, /Kind$/, "Tests");
    expect(codes()).toEqual(["SYN-NEW"]);
    expect(screen.getByText("1 of 3 tests and panels")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(await screen.findByRole("gridcell", { name: "SYN-CNF" })).toBeTruthy();
    expect(router.state.location.search).toBe("");
    expect(screen.queryByRole("button", { name: "Clear filters" })).toBeNull();
  });

  it("names the filters when they exclude every row, and clears them", async () => {
    const { user } = renderPage(
      {},
      MANAGER,
      "/configuration/orderables?kind=panel&state=none-in-effect&q=CNF",
    );
    expect(await screen.findByText("No tests and panels match 'CNF'")).toBeTruthy();
    await user.click(screen.getByText("Clear search"));
    expect(await screen.findByText("No tests and panels match the filters")).toBeTruthy();
    const [, emptyClear] = screen.getAllByRole("button", { name: "Clear filters" });
    if (!emptyClear) throw new Error("no Clear filters beside the no-results message");
    await user.click(emptyClear);
    expect(await screen.findByRole("gridcell", { name: "SYN-CNF" })).toBeTruthy();
  });

  it("applies filters and find from the URL", async () => {
    renderPage({}, MANAGER, "/configuration/orderables?kind=test&q=confirm");
    expect(await screen.findByRole("gridcell", { name: "SYN-CNF" })).toBeTruthy();
    expect(codes()).toEqual(["SYN-CNF"]);
    expect(screen.getByRole("searchbox", { name: "Find tests and panels" })).toHaveProperty(
      "value",
      "confirm",
    );
  });

  it("opens a row's page by click and by Enter", async () => {
    const first = renderPage();
    await openRow(first.user, "SYN-SCR");
    expect(
      await screen.findByText(`Navigated to /configuration/orderables/${SCREEN.id}`),
    ).toBeTruthy();
    first.unmount();
    const { user } = renderPage();
    await openRowWithEnter(user, "Synthetic confirmation");
    expect(
      await screen.findByText(`Navigated to /configuration/orderables/${CONFIRMATION.id}`),
    ).toBeTruthy();
  });

  it("adds a test or panel by keyboard and opens its page", async () => {
    const create = vi.fn(() => "00000000-0000-7000-8000-000000000e04");
    const { user } = renderPage({ create_orderable: create });
    await user.click(await screen.findByRole("button", { name: "Add test or panel" }));
    const dialog = await screen.findByRole("dialog", { name: "Add test or panel" });
    await user.keyboard("SYN-QNT");
    await typeInto(user, /^Name/, "Synthetic quantitation");
    await chooseWithKeyboard(user, /Kind$/, "Test (one component)");
    await user.click(within(dialog).getByRole("button", { name: "Add test or panel" }));
    expect(
      await screen.findByText(
        "Navigated to /configuration/orderables/00000000-0000-7000-8000-000000000e04",
      ),
    ).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: { code: "SYN-QNT", name: "Synthetic quantitation", kind: "test" },
    });
  });

  it("offers an approver no Add", async () => {
    renderPage({}, APPROVER);
    expect(await screen.findByRole("gridcell", { name: "SYN-SCR" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add test or panel" })).toBeNull();
  });

  it("shows the empty, no-results and error states", async () => {
    const empty = renderPage({ list_orderables: () => [] });
    expect(screen.getByText("Loading tests and panels…")).toBeTruthy();
    expect(screen.getByRole("searchbox", { name: "Find tests and panels" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Kind/ })).toBeTruthy();
    expect(screen.queryByText("0 tests and panels")).toBeNull();
    expect(await screen.findByText("No tests and panels yet")).toBeTruthy();
    empty.unmount();
    const found = renderPage({}, MANAGER, "/configuration/orderables?q=hair");
    expect(await screen.findByText("No tests and panels match 'hair'")).toBeTruthy();
    found.unmount();
    renderPage({
      list_orderables: () => Promise.reject(engineError("internal", "Synthetic failure.")),
    });
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The tests and panels could not be loaded",
    );
  });
});
