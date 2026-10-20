import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Tabs } from "@/components/Tabs";
import { useRouteTabs } from "./routeTabs";
import { renderRoutes } from "./test/router";

const TABS = [
  { id: "targets", label: "Targets" },
  { id: "runs", label: "Runs" },
] as const;

function Backups() {
  const tabs = useRouteTabs("/backups", TABS);
  return (
    <Tabs label="Backups" tabs={tabs.tabs} selectedId={tabs.selectedId}>
      <p>Showing {tabs.selectedId}</p>
    </Tabs>
  );
}

const renderAt = (path: string) =>
  renderRoutes([{ path: "/backups/:tab?", element: <Backups /> }], path);

describe("useRouteTabs", () => {
  it("selects the tab named by the route and links each tab to its own route", () => {
    renderAt("/backups/runs");
    expect(screen.getByText("Showing runs")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Runs" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Targets" }).getAttribute("href")).toBe(
      "/backups/targets",
    );
  });

  it("selects the first tab on the base path or an unknown segment", () => {
    renderAt("/backups");
    expect(screen.getByText("Showing targets")).toBeTruthy();
  });

  it("opens another tab on a click", async () => {
    const { user, router } = renderAt("/backups/unknown");
    expect(screen.getByText("Showing targets")).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Runs" }));
    expect(router.state.location.pathname).toBe("/backups/runs");
    expect(screen.getByText("Showing runs")).toBeTruthy();
  });
});
