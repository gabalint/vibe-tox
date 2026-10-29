import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { detailScreens } from "./detailScreens";
import { queue, renderApp } from "./frameTestData";
import { navigation } from "./navigation";

const ADMINISTRATOR = [
  "accession",
  "approve_configuration",
  "backup_restore",
  "billing",
  "deliver_reports",
  "enter_orders",
  "export",
  "manage_configuration",
  "manage_quality",
  "manage_users",
  "perform_analysis",
  "release_reports",
  "review_results",
  "view_audit",
];

function linksIn(name: string) {
  return within(screen.getByRole("list", { name }))
    .getAllByRole("link")
    .map((link) => link.textContent);
}

/** Opens a work area; every area starts closed. */
async function open(user: UserEvent, name: string) {
  await user.click(await screen.findByRole("button", { name }));
}

function currentLinks() {
  return within(screen.getByRole("navigation", { name: "Main" }))
    .getAllByRole("link")
    .filter((link) => link.hasAttribute("aria-current"))
    .map((link) => [link.textContent, link.getAttribute("aria-current")]);
}

describe("Side navigation", () => {
  beforeEach(() => localStorage.clear());

  it("makes financial follow-up available to billing staff without clinical front desk access", async () => {
    const { user } = renderApp("/", ["billing"]);
    await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await open(user, "Billing");
    expect(linksIn("Billing")).toEqual([
      "Billing follow-up",
      "Billing work",
      "Billing class mappings",
    ]);
    expect(screen.queryByRole("list", { name: "Front desk" })).toBeNull();
  });

  it("lets quality staff reach identity and material without order-entry navigation", async () => {
    const { user } = renderApp("/", ["manage_quality"]);
    await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await open(user, "Front desk");
    expect(linksIn("Front desk")).toEqual(["Patients", "Storage excursions", "Container lookup"]);
    expect(screen.queryByRole("link", { name: "Orders" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Incomplete identity intake" })).toBeNull();
  });

  // Presentation only: the engine refuses the commands of screens a session cannot open.
  it("shows each work area only to sessions holding one of its capabilities", async () => {
    const { user } = renderApp("/", ["view_audit"]);
    await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await open(user, "Quality and audit");
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(
      within(nav)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Quality and audit"]);
    expect(linksIn("Quality and audit")).toEqual(["Audit trail", "Audit anchors"]);
    expect(within(nav).queryByRole("link", { name: "Users" })).toBeNull();
    expect(within(nav).queryByRole("link", { name: "Host health" })).toBeNull();
  });

  it("orders an administrator's work areas by the laboratory's flow, all closed at first", async () => {
    const { user, container } = renderApp("/", ADMINISTRATOR);
    await screen.findByRole("heading", { level: 1, name: "Work queues" });
    const groups = within(screen.getByRole("navigation", { name: "Main" })).getAllByRole("button");
    expect(groups.map((group) => [group.textContent, group.getAttribute("aria-expanded")])).toEqual(
      [
        ["Front desk", "false"],
        ["Bench", "false"],
        ["Review", "false"],
        ["Reports", "false"],
        ["Billing", "false"],
        ["Quality and audit", "false"],
        ["Test catalog", "false"],
        ["Laboratory setup", "false"],
        ["System", "false"],
      ],
    );
    for (const area of ["Front desk", "Bench", "Review", "Reports", "Billing"]) {
      await open(user, area);
    }
    expect(linksIn("Front desk")).toEqual([
      "Patients",
      "Orders",
      "Incomplete identity intake",
      "Storage excursions",
      "Container lookup",
    ]);
    expect(linksIn("Bench")).toEqual(["Worklist", "Runs", "Bench setup"]);
    expect(linksIn("Review")).toEqual(["Result review", "Result imports"]);
    expect(linksIn("Reports")).toEqual([
      "Release queue",
      "Delivery",
      "Reports",
      "Report templates",
    ]);
    expect(linksIn("Billing")).toEqual([
      "Billing follow-up",
      "Billing work",
      "Billing class mappings",
    ]);
    expect(screen.queryByRole("list", { name: "System" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("gives configuration managers the test catalog, setup and system areas", async () => {
    const { user } = renderApp("/", ["manage_configuration"]);
    const catalog = await screen.findByRole("button", { name: "Test catalog" });
    expect(catalog.getAttribute("aria-expanded")).toBe("false");
    await user.click(catalog);
    expect(linksIn("Test catalog")).toEqual([
      "Tests and panels",
      "Methods",
      "Analytes",
      "Specimen types",
    ]);
    await user.click(screen.getByRole("button", { name: "Laboratory setup" }));
    expect(linksIn("Laboratory setup")).toEqual([
      "Material configuration",
      "Laboratory profile",
      "Catalog review",
      "Clients",
      "Practitioners",
      "Delivery destinations",
      "Instruments",
      "Starter data",
    ]);
    await user.click(screen.getByRole("button", { name: "System" }));
    expect(linksIn("System")).toEqual([
      "Host health",
      "Versions and updates",
      "Operations settings",
    ]);
    await open(user, "Bench");
    expect(linksIn("Bench")).toEqual(["Bench setup"]);
  });

  it("gives approvers the test catalog, Catalog review and Starter data", async () => {
    const { user } = renderApp("/", ["approve_configuration"]);
    await user.click(await screen.findByRole("button", { name: "Test catalog" }));
    expect(linksIn("Test catalog")).toEqual([
      "Tests and panels",
      "Methods",
      "Analytes",
      "Specimen types",
    ]);
    await user.click(screen.getByRole("button", { name: "Laboratory setup" }));
    expect(linksIn("Laboratory setup")).toEqual(["Catalog review", "Starter data"]);
  });

  it("opens Catalog review directly for an approver without loaded starter data", async () => {
    renderApp("/configuration/catalog-review", ["approve_configuration"], {
      standard_catalog_status: () => ({ loads: [], draftsRemaining: 0 }),
      list_catalog_previews: () => [],
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Catalog review" })).toBeTruthy();
    expect(currentLinks()).toEqual([["Catalog review", "page"]]);
    expect(screen.getByRole("button", { name: "Generate Catalog Preview" })).toBeTruthy();
  });

  it("sends the old catalog address to Tests and panels, its area opened", async () => {
    const { router } = renderApp("/configuration/catalog", ["manage_configuration"], {
      list_orderables: () => [],
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Tests and panels" })).toBeTruthy();
    expect(router.state.location.pathname).toBe("/configuration/orderables");
    expect(currentLinks()).toEqual([["Tests and panels", "page"]]);
  });

  it("remembers a closed or opened work area on this computer", async () => {
    const first = renderApp("/", ["perform_analysis", "backup_restore"]);
    await open(first.user, "Bench");
    await open(first.user, "System");
    await open(first.user, "System");
    expect(linksIn("Bench")).toEqual(["Worklist", "Runs", "Bench setup"]);
    expect(screen.queryByRole("list", { name: "System" })).toBeNull();
    first.unmount();
    renderApp("/", ["perform_analysis", "backup_restore"]);
    const system = await screen.findByRole("button", { name: "System" });
    expect(system.getAttribute("aria-expanded")).toBe("false");
    expect(linksIn("Bench")).toEqual(["Worklist", "Runs", "Bench setup"]);
  });

  it("opens the current screen's work area and marks only that entry current", async () => {
    localStorage.setItem(
      "vibe-tox.navigation.open-sections",
      JSON.stringify({ "Laboratory setup": false }),
    );
    renderApp("/users", ["manage_users"]);
    await screen.findByRole("heading", { level: 1, name: "Users" });
    expect(
      screen.getByRole("button", { name: "Laboratory setup" }).getAttribute("aria-expanded"),
    ).toBe("true");
    expect(currentLinks()).toEqual([["Users", "page"]]);
  });

  // Finding 11: `/reports/release` used to mark both "Release queue" and "Reports".
  it("marks one entry on nested paths, and the parent screen on a detail screen", async () => {
    const release = renderApp("/reports/release", ["release_reports", "deliver_reports"], {
      list_release_queue: () => [],
    });
    await screen.findByRole("heading", { level: 1, name: "Release queue" });
    expect(currentLinks()).toEqual([["Release queue", "page"]]);
    release.unmount();
    renderApp("/audit/anchors", ["view_audit"], {
      get_anchor_status: () => new Promise(() => {}),
      list_anchor_exports: () => new Promise(() => {}),
    });
    await screen.findByRole("heading", { level: 1, name: "Audit anchors" });
    expect(currentLinks()).toEqual([["Audit anchors", "page"]]);
  });

  it("marks the list a detail screen belongs to", async () => {
    renderApp("/bench/runs/00000000-0000-7000-8000-0000000000b1", ["perform_analysis"], {
      get_run: () => new Promise(() => {}),
    });
    const nav = await screen.findByRole("navigation", { name: "Main" });
    await within(nav).findByRole("link", { name: "Runs" });
    await waitFor(() => expect(currentLinks()).toEqual([["Runs", "true"]]));
  });

  it("keeps a tabbed screen's entry current on each tab", async () => {
    const backups = renderApp("/operations/backups/runs", ["backup_restore"], {
      list_backup_targets: () => new Promise(() => {}),
      list_backup_runs: () => new Promise(() => {}),
      list_restore_tests: () => new Promise(() => {}),
      get_backup_status: () => new Promise(() => {}),
    });
    await within(await screen.findByRole("navigation", { name: "Main" })).findByRole("link", {
      name: "Backups",
    });
    await waitFor(() => expect(currentLinks()).toEqual([["Backups", "true"]]));
    backups.unmount();
    renderApp("/bench/setup/lots", ["perform_analysis"], {
      list_instruments: () => new Promise(() => {}),
      list_reagent_lots: () => new Promise(() => {}),
    });
    await within(await screen.findByRole("navigation", { name: "Main" })).findByRole("link", {
      name: "Bench setup",
    });
    await waitFor(() => expect(currentLinks()).toEqual([["Bench setup", "true"]]));
  });

  // REQ-OPS-004: counts come only from list_work_queues, capped counts say so.
  it("shows the engine's queue counts beside their entries", async () => {
    const { user } = renderApp("/", ["enter_orders", "review_results", "release_reports"], {
      list_work_queues: () => [
        queue({ queue: "orders_needing_decision", ownerCapability: "enter_orders", count: 3 }),
        queue({ queue: "review_ready", count: 0 }),
        queue({
          queue: "release_ready",
          ownerCapability: "release_reports",
          count: 500,
          countCapped: true,
        }),
      ],
    });
    await open(user, "Front desk, 3 waiting");
    await open(user, "Review");
    await open(user, "Reports, 500 or more waiting");
    expect(screen.getByRole("link", { name: "Orders, 3 waiting" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Release queue, 500 or more waiting" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Work queues, 503 or more waiting" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Result review" })).toBeTruthy();
  });

  // Ticket #4462: a closed area totals its entries' counts; open, each entry shows its own.
  it("shows a closed work area's total, and each entry's count once opened", async () => {
    const { user } = renderApp("/", ["enter_orders", "review_results"], {
      list_work_queues: () => [
        queue({ queue: "review_ready", count: 2 }),
        queue({ queue: "import_quarantine", count: 2 }),
      ],
    });
    const review = await screen.findByRole("button", { name: "Review, 4 waiting" });
    expect(review.textContent).toBe("Review4");
    expect(screen.getByRole("button", { name: "Front desk" }).textContent).toBe("Front desk");
    await user.click(review);
    expect(review.getAttribute("aria-expanded")).toBe("true");
    expect(review.textContent).toBe("Review");
    expect(screen.getByRole("button", { name: "Review" })).toBe(review);
    expect(linksIn("Review")).toEqual(["Result review2", "Result imports2"]);
    expect(screen.getByRole("link", { name: "Result imports, 2 waiting" })).toBeTruthy();
    await user.click(review);
    expect(screen.getByRole("button", { name: "Review, 4 waiting" })).toBe(review);
  });

  it("announces unknown historical review counts without presenting zero", async () => {
    renderApp("/", ["review_results"], {
      list_work_queues: () => [queue({ queue: "review_stale", count: null })],
    });
    const nav = await screen.findByRole("navigation", { name: "Main" });
    expect(
      await within(nav).findByRole("link", { name: /Work queues.*some queues not counted/ }),
    ).toBeTruthy();
  });

  it("shows no counts when the queues cannot be read", async () => {
    const { user } = renderApp("/about", ["enter_orders"], {
      list_work_queues: () => Promise.reject(new Error("unavailable")),
    });
    await screen.findByRole("heading", { level: 1, name: "About this app" });
    await open(user, "Front desk");
    expect(screen.getByRole("link", { name: "Orders" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Work queues" })).toBeTruthy();
  });

  // Each route loads its screen on first use; a wrong export name would fail only then.
  it("loads a screen component for every route", async () => {
    const loaders = [...navigation.flatMap((section) => section.screens), ...detailScreens];
    const pages = await Promise.all(loaders.map(({ page }) => page()));
    expect(pages.every((page) => typeof page === "function")).toBe(true);
  });
});
