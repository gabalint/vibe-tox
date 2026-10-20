import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DemoDataLoaded } from "@/generated/DemoDataLoaded";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockGridLayout } from "@/lib/test/layout";
import { STARTER_DATA_PATH } from "./contract";
import { LOADED, NOT_LOADED, renderStarter, TREE } from "./testData";

type Handler = (args: unknown) => unknown;

const NOT_DEMO = { isDemo: false, demoDataLoaded: false };
const BOTH = ["manage_configuration", "approve_configuration"];

const DEMO_LOADED: DemoDataLoaded = {
  dataVersion: "1",
  notice: "Everything in the demo file is fictitious.",
  laboratoryProfileSet: true,
  clients: 4,
  clientLocations: 7,
  deliveryDestinations: 4,
  practitioners: 8,
  patients: 12,
  identityHolds: 1,
  demoCutoffsFilled: 9,
  demoCutoffsNotFilled: 2,
};

function renderPage(capabilities: readonly string[], commands: Record<string, Handler> = {}) {
  return renderStarter(STARTER_DATA_PATH, capabilities, {
    standard_catalog_tree: () => TREE,
    standard_catalog_status: () => LOADED,
    demo_status: () => NOT_DEMO,
    ...commands,
  });
}

describe("Starter data (REQ-OPS-014, -016)", () => {
  beforeEach(() => {
    mockGridLayout();
  });

  it("links to the laboratory's catalog review without loading preview history", async () => {
    const previews = vi.fn(() => []);
    const { user } = renderPage(BOTH, { list_catalog_previews: previews });
    await screen.findByText("Unreviewed starter content");
    expect(screen.queryByRole("button", { name: "Generate Catalog Preview" })).toBeNull();
    expect(previews).not.toHaveBeenCalled();
    await user.click(screen.getByRole("link", { name: "Catalog review" }));
    expect(await screen.findByText("Navigated to /configuration/catalog-review")).toBeTruthy();
  });

  it("shows the Standard Catalog's version, review status and loads", async () => {
    const { user, container } = renderPage(["manage_configuration"]);
    expect(await screen.findByText("Unreviewed starter content")).toBeTruthy();
    expect(screen.getByText("Version shipped").nextSibling?.textContent).toBe("1.0.0");
    expect(screen.getByText("Drafts awaiting review").nextSibling?.textContent).toBe("5");
    expect(screen.getByText(/Version 1.0.0, loaded/).textContent).toContain("14 records");
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Load Standard Catalog" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Load Standard Catalog" }),
    ).toBeTruthy();
  });

  it("offers approvers no load and reads no catalog tree", async () => {
    const tree = vi.fn(() => TREE);
    renderPage(["approve_configuration"], {
      standard_catalog_tree: tree,
      standard_catalog_status: () => NOT_LOADED,
    });
    expect(
      await screen.findByText("Nothing has been loaded from the Standard Catalog."),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load Standard Catalog" })).toBeNull();
    expect(tree).not.toHaveBeenCalled();
  });

  it("shows demo data only in the demo laboratory", async () => {
    renderPage(BOTH);
    await screen.findByText("Unreviewed starter content");
    expect(screen.queryByRole("heading", { name: "Demo data" })).toBeNull();
  });

  it("loads demo data after confirmation and shows what it created", async () => {
    const load = vi.fn(() => DEMO_LOADED);
    const { user } = renderPage(BOTH, {
      demo_status: () => ({ isDemo: true, demoDataLoaded: false }),
      load_demo_data: load,
    });
    const section = within(await screen.findByRole("region", { name: "Demo data" }));
    expect(section.getByText(/needs the Administrator and Accessioner roles/)).toBeTruthy();
    expect(section.getByText(/Load the Standard Catalog first, signed in as the same person/));
    await user.click(section.getByRole("button", { name: "Load Demo Data" }));
    const dialog = within(screen.getByRole("alertdialog", { name: "Load demo data?" }));
    await user.click(dialog.getByRole("button", { name: "Load Demo Data" }));
    expect(await section.findByText("Demo data loaded")).toBeTruthy();
    expect(section.getByText("Patients").nextSibling?.textContent).toBe("12");
    expect(section.getByText("Blank cutoffs not filled").nextSibling?.textContent).toBe("2");
    expect(section.getByText(/another person loaded those drafts/)).toBeTruthy();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("says when demo data is already loaded", async () => {
    renderPage(BOTH, { demo_status: () => ({ isDemo: true, demoDataLoaded: true }) });
    expect(await screen.findByText("Demo data is loaded.")).toBeTruthy();
  });
});
