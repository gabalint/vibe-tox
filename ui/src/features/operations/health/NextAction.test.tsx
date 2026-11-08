import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { healthCheck, hostHealth, sessionWith } from "../testData";
import { HealthPage } from "./HealthPage";

const HEALTH = hostHealth({
  overall: "alert",
  checks: [
    healthCheck({
      check: "backup_configured",
      label: "Backup target",
      state: "alert",
      nextAction: "Add a backup target.",
    }),
    healthCheck({
      check: "restore_test_age",
      label: "Restore test",
      state: "alert",
      nextAction: "Run a restore test.",
    }),
    healthCheck({
      check: "disk_encryption",
      label: "Disk encryption",
      state: "unknown",
      nextAction: "Turn on disk encryption, or attest it if the app cannot read it.",
      attestable: true,
    }),
    healthCheck(),
  ],
});

function renderPage(capabilities: string[], path = "/operations/health") {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_host_health: () => HEALTH,
  });
  return renderRoutes(
    [
      { path: "/operations/health", element: <HealthPage /> },
      { path: "/operations/backups/:tab?", element: <h1>Backups page</h1> },
    ],
    path,
  );
}

describe("Host health next action links (REQ-OPS-008)", () => {
  beforeEach(mockGridLayout);

  it("links each next action done in the app to its screen, in the table and the panel", async () => {
    const { user, router, container } = renderPage(["backup_restore", "manage_configuration"]);
    const target = await screen.findByRole("link", { name: "Open Backup targets" });
    expect(target.getAttribute("href")).toBe("/operations/backups/targets");
    expect(screen.getByRole("link", { name: "Open Restore tests" }).getAttribute("href")).toBe(
      "/operations/backups/restore-tests",
    );
    // Done outside the app: text only, with Attest still in the row's menu.
    expect(screen.getByText(/^Turn on disk encryption/)).toBeTruthy();
    expect(screen.getAllByRole("link", { name: /^Open / })).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Actions for Disk encryption" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await openRow(user, "Restore test");
    const panel = screen.getByRole("region", { name: "Restore test" });
    await user.click(within(panel).getByRole("link", { name: "Open Restore tests" }));
    expect(router.state.location.pathname).toBe("/operations/backups/restore-tests");
    expect(screen.getByRole("heading", { name: "Backups page" })).toBeTruthy();
  });

  it("shows the next action as text to people who may not act on its screen", async () => {
    renderPage(["manage_configuration"], "/operations/health?open=backup_configured");
    const panel = await screen.findByRole("region", { name: "Backup target" });
    expect(within(panel).getByText("Add a backup target.")).toBeTruthy();
    expect(screen.queryByRole("link", { name: /^Open / })).toBeNull();
  });
});
