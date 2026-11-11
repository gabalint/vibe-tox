import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import type { RevisionDetail } from "../contract";
import { revisionPath } from "../paths";
import { CONTENT, REPORT, REVISION, REVISION_ID, sessionWith } from "../testData";
import { RevisionPage } from "./RevisionPage";

const DETAIL: RevisionDetail = {
  reportId: REPORT.reportId,
  reportNumber: "A26000001-URN",
  summary: { ...REVISION, availability: "superseded" },
  content: CONTENT,
  provenance: {
    layoutName: null,
    templateId: "tox-report",
    templateVersion: 2,
    templateSha256: "b2".repeat(32),
    rendererVersion: "synthetic-renderer 1.0",
    contentSha256: "c3".repeat(32),
    dataSha256: "d4".repeat(32),
    artifactSha256: REVISION.artifactSha256,
    fonts: ["Synthetic Sans"],
  },
};

const PATH = `/reports/revisions/${REVISION_ID}`;

const ROUTE = { path: "/reports/revisions/:revisionId", element: <RevisionPage /> };

function mockPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["deliver_reports"]),
    get_report_revision: () => DETAIL,
    get_report: () => REPORT,
    ...commands,
  });
}

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockPage(commands);
  return renderRoutes([ROUTE], PATH);
}

describe("RevisionPage (REQ-RPT-001, NFR-07)", () => {
  it("shows the issued content, its provenance and its availability", async () => {
    const get = vi.fn(() => DETAIL);
    const { container } = renderPage({ get_report_revision: get });
    expect(
      await screen.findByRole("heading", { level: 1, name: "Report A26000001-URN, revision 1" }),
    ).toBeTruthy();
    expect(get).toHaveBeenCalledWith({ input: { revisionId: REVISION_ID } });
    expect(screen.getByText("Superseded")).toBeTruthy();
    expect(screen.getByRole("link", { name: "A26000001-URN" }).getAttribute("href")).toBe(
      `/reports/${DETAIL.reportId}`,
    );
    expect(screen.getByText("tox-report version 2, renderer synthetic-renderer 1.0")).toBeTruthy();
    expect(screen.getByText(REVISION.artifactSha256)).toBeTruthy();
    expect(screen.getByRole("article", { name: /Printed content/ })).toBeTruthy();
    // Opened from no list, the breadcrumb leads to the revision's report.
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(
      within(crumbs).getByRole("link", { name: "Report A26000001-URN" }).getAttribute("href"),
    ).toBe(`/reports/${DETAIL.reportId}`);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("names the patient and the specimen from the revision's report", async () => {
    const report = vi.fn(() => REPORT);
    renderPage({ get_report: report });
    const identity = await screen.findByRole("region", { name: "Patient and specimen identity" });
    expect(identity.textContent).toContain("Testpatient, Jane");
    expect(within(identity).getByText("MRN-00000")).toBeTruthy();
    expect(within(identity).getByText("A26000001")).toBeTruthy();
    expect(report).toHaveBeenCalledWith({ input: { reportId: DETAIL.reportId } });
  });

  it("keeps focus on Next while the next revision loads", async () => {
    const second = "00000000-0000-7000-8000-00000000e002";
    mockPage({
      get_report_revision: (args) =>
        JSON.stringify(args).includes(second)
          ? { ...DETAIL, summary: { ...DETAIL.summary, revisionId: second, revisionNo: 2 } }
          : DETAIL,
    });
    const { user } = await renderFromList({
      route: ROUTE,
      recordPath: revisionPath,
      ids: [REVISION_ID, second, "00000000-0000-7000-8000-00000000e003"],
    });
    await screen.findByRole("heading", { level: 1, name: "Report A26000001-URN, revision 1" });
    await expectNextKeepsFocus(user, "Report A26000001-URN, revision 2");
  });

  it("exports the issued PDF for the records, never as a delivery", async () => {
    const exportPdf = vi.fn(() => ({ fileName: "A26000001-URN-r1.pdf", folder: "downloads" }));
    const { user } = renderPage({ export_report_pdf: exportPdf });
    await user.click(await screen.findByRole("button", { name: "Export PDF" }));
    const dialog = await screen.findByRole("dialog", { name: "Export PDF of A26000001-URN" });
    expect(dialog.textContent).toContain("is not a delivery");
    await user.click(within(dialog).getByRole("button", { name: /Purpose/ }));
    expect(screen.queryByRole("option", { name: /For delivery/ })).toBeNull();
    await user.keyboard("{Escape}");
    await chooseWithKeyboard(user, /Purpose/, "For the laboratory's records");
    await user.click(within(dialog).getByRole("button", { name: "Export PDF" }));
    expect(
      await screen.findByText("PDF saved to your Downloads folder. An export is not a delivery."),
    ).toBeTruthy();
    expect(exportPdf).toHaveBeenCalledWith({
      input: { revisionId: REVISION_ID, purpose: "records" },
    });
  });

  it("shows a preservation incident the engine reports on export", async () => {
    const INCIDENT =
      "The issued report file is missing or damaged; this is a preservation incident.";
    const { user } = renderPage({
      export_report_pdf: () =>
        Promise.reject(
          engineError("not_found", INCIDENT, {
            gateReasons: [{ code: "artifact_missing", message: INCIDENT }],
          }),
        ),
    });
    await user.click(await screen.findByRole("button", { name: "Export PDF" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Purpose/, "Internal use");
    await user.click(within(dialog).getByRole("button", { name: "Export PDF" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("Issued report file missing or damaged");
  });

  it("shows the engine's error", async () => {
    renderPage({
      get_report_revision: () =>
        Promise.reject(engineError("not_found", "The report revision was not found.")),
    });
    expect(await screen.findByText("The report revision was not found.")).toBeTruthy();
  });
});
