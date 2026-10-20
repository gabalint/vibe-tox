import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ComponentGallery } from "./ComponentGallery";
import { syntheticRows } from "./syntheticRows";

const renderGallery = () =>
  renderRoutes([{ path: "/gallery", element: <ComponentGallery /> }], "/gallery");

describe("ComponentGallery", () => {
  beforeEach(mockGridLayout);

  it("renders every section without accessibility violations", async () => {
    const { container } = renderGallery();
    expect(screen.getByRole("heading", { level: 1, name: "Component gallery" })).toBeTruthy();
    expect(
      screen.getByRole("grid", { name: "Synthetic worklist" }).getAttribute("aria-rowcount"),
    ).toBe("10001");
    expect(await screen.findByText(/Newest synthetic accession read: SYN-000001/)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens a row of the 10,000-row grid with Enter", async () => {
    const { user } = renderGallery();
    const grid = screen.getByRole("grid", { name: "Synthetic worklist" });
    await user.click(within(grid).getByRole("gridcell", { name: "SYN-000002" }));
    await user.keyboard("{Enter}");
    expect(screen.getByText(/Last opened: SYN-000002\./)).toBeTruthy();
  });

  it("keeps the rows shown when a refresh fails", async () => {
    const { user } = renderGallery();
    expect(await screen.findByText("Newest synthetic accession read: SYN-000001")).toBeTruthy();
    await user.click(screen.getByRole("checkbox", { name: "Make refreshes fail" }));
    const section = screen.getByRole("region", { name: /^Data states/ });
    await user.click(within(section).getByRole("button", { name: "Refresh" }));
    expect(
      await screen.findByText("Could not refresh. The synthetic queue shown may be out of date."),
    ).toBeTruthy();
    expect(screen.getByText("Newest synthetic accession read: SYN-000001")).toBeTruthy();
  });

  it("generates synthetic identifiers only", () => {
    const rows = syntheticRows(3);
    expect(rows.map((row) => row.id)).toEqual(["SYN-000001", "SYN-000002", "SYN-000003"]);
  });
});
