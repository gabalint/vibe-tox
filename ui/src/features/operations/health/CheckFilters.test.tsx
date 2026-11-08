import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { healthCheck, hostHealth, sessionWith } from "../testData";
import { HealthPage } from "./HealthPage";

const HEALTH = hostHealth({
  overall: "alert",
  checks: [
    healthCheck(),
    healthCheck({ check: "ups", label: "UPS", state: "attested" }),
    healthCheck({ check: "restore_test_age", label: "Restore test", state: "alert" }),
    healthCheck({ check: "offline_copy_age", label: "Offline copy", state: "warning" }),
    healthCheck({ check: "clock", label: "Clock", state: "unknown" }),
  ],
});

function renderPage(path = "/operations/health") {
  mockCommands({
    current_session: () => sessionWith([]),
    get_host_health: () => HEALTH,
  });
  return renderRoutes([{ path: "/operations/health", element: <HealthPage /> }], path);
}

const shownChecks = () =>
  screen
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("gridcell")[0]?.textContent);

describe("Host health state filter (REQ-OPS-008)", () => {
  beforeEach(mockGridLayout);

  it("counts the checks by state, Unknown as Warning and Attested as OK", async () => {
    const { container } = renderPage();
    await screen.findByRole("grid", { name: "Health checks" });
    expect(screen.getByRole("button", { name: "1 Alert" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "2 Warning" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "2 OK" })).toBeTruthy();
    expect(shownChecks()).toHaveLength(5);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("filters by the chosen state and keeps the filter in the address", async () => {
    const { user, router } = renderPage();
    await screen.findByRole("grid", { name: "Health checks" });
    await chooseWithKeyboard(user, /State$/, "Warning");
    expect(router.state.location.search).toBe("?state=warning");
    expect(shownChecks()).toEqual(["Offline copy", "Clock"]);
    await user.click(screen.getByRole("button", { name: /State$/ }));
    await user.click(await screen.findByRole("option", { name: "All" }));
    expect(router.state.location.search).toBe("");
    expect(shownChecks()).toHaveLength(5);
  });

  it("applies the same filter when a count is pressed", async () => {
    const { user, router } = renderPage();
    await user.click(await screen.findByRole("button", { name: "1 Alert" }));
    expect(router.state.location.search).toBe("?state=alert");
    expect(shownChecks()).toEqual(["Restore test"]);
    expect(screen.getByRole("button", { name: /State$/ }).textContent).toContain("Alert");
  });

  it("reads the filter from the address", async () => {
    renderPage("/operations/health?state=ok");
    await screen.findByRole("grid", { name: "Health checks" });
    expect(shownChecks()).toEqual(["UPS", "Backup age"]);
  });

  it("says when no check is in the chosen state", async () => {
    mockCommands({
      current_session: () => sessionWith([]),
      get_host_health: () => hostHealth(),
    });
    renderRoutes(
      [{ path: "/operations/health", element: <HealthPage /> }],
      "/operations/health?state=alert",
    );
    expect(await screen.findByText("No checks in this state")).toBeTruthy();
    expect(screen.getByRole("button", { name: "0 Alert" })).toHaveProperty("disabled", true);
  });
});
