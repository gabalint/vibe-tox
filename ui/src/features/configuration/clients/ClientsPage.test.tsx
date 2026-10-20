import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClientView } from "@/generated/ClientView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { ClientPage } from "./ClientPage";
import { ClientsPage } from "./ClientsPage";

const NORTH: ClientView = {
  active: true,
  revision: 1,
  id: "00000000-0000-7000-8000-000000000601",
  code: "syn-clinic",
  name: "Synthetic Clinic",
  locations: [
    {
      active: true,
      revision: 1,
      id: "00000000-0000-7000-8000-000000000602",
      code: "north",
      name: "North Office",
    },
    {
      active: true,
      revision: 1,
      id: "00000000-0000-7000-8000-000000000603",
      code: "south",
      name: "South Office",
    },
  ],
};

const EAST: ClientView = {
  active: true,
  revision: 1,
  id: "00000000-0000-7000-8000-000000000611",
  code: "syn-east",
  name: "Synthetic East Practice",
  locations: [],
};

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}, path = "") {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_clients: () => [NORTH, EAST],
    list_delivery_destinations: () => [],
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/configuration/clients", element: <ClientsPage /> },
      { path: "/configuration/clients/:clientId", element: <ClientPage /> },
    ],
    `/configuration/clients${path}`,
  );
}

describe("ClientsPage (REQ-ORD-001 references)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists clients with their count of locations", async () => {
    const { container } = renderPage();
    expect(await screen.findByRole("gridcell", { name: "syn-clinic" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Synthetic East Practice" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "2" })).toBeTruthy();
    expect(screen.getByText("2 clients")).toBeTruthy();
    expect(screen.queryByText(/Select/)).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("opens a client's page by click, with the list in the breadcrumb and Next", async () => {
    const { user, router } = renderPage();
    await openRow(user, "syn-clinic");
    expect(
      await screen.findByRole("heading", { level: 1, name: "syn-clinic Synthetic Clinic" }),
    ).toBeTruthy();
    expect(router.state.location.pathname).toBe(`/configuration/clients/${NORTH.id}`);
    const breadcrumb = within(screen.getByRole("navigation", { name: "Breadcrumb" }));
    expect(breadcrumb.getByRole("link", { name: "Clients" }).getAttribute("href")).toBe(
      "/configuration/clients",
    );
    await user.click(screen.getByRole("link", { name: /Next/ }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "syn-east Synthetic East Practice" }),
    ).toBeTruthy();
  });

  it("opens a client's page with Enter", async () => {
    const { user, router } = renderPage();
    await screen.findByRole("gridcell", { name: "syn-east" });
    await openRowWithEnter(user, "syn-east");
    expect(await screen.findByRole("heading", { level: 1, name: /^syn-east/ })).toBeTruthy();
    expect(router.state.location.pathname).toBe(`/configuration/clients/${EAST.id}`);
  });

  it("offers Add client only with manage_configuration", async () => {
    renderPage({ current_session: () => sessionWith([]) });
    await screen.findByRole("gridcell", { name: "syn-clinic" });
    expect(screen.queryByRole("button", { name: "Add client" })).toBeNull();
  });

  it("finds clients by code or name, and says when none match", async () => {
    const { user, router } = renderPage();
    await screen.findByRole("gridcell", { name: "syn-clinic" });
    const find = screen.getByRole("searchbox", { name: "Find clients" });
    await user.type(find, "east{Enter}");
    expect(await screen.findByText("1 of 2 clients")).toBeTruthy();
    expect(router.state.location.search).toBe("?q=east");
    await user.type(find, "x{Enter}");
    expect(await screen.findByText("No clients match 'eastx'")).toBeTruthy();
  });

  it("adds a client, then opens its page", async () => {
    const added: ClientView = {
      active: true,
      revision: 1,
      id: "00000000-0000-7000-8000-000000000621",
      code: "syn-west",
      name: "Synthetic West",
      locations: [],
    };
    const create = vi.fn(() => added.id);
    const list = vi.fn(() => [NORTH, EAST]);
    const { user, router } = renderPage({ create_client: create, list_clients: list });
    await screen.findByRole("gridcell", { name: "syn-clinic" });
    list.mockReturnValue([NORTH, EAST, added]);
    await user.click(screen.getByRole("button", { name: "Add client" }));
    const dialog = await screen.findByRole("dialog", { name: "Add client" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Code/ }), " syn-west ");
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), "Synthetic West");
    await user.click(within(dialog).getByRole("button", { name: "Add client" }));
    expect(await screen.findByText("Client Synthetic West added")).toBeTruthy();
    expect(create).toHaveBeenCalledWith({ input: { code: "syn-west", name: "Synthetic West" } });
    expect(
      await screen.findByRole("heading", { level: 1, name: "syn-west Synthetic West" }),
    ).toBeTruthy();
    expect(router.state.location.pathname).toBe(`/configuration/clients/${added.id}`);
  });

  it("shows the empty state with the first action", async () => {
    renderPage({ list_clients: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "Clients" })).toBeTruthy();
    expect(screen.getByText("Loading clients…")).toBeTruthy();
    expect(await screen.findByText("No clients yet")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Add client" })).toHaveLength(2);
  });

  it("shows the engine's error, and who can help when it refuses", async () => {
    renderPage({
      list_clients: () => Promise.reject(engineError("forbidden", "You cannot see clients.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot see clients.");
    expect(alert.textContent).toContain("A laboratory administrator can change roles.");
  });
});
