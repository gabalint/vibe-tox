import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClientView } from "@/generated/ClientView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import type { DestinationView } from "../destinations/contract";
import { MANAGER, sessionWith } from "../testConfiguration";
import { ClientPage } from "./ClientPage";

const CLIENT: ClientView = {
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
  ],
};

const FAX: DestinationView = {
  destinationId: "00000000-0000-7000-8000-000000000c01",
  clientId: CLIENT.id,
  clientLocationId: null,
  channel: "fax",
  address: "312-555-0100",
  label: "Main fax",
  verified: null,
  state: "unverified",
  retired: false,
  retireReason: null,
  replacesId: null,
  createdBy: { userId: "00000000-0000-7000-8000-000000000e08", displayName: "Test Configurer" },
  revision: 1,
  allowedActions: ["verify_delivery_destination"],
};

const OLD_FAX: DestinationView = {
  ...FAX,
  destinationId: "00000000-0000-7000-8000-000000000c02",
  label: "Old fax",
  state: "retired",
  retired: true,
  allowedActions: [],
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  clientId = CLIENT.id,
) {
  const listDestinations = vi.fn(() => [FAX, OLD_FAX]);
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_clients: () => [CLIENT],
    list_delivery_destinations: listDestinations,
    ...commands,
  });
  const rendered = renderRoutes(
    [{ path: "/configuration/clients/:clientId", element: <ClientPage /> }],
    `/configuration/clients/${clientId}`,
  );
  return { ...rendered, listDestinations };
}

const section = (name: string) => within(screen.getByRole("region", { name }));

