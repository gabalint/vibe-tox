import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import type { ExportView, ReportDetail } from "../contract";
import { DESTINATION, REPORT, REPORT_ID, REVISION, sessionWith } from "../testData";
import { ReportPage } from "./ReportPage";

const SECOND_ID = "00000000-0000-7000-8000-00000000a002";

/** A corrected report: revision 1 superseded by the current revision 2. */
const CORRECTED: ReportDetail = {
  ...REPORT,
  revisions: [
    { ...REVISION, availability: "superseded" },
    { ...REVISION, revisionId: SECOND_ID, revisionNo: 2, relationship: "correction" },
  ],
};

const RELEASER = { userId: "00000000-0000-7000-8000-000000000e02", displayName: "Test Releaser" };

const exported = (revisionNo: number, exportedAt: string): ExportView => ({
  exportId: `00000000-0000-7000-8000-00000000x00${revisionNo}`,
  revisionNo,
  withdrawalNotice: false,
  purpose: revisionNo === 1 ? "internal" : "records",
  fileName: `A26000001-URN-r${revisionNo}.pdf`,
  exportedBy: RELEASER,
  exportedAt,
  label: "PDF exported by Test Releaser (not a delivery)",
});

function renderPage(report: ReportDetail, commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({
    current_session: () => sessionWith(["release_reports", "deliver_reports"]),
    get_report: () => report,
    list_delivery_destinations: () => [DESTINATION],
    ...commands,
  });
  return renderRoutes(
    [{ path: "/reports/:reportId", element: <ReportPage /> }],
    `/reports/${REPORT_ID}`,
  );
}

describe("ReportPage exports (REQ-RPT-011, NFR-07)", () => {
  beforeEach(mockGridLayout);

  it("exports the current revision from the report page, never as a delivery", async () => {
    const exportPdf = vi.fn(() => ({ fileName: "A26000001-URN-r2.pdf", folder: "downloads" }));
    const { user } = renderPage(CORRECTED, { export_report_pdf: exportPdf });
    await screen.findByRole("heading", { level: 1, name: "Report A26000001-URN" });
    await user.click(screen.getByRole("button", { name: "Export PDF (revision 2, current)" }));
    const dialog = await screen.findByRole("dialog", { name: "Export PDF of A26000001-URN" });
    expect(dialog.textContent).toContain("is not a delivery");
    await chooseWithKeyboard(user, /Purpose/, "For the laboratory's records");
    await user.click(within(dialog).getByRole("button", { name: "Export PDF" }));
    expect(
      await screen.findByText("PDF saved to your Downloads folder. An export is not a delivery."),
    ).toBeTruthy();
    expect(exportPdf).toHaveBeenCalledWith({
      input: { revisionId: SECOND_ID, purpose: "records" },
    });
  });

  it("lists every export, most recent first, with its revision, purpose, person and file", async () => {
    renderPage(
      {
        ...CORRECTED,
        exports: [exported(1, "2026-10-12T15:10:00Z"), exported(2, "2026-10-13T09:00:00Z")],
      },
      {},
    );
    const grid = await screen.findByRole("grid", { name: "PDF exports" });
    const rows = within(grid).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("gridcell")[1]?.textContent)).toEqual([
      "Revision 2 (current)",
      "Revision 1 (superseded)",
    ]);
    const latest = rows[0] as HTMLElement;
    expect(
      within(latest).getByRole("gridcell", { name: "For the laboratory's records" }),
    ).toBeTruthy();
    expect(within(latest).getByRole("gridcell", { name: "Test Releaser" })).toBeTruthy();
    expect(within(latest).getByRole("gridcell", { name: "A26000001-URN-r2.pdf" })).toBeTruthy();
    expect(screen.getByText("An export is not a delivery.")).toBeTruthy();
  });
});
