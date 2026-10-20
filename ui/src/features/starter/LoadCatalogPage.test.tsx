import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { StandardCatalogLoaded } from "@/generated/StandardCatalogLoaded";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { LOAD_CATALOG_PATH } from "./contract";
import { DISCLAIMER, renderStarter, TREE } from "./testData";

const LOADED: StandardCatalogLoaded = {
  loadId: "00000000-0000-7000-8000-00000000c001",
  specimenTypes: 1,
  analytes: 2,
  methods: 1,
  orderables: 1,
  skipped: 0,
};

function renderLoad(commands: Record<string, (args: unknown) => unknown> = {}) {
  return renderStarter(LOAD_CATALOG_PATH, ["manage_configuration"], {
    standard_catalog_tree: () => TREE,
    ...commands,
  });
}

function list(name: RegExp) {
  return screen.getByRole("region", { name });
}

function check(region: HTMLElement, name: RegExp) {
  return within(region).getByRole("checkbox", { name });
}

describe("Load Standard Catalog (proposed REQ-OPS-014)", () => {
  it("lists both trees with each item's state and the file's version", async () => {
    const { container } = renderLoad();
    expect(await screen.findByText(/version 1.0.0: unreviewed starter content/)).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Load Standard Catalog" })).toBeTruthy();
    const tests = list(/Tests and panels/);
    expect(check(tests, /Oral fluid opioids/).hasAttribute("disabled")).toBe(true);
    expect(within(tests).getByText("Method already in your catalog; add this test by hand."));
    const methods = list(/^Methods/);
    expect(check(methods, /Oral fluid opioids LC-MS\/MS/).hasAttribute("disabled")).toBe(true);
    expect(within(methods).getAllByText("Already in your catalog").length).toBeGreaterThan(0);
    expect(check(methods, /Urine cannabinoids/).hasAttribute("disabled")).toBe(true);
    expect(within(methods).getAllByText(/different name/).length).toBeGreaterThan(0);
    await expectNoA11yViolations(container);
  });

  it("selects a panel's method as needed, and releases it when the panel is cleared", async () => {
    const { user } = renderLoad();
    await screen.findByRole("region", { name: /Tests and panels/ });
    await user.click(check(list(/Tests and panels/), /Urine screen panel/));
    const method = check(list(/^Methods/), /Urine immunoassay screen/) as HTMLInputElement;
    expect(method.checked).toBe(true);
    expect(method.hasAttribute("disabled")).toBe(true);
    expect(within(list(/^Methods/)).getByText("Needed by Urine screen panel")).toBeTruthy();
    expect(within(list(/^Methods/)).getByText("Needed by Urine immunoassay screen")).toBeTruthy();

    await user.click(check(list(/Tests and panels/), /Urine screen panel/));
    expect(method.checked).toBe(false);
    expect(method.hasAttribute("disabled")).toBe(false);
    expect(within(list(/^Methods/)).queryByText("Needed by Urine screen panel")).toBeNull();
  });

  it("keeps Load disabled until the disclaimer is read, then sends only the choices", async () => {
    const load = vi.fn(() => LOADED);
    const { user } = renderLoad({ load_standard_catalog: load });
    await screen.findByRole("region", { name: /Tests and panels/ });
    const button = screen.getByRole("button", { name: "Load Standard Catalog" });
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("Choose at least one test, panel or method.")).toBeTruthy();

    await user.click(check(list(/Tests and panels/), /Urine screen panel/));
    expect(button.hasAttribute("disabled")).toBe(true);
    const summary = list(/Summary/);
    expect(within(summary).getByText(DISCLAIMER)).toBeTruthy();
    expect(within(summary).getByText("Panels").nextSibling?.textContent).toBe("1");
    expect(within(summary).getByText("Analytes").nextSibling?.textContent).toBe("2");

    await user.click(screen.getByRole("checkbox", { name: "I have read this" }));
    expect(button.hasAttribute("disabled")).toBe(false);
    await user.click(button);
    expect(await screen.findByText("Standard Catalog loaded as drafts")).toBeTruthy();
    expect(load).toHaveBeenCalledWith({
      input: {
        catalogVersion: "1.0.0",
        orderableCodes: ["UR-SCREEN"],
        methodCodes: [],
        omittedAnalytes: [],
        disclaimerAcknowledged: true,
      },
    });
    await user.click(screen.getByRole("link", { name: "Go to Catalog review" }));
    expect(await screen.findByText("Navigated to /configuration/catalog-review")).toBeTruthy();
  });

  it("removes an analyte from the method and its tests, but never the last one", async () => {
    const { user } = renderLoad();
    await screen.findByRole("region", { name: /Tests and panels/ });
    await user.click(check(list(/Tests and panels/), /Urine opioids/));
    const methods = list(/^Methods/);
    const opioids = check(methods, /Urine opioids LC-MS\/MS/).closest("li");
    if (!(opioids instanceof HTMLElement)) throw new Error("no method item");
    await user.click(check(methods, /^Codeine/));
    expect(
      within(methods).getByText(/Removed from this method\. Also removed from Urine opioids\./),
    ).toBeTruthy();
    expect(within(list(/Tests and panels/)).getByText(/Removed from Urine opioids LC-MS\/MS/));

    await user.click(check(opioids, /^Morphine/));
    expect(
      within(methods).getByText(
        "Urine opioids needs at least one analyte from Urine opioids LC-MS/MS; this is its last one.",
      ),
    ).toBeTruthy();
    expect((check(opioids, /^Morphine/) as HTMLInputElement).checked).toBe(true);
  });

  it("blocks loading an unavailable analyte until it is removed", async () => {
    const { user } = renderLoad();
    await screen.findByRole("region", { name: /Tests and panels/ });
    await user.click(check(list(/^Methods/), /Urine opioids LC-MS\/MS/));
    await user.click(screen.getByRole("checkbox", { name: "I have read this" }));
    const button = screen.getByRole("button", { name: "Load Standard Catalog" });
    expect(screen.getByText("These items cannot be loaded")).toBeTruthy();
    expect(button.hasAttribute("disabled")).toBe(true);
    await user.click(check(list(/^Methods/), /^6-Acetylmorphine/));
    expect(screen.queryByText("These items cannot be loaded")).toBeNull();
    expect(button.hasAttribute("disabled")).toBe(false);
  });

  it("explains a refused load and offers to reload the list", async () => {
    const tree = vi.fn(() => TREE);
    const { user } = renderLoad({
      standard_catalog_tree: tree,
      load_standard_catalog: () =>
        Promise.reject(
          engineError("conflict", "the standard catalog version changed; reload the list"),
        ),
    });
    await screen.findByRole("region", { name: /Tests and panels/ });
    await user.click(check(list(/^Methods/), /Urine immunoassay screen/));
    await user.click(screen.getByRole("checkbox", { name: "I have read this" }));
    await user.click(screen.getByRole("button", { name: "Load Standard Catalog" }));
    expect(await screen.findByText("Not loaded")).toBeTruthy();
    expect(screen.getByText(/changed since this list was shown/)).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Reload the list" }));
    expect(tree).toHaveBeenCalledTimes(2);
  });

  it("filters both lists with Find, keeping chosen items listed", async () => {
    const { user } = renderLoad();
    await screen.findByRole("region", { name: /Tests and panels/ });
    await user.click(check(list(/Tests and panels/), /Urine screen panel/));
    await user.type(
      screen.getByRole("searchbox", { name: "Find tests, panels and methods" }),
      "opioids{Enter}",
    );
    expect(check(list(/Tests and panels/), /Urine screen panel/)).toBeTruthy();
    expect(check(list(/Tests and panels/), /^Urine opioids UR-OPI$/)).toBeTruthy();
    expect(within(list(/^Methods/)).queryByRole("checkbox", { name: /cannabinoids/ })).toBeNull();

    await user.click(check(list(/Tests and panels/), /Urine screen panel/));
    expect(
      within(list(/Tests and panels/)).queryByRole("checkbox", { name: /Urine screen panel/ }),
    ).toBeNull();
    const find = screen.getByRole("searchbox", { name: "Find tests, panels and methods" });
    await user.clear(find);
    await user.type(find, "zzz{Enter}");
    expect(screen.getByText("No tests, panels or methods match 'zzz'")).toBeTruthy();
  });
});
