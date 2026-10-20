import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { AMPHETAMINE, AUTHOR, CURRENT_VIEW, DRAFT_VIEW, OPIATE, sessionWith } from "../testData";
import { ClassMappingPage } from "./ClassMappingPage";
import { ClassMappingsPage } from "./ClassMappingsPage";

const AWAITING: BillingClassMappingVersionView = {
  ...DRAFT_VIEW,
  status: "approved",
  revision: 2,
  createdBy: AUTHOR,
  approvedBy: "00000000-0000-7000-8000-0000000000e3",
  approvedAt: "2026-10-12T10:00:00Z",
  allowedActions: ["confirm_class_mapping_version"],
};

/** The version list as the engine returns it beside the version shown. */
function summaryOf({ entries, allowedActions, ...summary }: BillingClassMappingVersionView) {
  return summary;
}

function renderPage(
  version: BillingClassMappingVersionView,
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(["billing", "approve_configuration"]),
    list_analytes: () => [AMPHETAMINE, OPIATE],
    list_class_mapping_versions: () => [summaryOf(version)],
    get_class_mapping_version: () => version,
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/billing/class-mappings", element: <ClassMappingsPage /> },
      { path: "/billing/class-mappings/:versionId", element: <ClassMappingPage /> },
    ],
    `/billing/class-mappings?version=${version.version}`,
  );
}

