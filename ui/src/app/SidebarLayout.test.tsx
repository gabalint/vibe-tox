import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { tabTo } from "@/lib/test/keyboard";
import { queue, renderApp } from "./frameTestData";

const LAYOUT_KEY = "vibe-tox.navigation.layout-v1";

function navigationWidth() {
  return screen.getByRole("separator", { name: "Navigation width" });
}

describe("Sidebar layout", () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  // NFR-09: resizing and collapse stay reachable without pointer dragging.
  it("names its controls and exposes the navigation pane and its current width", async () => {
    renderApp("/", ["enter_orders"]);
    const collapse = await screen.findByRole("button", { name: "Collapse navigation" });
    const sidebar = screen.getByRole("complementary", { name: "Navigation sidebar" });
    const nav = within(sidebar).getByRole("navigation", { name: "Main" });
    expect(nav.id).toBe("main-navigation");
    expect(sidebar.contains(collapse)).toBe(true);
    expect(nav.contains(collapse)).toBe(false);
    expect(collapse.getAttribute("aria-expanded")).toBe("true");
    expect(collapse.getAttribute("aria-controls")).toBe(nav.id);
    expect(navigationWidth().getAttribute("aria-controls")).toBe(nav.id);
    expect(navigationWidth().getAttribute("aria-orientation")).toBe("vertical");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("256");
    expect(navigationWidth().getAttribute("aria-valuemin")).toBe("56");
    expect(navigationWidth().getAttribute("aria-valuemax")).toBe("360");
    expect(navigationWidth().getAttribute("aria-valuetext")).toBe("256 pixels");
  });

  it("remembers the chosen width and collapsed state across app mounts", async () => {
    const first = renderApp("/", ["enter_orders"]);
    await screen.findByRole("button", { name: "Collapse navigation" });
    await tabTo(first.user, navigationWidth());
    await first.user.keyboard("{ArrowRight}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("272");
    await first.user.click(screen.getByRole("button", { name: "Collapse navigation" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    expect(navigationWidth().getAttribute("aria-valuetext")).toBe("Collapsed");
    expect(localStorage.getItem(LAYOUT_KEY)).toBe(JSON.stringify({ width: 272, collapsed: true }));
    first.unmount();

    const second = renderApp("/", ["enter_orders"]);
    const expand = await screen.findByRole("button", { name: "Expand navigation" });
    expect(expand.getAttribute("aria-expanded")).toBe("false");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    await second.user.click(expand);
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("272");
    expect(localStorage.getItem(LAYOUT_KEY)).toBe(JSON.stringify({ width: 272, collapsed: false }));
  });

  it("supports keyboard bounds and restores the width after an Enter collapse", async () => {
    const { user } = renderApp("/", []);
    await screen.findByRole("button", { name: "Collapse navigation" });
    await tabTo(user, navigationWidth());
    await user.keyboard("{ArrowLeft}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("240");
    await user.keyboard("{Home}{ArrowLeft}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("224");
    await user.keyboard("{End}{ArrowRight}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("360");
    await user.keyboard("{Enter}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    expect(screen.getByRole("button", { name: "Expand navigation" })).toBeTruthy();
    await user.keyboard("{ArrowLeft}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    expect(document.activeElement).toBe(navigationWidth());
    await user.keyboard("{ArrowRight}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("360");
    await user.keyboard("{Enter}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    await user.keyboard("{Enter}");
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("360");
    expect(document.activeElement).toBe(navigationWidth());
  });

  it("offers narrower, wider and reset actions without dragging", async () => {
    const { user } = renderApp("/", []);
    const options = await screen.findByRole("button", { name: "Navigation width options" });
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Wider navigation" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("272");
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Narrower navigation" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("256");
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Narrower navigation" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("240");
    await user.click(options);
    await user.click(await screen.findByRole("menuitem", { name: "Reset navigation width" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("256");
  });

  it("preserves open work-area choices when the whole sidebar collapses and expands", async () => {
    const { user } = renderApp("/", ["enter_orders", "manage_configuration"]);
    await user.click(await screen.findByRole("button", { name: "Front desk" }));
    await user.click(screen.getByRole("button", { name: "Laboratory setup" }));
    await user.click(screen.getByRole("button", { name: "Collapse navigation" }));
    expect(screen.queryByRole("list", { name: "Front desk" })).toBeNull();
    expect(screen.queryByRole("list", { name: "Laboratory setup" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Expand navigation" }));
    expect(screen.getByRole("button", { name: "Front desk" }).getAttribute("aria-expanded")).toBe(
      "true",
    );
    expect(
      screen.getByRole("button", { name: "Laboratory setup" }).getAttribute("aria-expanded"),
    ).toBe("true");
    expect(screen.getByRole("list", { name: "Laboratory setup" })).toBeTruthy();
  });

  // NFR-09 and REQ-OPS-004: compact navigation retains ordinary links and engine counts.
  it("opens compact work areas by keyboard, restores focus and closes on navigation", async () => {
    const { user, router, container } = renderApp("/", ["enter_orders"], {
      list_orders: () => [],
      list_work_queues: () => [
        queue({ queue: "orders_needing_decision", ownerCapability: "enter_orders", count: 3 }),
      ],
    });
    await user.click(await screen.findByRole("button", { name: "Collapse navigation" }));
    const frontDesk = screen.getByRole("button", { name: "Front desk, 3 waiting" });
    await tabTo(user, frontDesk);
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog", { name: "Front desk" });
    expect(within(dialog).getByRole("link", { name: "Patients" })).toBeTruthy();
    expect(await within(dialog).findByRole("link", { name: "Orders, 3 waiting" })).toBeTruthy();
    await expectNoA11yViolations(dialog);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Front desk" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(frontDesk));
    await expectNoA11yViolations(container, { wholePage: true });

    await user.keyboard("{Enter}");
    const reopened = await screen.findByRole("dialog", { name: "Front desk" });
    await user.click(within(reopened).getByRole("link", { name: "Orders, 3 waiting" }));
    await screen.findByRole("heading", { level: 1, name: "Orders" });
    expect(router.state.location.pathname).toBe("/orders");
    expect(screen.queryByRole("dialog", { name: "Front desk" })).toBeNull();

    await user.click(frontDesk);
    const currentArea = await screen.findByRole("dialog", { name: "Front desk" });
    const currentOrder = within(currentArea).getByRole("link", { name: "Orders, 3 waiting" });
    expect(currentOrder.getAttribute("aria-current")).toBe("page");
    await user.click(currentOrder);
    expect(screen.queryByRole("dialog", { name: "Front desk" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(frontDesk));

    await user.click(screen.getByRole("button", { name: "Expand navigation" }));
    const orders = within(screen.getByRole("navigation", { name: "Main" })).getByRole("link", {
      name: "Orders, 3 waiting",
    });
    expect(orders.getAttribute("aria-current")).toBe("page");
  });

  it.each(["{broken", "null", "[]", '{"width":"wide","collapsed":"yes"}'])(
    "uses the default layout when saved preferences are invalid: %s",
    async (stored) => {
      localStorage.setItem(LAYOUT_KEY, stored);
      renderApp("/", []);
      await screen.findByRole("button", { name: "Collapse navigation" });
      expect(navigationWidth().getAttribute("aria-valuenow")).toBe("256");
    },
  );

  it("keeps navigation usable when the computer cannot read or save UI preferences", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("Storage unavailable", "SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("Storage full", "QuotaExceededError");
    });
    const { user } = renderApp("/", ["enter_orders"]);
    await user.click(await screen.findByRole("button", { name: "Collapse navigation" }));
    expect(navigationWidth().getAttribute("aria-valuenow")).toBe("56");
    await user.click(screen.getByRole("button", { name: "Expand navigation" }));
    await user.click(screen.getByRole("button", { name: "Front desk" }));
    expect(screen.getByRole("link", { name: "Orders" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Front desk" }));
    expect(screen.queryByRole("list", { name: "Front desk" })).toBeNull();
  });
});
