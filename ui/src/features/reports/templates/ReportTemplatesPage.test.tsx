import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LayoutVersionView } from "@/generated/LayoutVersionView";
import type { ReportLayoutOption } from "@/generated/ReportLayoutOption";
import type { ReportLayoutsView } from "@/generated/ReportLayoutsView";
import type { SessionInfo } from "@/generated/SessionInfo";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ReportTemplatesPage } from "./ReportTemplatesPage";

const ADMIN = "00000000-0000-4000-8000-0000000000a1";
const QUALITY = "00000000-0000-4000-8000-0000000000b2";
const DRAFT_ID = "00000000-0000-4000-8000-0000000000d1";

function session(capabilities: string[]): SessionInfo {
  return {
    userId: QUALITY,
    username: "quality",
    displayName: "Test Quality Manager",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

const option = (
  layout: ReportLayoutOption["layout"],
  name: string,
  templateId: string,
): ReportLayoutOption => ({
  layout,
  name,
  description: `Synthetic description of ${name}.`,
  templateId,
  templateVersion: 1,
});

const LAYOUTS = [
  option("standard", "Standard", "tox-report-standard"),
  option("compact", "Compact", "tox-report-compact"),
  option("results_first", "Results first", "tox-report-results-first"),
  { ...option("classic", "Classic", "tox-report"), templateVersion: 3 },
];

const DRAFT: LayoutVersionView = {
  versionId: DRAFT_ID,
  versionNo: 1,
  layout: "standard",
  name: "Standard",
  state: "draft",
  draftedBy: { userId: ADMIN, displayName: "Test Administrator" },
  draftedAt: "2026-10-27T09:00:00Z",
  decidedBy: null,
  decidedAt: null,
  rejectionReason: null,
  supersededAt: null,
  revision: 1,
};

const DEFAULT_VIEW: ReportLayoutsView = {
  layouts: LAYOUTS,
  inForce: { layout: "classic", name: "Classic", version: null },
  draft: null,
  versions: [],
  allowedActions: ["draft_report_layout"],
};

const DRAFT_VIEW: ReportLayoutsView = {
  ...DEFAULT_VIEW,
  draft: DRAFT,
  versions: [DRAFT],
  allowedActions: ["approve_report_layout", "reject_report_layout"],
};

function renderPage(capabilities: string[], commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({ current_session: () => session(capabilities), ...commands });
  return renderRoutes(
    [{ path: "/reports/templates", element: <ReportTemplatesPage /> }],
    "/reports/templates",
  );
}

describe("ReportTemplatesPage (REQ-RPT-007)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the active default layout and saves another layout as a draft", async () => {
    const draft = vi.fn(() => DRAFT_VIEW);
    const { user, container } = renderPage(["manage_configuration"], {
      get_report_layouts: () => DEFAULT_VIEW,
      draft_report_layout: draft,
    });
    expect(await screen.findByText("Active: Classic")).toBeTruthy();
    expect(screen.getByText("Built-in default, never approved")).toBeTruthy();
    const cards = screen.getAllByRole("listitem");
    expect(cards.map((card) => within(card).getByRole("heading").textContent)).toEqual([
      "Standard",
      "Compact",
      "Results first",
      "Classic",
    ]);
    // The active layout cannot be drafted again.
    expect(
      within(cards[3] as HTMLElement).queryByRole("button", { name: "Save as draft" }),
    ).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(
      within(cards[1] as HTMLElement).getByRole("button", { name: "Save as draft" }),
    );
    const dialog = await screen.findByRole("alertdialog", {
      name: "Save the Compact layout as a draft?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Save as draft" }));
    expect(draft).toHaveBeenCalledWith({ input: { layout: "compact" } });
  });

  it("previews a layout in a report window, and shows a refused preview", async () => {
    const preview = vi
      .fn()
      .mockReturnValueOnce(null)
      .mockImplementationOnce(() => {
        throw engineError("internal", "The preview could not be rendered.");
      });
    const { user } = renderPage(["manage_quality"], {
      get_report_layouts: () => ({ ...DEFAULT_VIEW, allowedActions: [] }),
      preview_report_layout: preview,
    });
    const cards = await screen.findAllByRole("listitem");
    expect(
      within(cards[0] as HTMLElement).queryByRole("button", { name: "Save as draft" }),
    ).toBeNull();
    await user.click(within(cards[2] as HTMLElement).getByRole("button", { name: "Preview" }));
    expect(preview).toHaveBeenCalledWith({ input: { layout: "results_first" } });
    await user.click(within(cards[2] as HTMLElement).getByRole("button", { name: "Preview" }));
    expect(await screen.findByText("Preview not shown")).toBeTruthy();
  });

  it("lets quality approve the draft waiting for approval", async () => {
    const approve = vi.fn(() => DEFAULT_VIEW);
    const { user } = renderPage(["manage_quality"], {
      get_report_layouts: () => DRAFT_VIEW,
      approve_report_layout: approve,
    });
    expect(await screen.findByText("Active: Classic, Standard waiting for approval")).toBeTruthy();
    const waiting = screen.getByRole("region", { name: "Waiting for approval" });
    expect(within(waiting).getByText("Test Administrator")).toBeTruthy();
    expect(screen.getAllByText("Waiting for approval").length).toBeGreaterThan(1);
    await user.click(within(waiting).getByRole("button", { name: "Approve" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Approve the Standard layout?" });
    await user.click(within(dialog).getByRole("button", { name: "Approve layout" }));
    expect(approve).toHaveBeenCalledWith({
      input: { versionId: DRAFT_ID, expectedRevision: 1, selfApproval: false },
    });
  });

  it("rejects the draft with a reason", async () => {
    const reject = vi.fn(() => DEFAULT_VIEW);
    const { user } = renderPage(["manage_quality"], {
      get_report_layouts: () => DRAFT_VIEW,
      reject_report_layout: reject,
    });
    const waiting = await screen.findByRole("region", { name: "Waiting for approval" });
    await user.click(within(waiting).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Reject the Standard layout?" });
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Synthetic: limits too small",
    );
    await user.click(within(dialog).getByRole("button", { name: "Reject layout" }));
    expect(reject).toHaveBeenCalledWith({
      input: {
        versionId: DRAFT_ID,
        expectedRevision: 1,
        reason: "Synthetic: limits too small",
      },
    });
  });
});