describe("ClassMappingPage (REQ-BIL-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the lifecycle and entries, with an unknown analyte by its ID", async () => {
    const { container } = renderPage(DRAFT_VIEW);
    expect(await screen.findByRole("gridcell", { name: "Synthetic amphetamine" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", { name: "00000000-0000-7000-8000-0000000000a9" }),
    ).toBeTruthy();
    expect(screen.getByText("Not approved")).toBeTruthy();
    expect(screen.getAllByText("Not yet effective").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Retire version" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Confirm/ })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("keeps a version's old address working, showing it by its number", async () => {
    mockCommands({
      current_session: () => sessionWith(["billing"]),
      list_analytes: () => [AMPHETAMINE, OPIATE],
      list_class_mapping_versions: () => [summaryOf(CURRENT_VIEW), summaryOf(DRAFT_VIEW)],
      get_class_mapping_version: () => CURRENT_VIEW,
    });
    const { router } = renderRoutes(
      [
        { path: "/billing/class-mappings", element: <ClassMappingsPage /> },
        { path: "/billing/class-mappings/:versionId", element: <ClassMappingPage /> },
      ],
      `/billing/class-mappings/${CURRENT_VIEW.versionId}`,
    );
    expect(await screen.findByRole("heading", { level: 2, name: "Version 1" })).toBeTruthy();
    expect(router.state.location.pathname).toBe("/billing/class-mappings");
    expect(router.state.location.search).toBe("?version=1");
    expect(router.state.historyAction).toBe("REPLACE");
  });

  it("says when an old address names no version", async () => {
    mockCommands({ list_class_mapping_versions: () => [summaryOf(CURRENT_VIEW)] });
    renderRoutes(
      [{ path: "/billing/class-mappings/:versionId", element: <ClassMappingPage /> }],
      "/billing/class-mappings/00000000-0000-7000-8000-0000000000ff",
    );
    expect(await screen.findByText("Class mapping version not found")).toBeTruthy();
  });

  it("edits the draft's entries with its revision", async () => {
    const update = vi.fn(() => DRAFT_VIEW);
    const { user } = renderPage(DRAFT_VIEW, { update_class_mapping_draft: update });
    await user.click(await screen.findByRole("button", { name: "Edit entries" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit entries of version 2" });
    await user.click(within(dialog).getByRole("button", { name: "Remove entry 2" }));
    const label = within(dialog).getByRole("textbox", { name: /Class label/ });
    await user.clear(label);
    await user.type(label, "Synthetic class one");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await screen.findByText("Draft saved")).toBeTruthy();
    expect(update).toHaveBeenCalledWith({
      input: {
        versionId: DRAFT_VIEW.versionId,
        expectedRevision: 1,
        entries: [
          { analyteId: AMPHETAMINE.id, classCode: "SYN-C1", classLabel: "Synthetic class one" },
        ],
      },
    });
  });

  it("shows the engine's refusal when the author approves", async () => {
    const approve = vi.fn(() =>
      Promise.reject(
        engineError("gate_blocked", "A billing user other than the author must approve."),
      ),
    );
    const { user } = renderPage(DRAFT_VIEW, { approve_class_mapping_version: approve });
    await user.click(await screen.findByRole("button", { name: "Approve version" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Approve class mapping version 2?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Approve version" }));
    expect(
      await within(dialog).findByText("A billing user other than the author must approve."),
    ).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: { versionId: DRAFT_VIEW.versionId, expectedRevision: 1, selfApproval: false },
    });
  });

  it("asks the author to confirm a self-approval the laboratory allows (ADR 0015)", async () => {
    const message =
      "You wrote this configuration. Your laboratory allows you to approve it yourself: confirm that no second person reviewed it.";
    const approve = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
        }),
      )
      .mockReturnValue({});
    const { user } = renderPage(DRAFT_VIEW, { approve_class_mapping_version: approve });
    await user.click(await screen.findByRole("button", { name: "Approve version" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Approve class mapping version 2?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Approve version" }));
    await user.click(await within(dialog).findByRole("checkbox", { name: /no second person/ }));
    await user.click(within(dialog).getByRole("button", { name: "Approve version" }));
    expect(await screen.findByText("Version 2 approved")).toBeTruthy();
    expect(approve).toHaveBeenLastCalledWith({
      input: { versionId: DRAFT_VIEW.versionId, expectedRevision: 1, selfApproval: true },
    });
  });

  it("lets the laboratory confirm an approved version", async () => {
    const confirm = vi.fn(() => ({}));
    const { user } = renderPage(AWAITING, { confirm_class_mapping_version: confirm });
    expect((await screen.findAllByText("Awaiting confirmation")).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Confirm analyte identities" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("previous current version ends");
    await user.click(within(dialog).getByRole("button", { name: "Confirm analyte identities" }));
    expect(await screen.findByText("Version 2 is now current")).toBeTruthy();
    expect(confirm).toHaveBeenCalledWith({
      input: { versionId: AWAITING.versionId, expectedRevision: 2 },
    });
  });

  it("shows the engine's refusal of a confirmation", async () => {
    const { user } = renderPage(AWAITING, {
      confirm_class_mapping_version: () =>
        Promise.reject(engineError("forbidden", "Confirmation needs approve configuration.")),
    });
    await user.click(await screen.findByRole("button", { name: "Confirm analyte identities" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Confirm analyte identities" }));
    expect(
      await within(dialog).findByText("Confirmation needs approve configuration."),
    ).toBeTruthy();
  });

  it("retires the current version with a reason", async () => {
    const retire = vi.fn(() => ({}));
    const { user } = renderPage(CURRENT_VIEW, { retire_class_mapping_version: retire });
    expect((await screen.findAllByText("Current")).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Retire version" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Replaced by contract",
    );
    await user.click(within(dialog).getByRole("button", { name: "Retire version" }));
    expect(await screen.findByText("Version 1 retired")).toBeTruthy();
    expect(retire).toHaveBeenCalledWith({
      input: {
        versionId: CURRENT_VIEW.versionId,
        expectedRevision: 3,
        reason: "Replaced by contract",
      },
    });
  });

  it("shows the engine's refusal of a retirement", async () => {
    const { user } = renderPage(CURRENT_VIEW, {
      retire_class_mapping_version: () =>
        Promise.reject(engineError("conflict", "The version changed since it was shown.")),
    });
    await user.click(await screen.findByRole("button", { name: "Retire version" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Replaced");
    await user.click(within(dialog).getByRole("button", { name: "Retire version" }));
    expect(await within(dialog).findByText("The version changed since it was shown.")).toBeTruthy();
  });

  it("shows the engine's error", async () => {
    renderPage(DRAFT_VIEW, {
      get_class_mapping_version: () =>
        Promise.reject(engineError("not_found", "No such class mapping version.")),
    });
    expect(await screen.findByText("No such class mapping version.")).toBeTruthy();
  });
});
