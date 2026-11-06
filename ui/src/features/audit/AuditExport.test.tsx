import { screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { AuditTrailPage } from "./AuditTrailPage";

function renderPage(capabilities: string[], exportEvents: (args: unknown) => unknown) {
  mockCommands({
    current_session: () => ({ capabilities }),
    list_audit_events: () => [],
    export_audit_events: exportEvents,
  });
  const router = createMemoryRouter([{ path: "/audit", element: <AuditTrailPage /> }], {
    initialEntries: ["/audit?stream=clinical&outcome=succeeded"],
  });
  return renderWithProviders(<RouterProvider router={router} />);
}

describe("Audit trail export (REQ-OPS-001)", () => {
  beforeEach(mockGridLayout);

  it("exports every event the filters match as CSV or PDF and says where the file went", async () => {
    const exportEvents = vi.fn(() => ({
      fileName: "audit-trail-2026-11-05.csv",
      folder: "downloads",
      events: 12,
    }));
    const { user } = renderPage(["view_audit", "export"], exportEvents);
    await user.click(await screen.findByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: "CSV, for analysis" }));
    expect(
      await screen.findByText("Saved audit-trail-2026-11-05.csv to your Downloads folder."),
    ).toBeTruthy();
    expect(screen.getByText("12 events exported.")).toBeTruthy();
    expect(exportEvents).toHaveBeenCalledWith({
      input: { format: "csv", filter: { stream: "clinical", outcome: "succeeded", limit: 500 } },
    });
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: "PDF, for printing" }));
    expect(exportEvents).toHaveBeenLastCalledWith({
      input: { format: "pdf", filter: { stream: "clinical", outcome: "succeeded", limit: 500 } },
    });
  });

  it("shows why an export was refused", async () => {
    const { user } = renderPage(["view_audit", "export"], () =>
      Promise.reject(engineError("validation", "More than 2000 events match these filters.")),
    );
    await user.click(await screen.findByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: "PDF, for printing" }));
    expect(await screen.findByText("Not exported")).toBeTruthy();
    expect(screen.getByText("More than 2000 events match these filters.")).toBeTruthy();
  });

  it("offers no export without the export capability", async () => {
    renderPage(["view_audit"], vi.fn());
    expect(await screen.findByRole("heading", { name: "Audit trail" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Export" })).toBeNull();
  });
});
