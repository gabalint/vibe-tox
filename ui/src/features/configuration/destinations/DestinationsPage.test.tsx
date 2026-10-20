import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClientView } from "@/generated/ClientView";
import { useCommandQuery } from "@/lib/commands";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import type { DestinationView } from "./contract";
import { DestinationsPage } from "./DestinationsPage";

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

const OTHER_CLIENT: ClientView = {
  active: true,
  revision: 1,
  id: "00000000-0000-7000-8000-000000000611",
  code: "syn-other",
  name: "Synthetic Other Practice",
  locations: [],
};

const UNVERIFIED: DestinationView = {
  destinationId: "00000000-0000-7000-8000-000000000c01",
  clientId: CLIENT.id,
  clientLocationId: "00000000-0000-7000-8000-000000000602",
  channel: "fax",
  address: "312-555-0100",
  label: "North fax",
  verified: null,
  state: "unverified",
  retired: false,
  retireReason: null,
  replacesId: null,
  createdBy: { userId: "00000000-0000-7000-8000-000000000e08", displayName: "Test Configurer" },
  revision: 1,
  allowedActions: [
    "verify_delivery_destination",
    "replace_delivery_destination",
    "retire_delivery_destination",
  ],
};

const VERIFIED: DestinationView = {
  ...UNVERIFIED,
  destinationId: "00000000-0000-7000-8000-000000000c02",
  clientLocationId: null,
  channel: "pickup",
  address: null,
  label: "Front desk pickup",
  verified: {
    by: { userId: "00000000-0000-7000-8000-000000000e09", displayName: "Test Verifier" },
    at: "2026-10-12T14:03:00Z",
    method: "Seen in person",
    byCreator: false,
  },
  state: "verified",
  revision: 2,
  allowedActions: ["replace_delivery_destination", "retire_delivery_destination"],
};

const RETIRED: DestinationView = {
  ...UNVERIFIED,
  destinationId: "00000000-0000-7000-8000-000000000c03",
  clientLocationId: null,
  channel: "email",
  address: "old@clinic.example",
  label: "Old email",
  state: "retired",
  retired: true,
  allowedActions: [],
};

/** The engine's answer to a stale `expectedRevision`. */
const CHANGED = "This record changed since it was shown. Reload and try again.";
const STALE_REVISION = { code: "revision_conflict", message: CHANGED };

function renderPage(path: string, commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_clients: () => [CLIENT],
    list_delivery_destinations: () => [UNVERIFIED, VERIFIED, RETIRED],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/destinations", element: <DestinationsPage /> }],
    path,
  );
}

/** Opens the destination's panel from its row and returns the panel. */
async function openDestination(user: ReturnType<typeof renderPage>["user"], label: string) {
  await openRow(user, label);
  return screen.findByRole("region", { name: label });
}