describe("ClientPage (REQ-ORD-001 references, REQ-RPT-012)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("shows the client's details, locations and destinations that are not retired", async () => {
    const { container, listDestinations } = renderPage();
    expect(
      await screen.findByRole("heading", { level: 1, name: "syn-clinic Synthetic Clinic" }),
    ).toBeTruthy();
    const breadcrumb = within(screen.getByRole("navigation", { name: "Breadcrumb" }));
    expect(breadcrumb.getByRole("link", { name: "Clients" })).toBeTruthy();
    expect(breadcrumb.getByRole("link", { name: "syn-clinic" })).toBeTruthy();
    expect(section("Locations").getByRole("gridcell", { name: "North Office" })).toBeTruthy();
    const destinations = section("Delivery destinations");
    expect(await destinations.findByRole("gridcell", { name: "Main fax" })).toBeTruthy();
    expect(destinations.queryByRole("gridcell", { name: "Old fax" })).toBeNull();
    expect(listDestinations).toHaveBeenCalledWith({ input: { clientId: CLIENT.id } });
    await expectNoA11yViolations(container);
  });

  it("opens a destination's panel in the destinations directory, by click or Enter", async () => {
    const { user, router } = renderPage();
    await openRow(user, "Main fax");
    expect(await screen.findByText(/Navigated to \/configuration\/destinations/)).toBeTruthy();
    expect(router.state.location.search).toBe(`?open=${FAX.destinationId}`);
    await router.navigate(-1);
    await screen.findByRole("gridcell", { name: "Main fax" });
    await openRowWithEnter(user, "Main fax");
    expect(await screen.findByText(/Navigated to \/configuration\/destinations/)).toBeTruthy();
  });

  it("adds a location from its section", async () => {
    const add = vi.fn(() => "00000000-0000-7000-8000-000000000603");
    const { user } = renderPage({ add_client_location: add });
    await screen.findByRole("heading", { level: 1, name: /^syn-clinic/ });
    await user.click(section("Locations").getByRole("button", { name: "Add location" }));
    const dialog = await screen.findByRole("dialog", { name: "Add location to Synthetic Clinic" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Code/ }), "south");
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), " South Office ");
    await user.click(within(dialog).getByRole("button", { name: "Add location" }));
    expect(await screen.findByText("Location South Office added")).toBeTruthy();
    expect(add).toHaveBeenCalledWith({
      input: { clientId: CLIENT.id, code: "south", name: "South Office" },
    });
  });

  it("adds a destination for this client without asking for the client", async () => {
    const create = vi.fn(() => ({ ...FAX, destinationId: "new", label: "Front desk" }));
    const { user } = renderPage({ create_delivery_destination: create });
    await screen.findByRole("heading", { level: 1, name: /^syn-clinic/ });
    await user.click(
      section("Delivery destinations").getByRole("button", { name: "Add destination" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Add destination for Synthetic Clinic",
    });
    expect(within(dialog).queryByRole("combobox", { name: /^Client/ })).toBeNull();
    await chooseWithKeyboard(user, /Channel$/, "Pickup at the laboratory");
    await typeInto(user, /^Label/, "Front desk");
    await user.click(within(dialog).getByRole("button", { name: "Add destination" }));
    expect(await screen.findByText("Destination Front desk added")).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: { clientId: CLIENT.id, channel: "pickup", label: "Front desk" },
    });
  });

  it("says so when the client has no locations or destinations yet", async () => {
    renderPage({
      list_clients: () => [{ ...CLIENT, locations: [] }],
      list_delivery_destinations: () => [OLD_FAX],
    });
    expect(await screen.findByText("No locations yet.")).toBeTruthy();
    expect(await screen.findByText("No delivery destinations yet")).toBeTruthy();
  });

  it("keeps the header when the client is unknown or cannot be loaded", async () => {
    renderPage({}, "00000000-0000-7000-8000-000000000699");
    expect(await screen.findByText("Client not found")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Client" })).toBeTruthy();
  });

  it("shows the engine's error under a generic header", async () => {
    renderPage({
      list_clients: () => Promise.reject(engineError("internal", "Clients are unavailable.")),
    });
    expect(screen.getByRole("heading", { level: 1, name: "Client" })).toBeTruthy();
    expect(await screen.findByText("Clients are unavailable.")).toBeTruthy();
  });

  it("shows a failed destinations list in its own section, with the rest working", async () => {
    renderPage({
      list_delivery_destinations: () =>
        Promise.reject(engineError("internal", "Destinations are unavailable.")),
    });
    expect(await screen.findByText("Destinations are unavailable.")).toBeTruthy();
    expect(section("Locations").getByRole("gridcell", { name: "North Office" })).toBeTruthy();
  });
  it("corrects and retires a client with its current revision and a required reason", async () => {
    const save = vi.fn(() => ({ ...CLIENT, active: false, revision: 2 }));
    const { user } = renderPage({ update_client: save });
    await user.click(await screen.findByRole("button", { name: "Correct client" }));
    const dialog = await screen.findByRole("dialog", { name: "Correct client" });
    await chooseWithKeyboard(user, /Availability for new orders$/, "Retired");
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Reason/ }),
      "Synthetic closure evidence",
    );
    await user.click(within(dialog).getByRole("button", { name: "Save correction" }));
    expect(await screen.findByText("Directory correction saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        clientId: CLIENT.id,
        expectedRevision: 1,
        code: CLIENT.code,
        name: CLIENT.name,
        active: false,
        reason: "Synthetic closure evidence",
      },
    });
  });
  it("loads a legacy history only when requested and identifies unknown attribution", async () => {
    const history = vi.fn(() => [
      {
        revision: 1,
        snapshot: { kind: "client", code: CLIENT.code, name: CLIENT.name, active: true },
        recordedAt: "2020-01-01T10:00:00Z",
        recordedBy: null,
        reason: null,
        origin: "legacy_baseline",
      },
    ]);
    const { user } = renderPage({ directory_history: history });
    await screen.findByRole("button", { name: "Show correction history" });
    expect(history).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Show correction history" }));
    expect(await screen.findByText("Unknown — legacy baseline")).toBeTruthy();
    expect(history).toHaveBeenCalledWith({ input: { record: { kind: "client", id: CLIENT.id } } });
  });
});
