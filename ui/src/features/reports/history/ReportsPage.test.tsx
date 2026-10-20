import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import type { ReportSummary } from "../contract";
import { REPORT_ID, sessionWith } from "../testData";
import { ReportsPage } from "./ReportsPage";

const ENTRY: ReportSummary = {
  reportId: REPORT_ID,
  reportNumber: "A26000001-URN",
  specimenId: "00000000-0000-7000-8000-000000006001",
  accessionNumber: "A26000001",
  latestRevisionNo: 2,
  availability: "current",
  completeness: "final",
  releasedAt: "2026-10-12T15:00:00Z",
};

function renderPage(path: string, list: (args: unknown) => unknown) {
  mockCommands({
    current_session: () => sessionWith(["deliver_reports"]),
    list_reports: list,
  });
  return renderRoutes([{ path: "/reports", element: <ReportsPage /> }], path);
}

describe("ReportsPage (REQ-RPT-002)", () => {
  beforeEach(mockGridLayout);

  it("lists reports and opens one with Enter", async () => {
    const list = vi.fn(() => [ENTRY]);
    const { user, container } = renderPage("/reports", list);
    expect(await screen.findByRole("gridcell", { name: "A26000001-URN" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { filter: "all", limit: 200 } });
    expect(screen.getByRole("gridcell", { name: "2026-10-12 15:00 UTC" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "A26000001" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "2" }).getAttribute("data-align")).toBe("end");
    expect(screen.queryByText(/double-click/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "A26000001");
    expect(await screen.findByText(`Navigated to /reports/${REPORT_ID}`)).toBeTruthy();
  });

  it("opens a report with a single click, passing the list for its breadcrumb", async () => {
    const { user, router } = renderPage("/reports?filter=partial", () => [ENTRY]);
    await openRow(user, "A26000001-URN");
    expect(await screen.findByText(`Navigated to /reports/${REPORT_ID}`)).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Reports", href: "/reports?filter=partial" },
        ids: [REPORT_ID],
      },
    });
  });

  it("loads older reports from where the engine's page ended", async () => {
    const page = (from: number, count: number, hasMore: boolean): ReportSummary[] =>
      Array.from({ length: count }, (_, index) => ({
        ...ENTRY,
        reportId: `00000000-0000-7000-8000-${String(from + index).padStart(12, "0")}`,
        reportNumber: `A2600${String(from + index).padStart(4, "0")}-URN`,
        releasedAt: `2026-10-12T15:${String(59 - Math.floor((from + index) / 60)).padStart(2, "0")}:${String(59 - ((from + index) % 60)).padStart(2, "0")}Z`,
        hasMore,
      }));
    const first = page(0, 200, true);
    const list = vi
      .fn()
      .mockReturnValueOnce(first)
      .mockReturnValueOnce(page(200, 3, false));
    const { user } = renderPage("/reports", list);
    expect(await screen.findByText("200 reports shown · more available")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load older reports" }));
    expect(await screen.findByText("203 reports shown")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: {
        filter: "all",
        limit: 200,
        beforeReleasedAt: first.at(-1)?.releasedAt,
        beforeReportId: first.at(-1)?.reportId,
      },
    });
    expect(screen.getByRole("grid").getAttribute("aria-rowcount")).toBe("204");
    expect(screen.queryByRole("button", { name: "Load older reports" })).toBeNull();
  });

  it("finds by report number after a pause in typing, with no usage hint", async () => {
    const list = vi.fn(() => [ENTRY]);
    const { user, router } = renderPage("/reports", list);
    await screen.findByRole("gridcell", { name: "A26000001-URN" });
    expect(screen.queryByText(/Press Enter/)).toBeNull();
    await user.type(screen.getByRole("searchbox", { name: "Find by report number" }), "A26");
    await vi.waitFor(() => expect(router.state.location.search).toBe("?q=A26"));
    await vi.waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { filter: "all", search: "A26", limit: 200 },
      }),
    );
  });

  it("keeps the filter and the find text in the address", async () => {
    const list = vi.fn(() => [ENTRY]);
    const { user, router } = renderPage("/reports", list);
    await screen.findByRole("gridcell", { name: "A26000001-URN" });
    await chooseWithKeyboard(user, /Show$/, "Partial reports");
    expect(list).toHaveBeenLastCalledWith({ input: { filter: "partial", limit: 200 } });
    await user.type(
      screen.getByRole("searchbox", { name: "Find by report number" }),
      "A26000001{Enter}",
    );
    expect(router.state.location.search).toBe("?filter=partial&q=A26000001");
    expect(list).toHaveBeenLastCalledWith({
      input: { filter: "partial", search: "A26000001", limit: 200 },
    });
  });

  it("says when nothing matches, and shows all reports again", async () => {
    const list = vi.fn(() => []);
    const { user } = renderPage("/reports?filter=withdrawn&q=A26", list);
    expect(await screen.findByText("No report matches")).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show all reports" }));
    expect(await screen.findByText("No report has been released yet.")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({ input: { filter: "all", limit: 200 } });
    expect(screen.getByRole("searchbox", { name: "Find by report number" })).toHaveProperty(
      "value",
      "",
    );
  });

  it("shows the engine's error", async () => {
    renderPage("/reports", () =>
      Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
    );
    expect(await screen.findByText("You do not have permission for this action.")).toBeTruthy();
  });
});