describe("DestinationsPage (REQ-RPT-012)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists destinations that are not retired, with client, channel, address and state", async () => {
    const list = vi.fn(() => [UNVERIFIED, VERIFIED, RETIRED]);
    const { container } = renderPage("/configuration/destinations", {
      list_delivery_destinations: list,
    });
    expect(await screen.findByRole("gridcell", { name: "North fax" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: {} });
    expect(screen.getByRole("gridcell", { name: "Synthetic Clinic, North Office" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Synthetic Clinic" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Pickup at the laboratory" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Not verified" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "Old email" })).toBeNull();
    expect(screen.getByRole("gridcell", { name: "Verified" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", {
        name: "Test Verifier, 2026-10-12 14:03 UTC: Seen in person",
      }),
    ).toBeTruthy();
    expect(screen.getByText("2 destinations · retired excluded")).toBeTruthy();
    expect(screen.queryByText(/Select a destination/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("explains a retired-only scope and includes it through More filters", async () => {
    const { user } = renderPage("/configuration/destinations", {
      list_delivery_destinations: () => [RETIRED],
    });
    expect(await screen.findByText("No destinations in this scope")).toBeTruthy();
    expect(screen.getByText("0 destinations · retired excluded")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Clear filters" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.click(screen.getByRole("checkbox", { name: "Show retired" }));
    expect(await screen.findByRole("gridcell", { name: "Old email" })).toBeTruthy();
    expect(screen.getByText("1 destination")).toBeTruthy();
  });

  it("shows a destination a delivery attempt found wrong as the engine states it", async () => {
    const wrong: DestinationView = {
      ...VERIFIED,
      label: "Wrong fax",
      state: "reported_wrong",
    };
    renderPage("/configuration/destinations", {
      list_delivery_destinations: () => [wrong],
    });
    expect(await screen.findByRole("gridcell", { name: "Wrong fax" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Reported wrong" })).toBeTruthy();
  });

  it("filters by state, client and retired in the address, and clears the filters", async () => {
    const other: DestinationView = {
      ...VERIFIED,
      destinationId: "00000000-0000-7000-8000-000000000c04",
      clientId: OTHER_CLIENT.id,
      label: "Other pickup",
    };
    const { user, router } = renderPage("/configuration/destinations", {
      list_clients: () => [CLIENT, OTHER_CLIENT],
      list_delivery_destinations: () => [UNVERIFIED, VERIFIED, RETIRED, other],
    });
    await screen.findByRole("gridcell", { name: "North fax" });
    await chooseWithKeyboard(user, /State$/, "Not verified");
    expect(router.state.location.search).toBe("?state=unverified");
    expect(await screen.findByText("1 of 3 destinations · retired excluded")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await user.type(screen.getByRole("combobox", { name: /^Client/ }), "other");
    await user.click(await screen.findByRole("option", { name: /Other Practice/ }));
    expect(router.state.location.search).toBe(`?client=${OTHER_CLIENT.id}`);
    expect(await screen.findByText("1 of 3 destinations · retired excluded")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Other pickup" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.click(screen.getByRole("checkbox", { name: "Show retired" }));
    expect(router.state.location.search).toBe("?retired=show");
    expect(await screen.findByRole("gridcell", { name: "Old email" })).toBeTruthy();
    expect(screen.getByText("4 destinations")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "More filters" }));
    expect(screen.queryByRole("checkbox", { name: "Show retired" })).toBeNull();
    expect(screen.getByText("Retired destinations included")).toBeTruthy();
  });

  it("adds a fax destination for a client location, requiring its address", async () => {
    const create = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "address", code: "format", message: "Enter a fax number of 7 to 20 digits." },
          ],
        }),
      )
      .mockReturnValue({ ...UNVERIFIED, destinationId: "new" });
    const { user, router } = renderPage("/configuration/destinations", {
      create_delivery_destination: create,
    });
    await user.click(await screen.findByRole("button", { name: "Add destination" }));
    const dialog = await screen.findByRole("dialog", { name: "Add destination" });
    await user.type(within(dialog).getByRole("combobox", { name: /^Client/ }), "syn-cl");
    await user.click(await screen.findByRole("option", { name: /Synthetic Clinic/ }));
    await user.click(within(dialog).getByRole("combobox", { name: /^Location/ }));
    await user.keyboard("{ArrowDown}");
    await user.click(await screen.findByRole("option", { name: /North Office/ }));
    await chooseWithKeyboard(user, /Channel$/, "Fax");
    await typeInto(user, /^Label/, "North fax 2");
    await user.click(within(dialog).getByRole("button", { name: "Add destination" }));
    expect(await within(dialog).findByText("Enter address.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();
    await typeInto(user, /^Address/, " 312-555-0199 ");
    await user.click(within(dialog).getByRole("button", { name: "Add destination" }));
    const address = within(dialog).getByLabelText(/^Address/);
    await within(dialog).findByText("Enter a fax number of 7 to 20 digits.");
    expect(address.getAttribute("aria-invalid")).toBe("true");
    await user.click(within(dialog).getByRole("button", { name: "Add destination" }));
    expect(await screen.findByText("Destination North fax 2 added")).toBeTruthy();
    expect(router.state.location.search).toBe("?open=new");
    expect(create).toHaveBeenCalledWith({
      input: {
        clientId: CLIENT.id,
        clientLocationId: "00000000-0000-7000-8000-000000000602",
        channel: "fax",
        address: "312-555-0199",
        label: "North fax 2",
      },
    });
  });

  it("verifies the selected destination, and shows the engine's refusal of its creator", async () => {
    const verify = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("gate_blocked", "This action is blocked by one or more gates.", {
          gateReasons: [
            {
              code: "second_person_required",
              message: "A person other than its creator must verify a destination.",
            },
          ],
        }),
      )
      .mockReturnValue({ ...UNVERIFIED, verified: VERIFIED.verified });
    const { user } = renderPage("/configuration/destinations", {
      verify_delivery_destination: verify,
    });
    const panel = await openDestination(user, "North fax");
    await user.click(within(panel).getByRole("button", { name: "Verify" }));
    const dialog = await screen.findByRole("dialog", { name: "Verify North fax" });
    await typeInto(user, /^How it was verified/, "Test fax confirmed by phone");
    await user.click(within(dialog).getByRole("button", { name: "Verify destination" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain(
      "A person other than its creator must verify a destination.",
    );
    expect(verify).toHaveBeenCalledWith({
      input: {
        destinationId: UNVERIFIED.destinationId,
        expectedRevision: 1,
        method: "Test fax confirmed by phone",
        selfApproval: false,
      },
    });
    await user.click(within(dialog).getByRole("button", { name: "Verify destination" }));
    expect(await screen.findByText("North fax verified")).toBeTruthy();
  });

  it("asks the creator to confirm a self-verification, then shows who verified it", async () => {
    const message =
      "You created this destination. Your laboratory allows you to verify it yourself: confirm that no second person checked it.";
    const selfVerified = {
      ...UNVERIFIED,
      state: "verified",
      allowedActions: [],
      verified: {
        by: UNVERIFIED.createdBy,
        at: "2026-10-12T14:03:00Z",
        method: "Test fax confirmed by phone",
        byCreator: true,
      },
    } satisfies DestinationView;
    const verify = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
        }),
      )
      .mockReturnValue(selfVerified);
    const list = vi.fn().mockReturnValueOnce([UNVERIFIED]).mockReturnValue([selfVerified]);
    const { user } = renderPage("/configuration/destinations", {
      verify_delivery_destination: verify,
      list_delivery_destinations: list,
    });
    const panel = await openDestination(user, "North fax");
    await user.click(within(panel).getByRole("button", { name: "Verify" }));
    const dialog = await screen.findByRole("dialog", { name: "Verify North fax" });
    await typeInto(user, /^How it was verified/, "Test fax confirmed by phone");
    await user.click(within(dialog).getByRole("button", { name: "Verify destination" }));
    const confirm = await within(dialog).findByRole("checkbox", { name: /no second person/ });
    expect(within(dialog).getByText(message)).toBeTruthy();
    expect(within(dialog).queryByText("Not recorded")).toBeNull();
    await user.click(confirm);
    await user.click(within(dialog).getByRole("button", { name: "Verify destination" }));
    expect(await screen.findByText("North fax verified")).toBeTruthy();
    expect(verify).toHaveBeenLastCalledWith({
      input: expect.objectContaining({ selfApproval: true }),
    });
    expect(
      await within(panel).findByText(/Test Configurer \(verified by its creator\)/),
    ).toBeTruthy();
  });

  it("offers only the actions the engine allows, in the panel and the row menu", async () => {
    const { user } = renderPage("/configuration/destinations?retired=show");
    const panel = within(await openDestination(user, "Front desk pickup"));
    expect(panel.queryByRole("button", { name: "Verify" })).toBeNull();
    expect(panel.getByRole("button", { name: "Replace" }).getAttribute("data-variant")).toBe(
      "primary",
    );
    expect(panel.getByRole("button", { name: "Retire" }).getAttribute("data-variant")).toBe(
      "danger",
    );
    const menu = within(await openRowActions(user, "North fax"));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Verify",
      "Replace",
      "Retire",
    ]);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("button", { name: "Actions for Old email" })).toBeNull();
    const retired = within(await openDestination(user, "Old email"));
    expect(retired.queryByRole("button", { name: "Replace" })).toBeNull();
    expect(retired.queryByRole("button", { name: "Retire" })).toBeNull();
  });

  it("opens with Enter, links the client, and Escape closes with focus back on the row", async () => {
    const { user, router } = renderPage("/configuration/destinations");
    await screen.findByRole("gridcell", { name: "North fax" });
    await openRowWithEnter(user, "North fax");
    const panel = within(await screen.findByRole("region", { name: "North fax" }));
    expect(router.state.location.search).toBe(`?open=${UNVERIFIED.destinationId}`);
    expect(panel.getByRole("link", { name: "Synthetic Clinic" }).getAttribute("href")).toBe(
      `/configuration/clients/${CLIENT.id}`,
    );
    expect(panel.getByText("North Office")).toBeTruthy();
    await user.click(panel.getByRole("heading", { name: "North fax" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "North fax" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "North fax" })),
    );
  });

  it("opens the exact destination a work item names, even a retired one", async () => {
    const replacement: DestinationView = {
      ...UNVERIFIED,
      destinationId: "00000000-0000-7000-8000-000000000c05",
      label: "North fax new",
      replacesId: RETIRED.destinationId,
    };
    const { user } = renderPage(`/configuration/destinations?open=${RETIRED.destinationId}`, {
      list_delivery_destinations: () => [UNVERIFIED, RETIRED, replacement],
    });
    expect(await screen.findByRole("region", { name: "Old email" })).toBeTruthy();
    await openRow(user, "North fax new");
    const panel = within(await screen.findByRole("region", { name: "North fax new" }));
    await user.click(panel.getByRole("link", { name: "Old email" }));
    expect(await screen.findByRole("region", { name: "Old email" })).toBeTruthy();
  });

  it("replaces the selected destination with a reason, and shows a refusal", async () => {
    const replace = vi
      .fn()
      .mockRejectedValueOnce(engineError("conflict", CHANGED, { gateReasons: [STALE_REVISION] }))
      .mockReturnValue({ ...UNVERIFIED, destinationId: "replacement" });
    const { user } = renderPage("/configuration/destinations", {
      replace_delivery_destination: replace,
    });
    const panel = await openDestination(user, "North fax");
    await user.click(within(panel).getByRole("button", { name: "Replace" }));
    const dialog = await screen.findByRole("dialog", { name: "Replace North fax" });
    await typeInto(user, /^Address/, "312-555-0111");
    await typeInto(user, /^Label/, "North fax new");
    await typeInto(user, /^Reason/, "Clinic changed its fax number");
    await user.click(within(dialog).getByRole("button", { name: "Replace destination" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("Changed since it was shown");
    expect(replace).toHaveBeenCalledWith({
      input: {
        destinationId: UNVERIFIED.destinationId,
        expectedRevision: 1,
        address: "312-555-0111",
        label: "North fax new",
        reason: "Clinic changed its fax number",
      },
    });
    await user.click(within(dialog).getByRole("button", { name: "Replace destination" }));
    expect(await screen.findByText("North fax replaced")).toBeTruthy();
  });

  it("retires the selected destination with a reason, and shows a refusal", async () => {
    const retire = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("forbidden", "You do not have permission for this action."),
      )
      .mockReturnValue({ ...UNVERIFIED, retired: true });
    const { user } = renderPage("/configuration/destinations", {
      retire_delivery_destination: retire,
    });
    const panel = await openDestination(user, "North fax");
    await user.click(within(panel).getByRole("button", { name: "Retire" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Retire North fax" });
    await typeInto(user, /^Reason/, "Clinic closed");
    await user.click(within(dialog).getByRole("button", { name: "Retire destination" }));
    expect(
      await within(dialog).findByText("You do not have permission for this action."),
    ).toBeTruthy();
    expect(retire).toHaveBeenCalledWith({
      input: {
        destinationId: UNVERIFIED.destinationId,
        expectedRevision: 1,
        reason: "Clinic closed",
      },
    });
    await user.click(within(dialog).getByRole("button", { name: "Retire destination" }));
    expect(await screen.findByText("North fax retired")).toBeTruthy();
  });

  it("refreshes the work queue counts after a destination changes", async () => {
    const queues = vi.fn(() => []);
    mockCommands({
      current_session: () => sessionWith(MANAGER),
      list_clients: () => [CLIENT],
      list_delivery_destinations: () => [UNVERIFIED],
      list_work_queues: queues,
      retire_delivery_destination: () => ({ ...UNVERIFIED, retired: true }),
    });
    function WithQueueCounts() {
      useCommandQuery("list_work_queues");
      return <DestinationsPage />;
    }
    const { user } = renderRoutes(
      [{ path: "/configuration/destinations", element: <WithQueueCounts /> }],
      "/configuration/destinations",
    );
    const panel = await openDestination(user, "North fax");
    await waitFor(() => expect(queues).toHaveBeenCalledOnce());
    await user.click(within(panel).getByRole("button", { name: "Retire" }));
    await typeInto(user, /^Reason/, "Clinic closed");
    await user.click(
      within(await screen.findByRole("alertdialog", { name: "Retire North fax" })).getByRole(
        "button",
        { name: "Retire destination" },
      ),
    );
    await waitFor(() => expect(queues).toHaveBeenCalledTimes(2));
  });

  it("offers Add destination only with manage_configuration", async () => {
    renderPage("/configuration/destinations", { current_session: () => sessionWith([]) });
    await screen.findByRole("gridcell", { name: "North fax" });
    expect(screen.queryByRole("button", { name: "Add destination" })).toBeNull();
  });

  it("tells nothing yet from a filter that excludes everything", async () => {
    const { user, router } = renderPage("/configuration/destinations?state=reported_wrong");
    expect(await screen.findByText("No destinations match the filters")).toBeTruthy();
    const clear = screen.getAllByRole("button", { name: "Clear filters" }).at(-1);
    if (!clear) throw new Error("no Clear filters");
    await user.click(clear);
    expect(await screen.findByRole("gridcell", { name: "North fax" })).toBeTruthy();
    expect(router.state.location.search).toBe("");
  });

  it("shows the destinations when the client names cannot be loaded, and says so", async () => {
    renderPage("/configuration/destinations", {
      list_clients: () => Promise.reject(engineError("internal", "Clients are unavailable.")),
    });
    expect(await screen.findByRole("gridcell", { name: "North fax" })).toBeTruthy();
    expect(await screen.findByText("The client names could not be loaded")).toBeTruthy();
  });

  it("shows the empty state with the first action", async () => {
    renderPage("/configuration/destinations", { list_delivery_destinations: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "Delivery destinations" })).toBeTruthy();
    expect(screen.getByText("Loading delivery destinations…")).toBeTruthy();
    expect(await screen.findByText("No delivery destinations yet")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Add destination" })).toHaveLength(2);
  });

  it("shows the engine's error", async () => {
    renderPage("/configuration/destinations", {
      list_delivery_destinations: () =>
        Promise.reject(engineError("internal", "Destinations are unavailable.")),
    });
    expect(await screen.findByText("Destinations are unavailable.")).toBeTruthy();
  });
});
