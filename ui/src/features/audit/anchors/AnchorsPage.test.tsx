import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AnchorExportView } from "@/generated/AnchorExportView";
import type { AnchorStatusView } from "@/generated/AnchorStatusView";
import type { AnchorVerification } from "@/generated/AnchorVerification";
import type { ExportReport } from "@/generated/ExportReport";
import type { SessionInfo } from "@/generated/SessionInfo";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { AnchorsPage } from "./AnchorsPage";

function session(capabilities: string[]): SessionInfo {
  return {
    userId: "00000000-0000-7000-8000-0000000000a1",
    username: "auditor",
    displayName: "Test Auditor",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

const STATUS: AnchorStatusView = {
  streams: [
    {
      stream: "clinical",
      headSeq: 1350,
      sealedThroughSeq: 1340,
      anchoredThroughSeq: 1201,
      oldestUnanchoredAt: "2026-10-13T13:45:00Z",
    },
    {
      stream: "security",
      headSeq: 88,
      sealedThroughSeq: 88,
      anchoredThroughSeq: 88,
      oldestUnanchoredAt: null,
    },
  ],
  lagMinutes: 20,
  offHostTargets: 1,
  lastExport: { at: "2026-10-13T13:30:00Z", targetLabel: "Synthetic USB drive" },
};

const EXPORTS: AnchorExportView[] = [
  {
    exportId: "00000000-0000-7000-8000-0000000000a2",
    targetLabel: "Synthetic USB drive",
    fileName: "anchors-20261013T133000Z.jsonl",
    exportedAt: "2026-10-13T13:30:00Z",
    ranges: [
      { stream: "clinical", firstSealNo: 11, lastSealNo: 12 },
      { stream: "security", firstSealNo: 4, lastSealNo: 4 },
    ],
  },
];

const VERIFICATION: AnchorVerification = {
  anchorsRead: 40,
  summary: {
    compared: 40,
    matched: 38,
    lostInterval: 0,
    unverifiedRestoreBoundary: 0,
    mismatched: 1,
    localEventsMissing: 0,
    conflicts: 3,
    gaps: 1,
    unanchored: 2,
  },
  chains: [
    { stream: "clinical", intact: true },
    { stream: "security", intact: false },
  ],
  findings: [
    { code: "anchor_mismatch", stream: "clinical", sealNo: 9, lastSealNo: 9, problem: true },
    { code: "unanchored", stream: "access", sealNo: 5, lastSealNo: 7, problem: false },
    { code: "new_code", stream: "security", sealNo: null, lastSealNo: null, problem: true },
    { code: "unreadable_file", stream: null, sealNo: null, lastSealNo: null, problem: true },
  ],
};

function renderPage(capabilities: string[], commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({
    current_session: () => session(capabilities),
    get_anchor_status: () => STATUS,
    list_anchor_exports: () => EXPORTS,
    ...commands,
  });
  return renderRoutes([{ path: "/audit/anchors", element: <AnchorsPage /> }], "/audit/anchors");
}

describe("AnchorsPage (NFR-18)", () => {
  beforeEach(mockGridLayout);

  it("shows the streams, the lag as sent and the files written, and exports now", async () => {
    const exportNow = vi.fn(
      (): ExportReport => ({
        targetsWritten: 2,
        sealsWritten: 3,
        targetsFailed: 0,
        targetsUnavailable: 0,
      }),
    );
    const { user, container } = renderPage(["view_audit", "backup_restore"], {
      export_anchors_now: exportNow,
    });
    expect(await screen.findByRole("gridcell", { name: "Clinical" })).toBeTruthy();
    expect(screen.getByText("20 minutes")).toBeTruthy();
    expect(screen.getByText("Anchor lag never blocks the release of reports.")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "1201" })).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Anchor files" }));
    expect(
      await screen.findByRole("gridcell", { name: "Clinical: seals 11–12; Security: seals 4–4" }),
    ).toBeTruthy();
    expect(screen.queryByText("No off-host target")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Export anchors now" }));
    expect(await screen.findByText("Anchors exported")).toBeTruthy();
    expect(screen.getByText("Seals written: 3. Targets written: 2.")).toBeTruthy();
    expect(exportNow).toHaveBeenCalledTimes(1);
  });

  it("keeps status visible and reads files only when their tab is explicitly opened", async () => {
    const list = vi.fn(() => EXPORTS);
    const { user, router } = renderPage(["view_audit"], { list_anchor_exports: list });
    expect(await screen.findByRole("grid", { name: "Audit streams" })).toBeTruthy();
    const verifyTab = screen.getByRole("tab", { name: "Verify anchors" });
    verifyTab.focus();
    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Anchor files" }));
    expect(list).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("grid", { name: "Anchor files" })).toBeTruthy();
    expect(screen.getByRole("grid", { name: "Audit streams" })).toBeTruthy();
    expect(screen.getByText("1 anchor file shown")).toBeTruthy();
    expect(router.state.location.search).toBe("?tab=files");
    expect(list).toHaveBeenCalledWith({ input: { limit: 101 } });
  });

  it("verifies the anchors and lists what was found", async () => {
    const verify = vi.fn(() => VERIFICATION);
    const { user, container } = renderPage(["view_audit"], { verify_audit_anchors: verify });
    expect(screen.queryByRole("button", { name: "Export anchors now" })).toBeNull();
    expect(screen.getByText(/Each verification is recorded in the audit trail/)).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: "Verify anchors" }));
    expect(
      await screen.findByText(
        "The kept anchor does not match this computer's audit trail (Clinical stream, seal 9).",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText("Sealed on this computer but not yet anchored (Access stream, seals 5–7)."),
    ).toBeTruthy();
    expect(screen.getByText("new_code (Security stream).")).toBeTruthy();
    expect(screen.getByText("Some anchor files could not be read.")).toBeTruthy();
    expect(screen.getAllByText("Needs investigation").length).toBe(3);
    expect(screen.getAllByText("For information").length).toBe(1);
    expect(screen.getByText("Conflicting anchors for one seal").nextSibling?.textContent).toBe("3");
    expect(screen.getByText("Not intact")).toBeTruthy();
    expect(verify).toHaveBeenCalledWith({ input: { chooseFolder: false } });
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("warns when an export missed a target, so its anchors stay pending", async () => {
    const { user } = renderPage(["view_audit", "backup_restore"], {
      export_anchors_now: (): ExportReport => ({
        targetsWritten: 1,
        sealsWritten: 3,
        targetsFailed: 1,
        targetsUnavailable: 1,
      }),
    });
    await user.click(await screen.findByRole("button", { name: "Export anchors now" }));
    expect(await screen.findByText("Anchors not written to every target")).toBeTruthy();
    expect(
      screen.getByText(
        "Seals written: 3. Targets written: 1. Targets failed: 1. Targets not reachable: 1. Their anchors stay pending.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Anchors exported")).toBeNull();
  });

  it("shows the engine's refusal when the chosen folder cannot be read", async () => {
    const { user } = renderPage(["view_audit"], {
      verify_audit_anchors: () =>
        Promise.reject(engineError("conflict", "The chosen folder cannot be read.")),
    });
    await user.click(
      await screen.findByRole("button", { name: "Verify with anchors from another folder" }),
    );
    expect(await screen.findByText("The chosen folder cannot be read.")).toBeTruthy();
    expect(screen.getByText("Anchors not verified")).toBeTruthy();
    expect(screen.queryByText("No findings.")).toBeNull();
  });

  it("says so when the folder dialog is cancelled", async () => {
    const verify = vi.fn(() => null);
    const { user } = renderPage(["view_audit"], { verify_audit_anchors: verify });
    await user.click(
      await screen.findByRole("button", { name: "Verify with anchors from another folder" }),
    );
    expect(await screen.findByText("No folder was chosen, so nothing was verified.")).toBeTruthy();
    expect(verify).toHaveBeenCalledWith({ input: { chooseFolder: true } });
  });

  it("warns when there is no off-host target and says when no file was written", async () => {
    const { container, user } = renderPage(["view_audit"], {
      get_anchor_status: () => ({
        ...STATUS,
        offHostTargets: 0,
        lagMinutes: null,
        lastExport: null,
      }),
      list_anchor_exports: () => [],
    });
    expect(await screen.findByText("No off-host target")).toBeTruthy();
    expect(screen.getByText("Not reported")).toBeTruthy();
    expect(screen.getByText("None yet")).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Anchor files" }));
    expect(await screen.findByText("No anchor files yet")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's errors", async () => {
    const { user } = renderPage(["view_audit", "backup_restore"], {
      get_anchor_status: () => Promise.reject(engineError("internal", "Status unavailable.")),
      export_anchors_now: () =>
        Promise.reject(
          engineError("conflict", "Connect a backup target first: none is available."),
        ),
    });
    expect(await screen.findByText("Status unavailable.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Export anchors now" }));
    expect(
      await screen.findByText("Connect a backup target first: none is available."),
    ).toBeTruthy();
    expect(screen.getByText("Anchors not exported")).toBeTruthy();
    expect(screen.getAllByText("corr-test-0001").length).toBe(2);
  });
});
