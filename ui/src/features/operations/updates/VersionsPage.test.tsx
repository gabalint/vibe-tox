import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstalledVersionView } from "@/generated/InstalledVersionView";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { backupRun, RUN_ID, sessionWith } from "../testData";
import { VersionsPage } from "./VersionsPage";

/** Principal IDs as the engine sends them, in place of names. */
const OTHER_USER = "00000000-0000-7000-8000-0000000000e2";
const SIGNED_IN = sessionWith([]).userId;

const APPROVAL: UpdateApprovalView = {
  approvalId: "00000000-0000-7000-8000-0000000000f1",
  version: "0.4.0",
  approvedBy: OTHER_USER,
  approvedAt: "2026-10-12T09:00:00Z",
  packageFileName: "vibe-tox-lis-0.4.0-synthetic.msi",
  packageSha256: "ab".repeat(32),
  riskClassText: "Minor",
  preUpdateRunId: RUN_ID,
  withdrawn: null,
  installed: true,
  revision: 1,
};

const PENDING: UpdateApprovalView = {
  ...APPROVAL,
  approvalId: "00000000-0000-7000-8000-0000000000f2",
  version: "0.5.0",
  approvedAt: "2026-10-13T09:00:00Z",
  packageFileName: "vibe-tox-lis-0.5.0-synthetic.msi",
  installed: false,
  revision: 3,
};

const WITHDRAWN: UpdateApprovalView = {
  ...PENDING,
  approvalId: "00000000-0000-7000-8000-0000000000f3",
  version: "0.4.1",
  packageFileName: "vibe-tox-lis-0.4.1-synthetic.msi",
  withdrawn: { by: OTHER_USER, at: "2026-10-12T12:00:00Z" },
};

const PREVIOUS: InstalledVersionView = {
  version: "0.3.0",
  firstStartedAt: "2026-09-01T08:00:00Z",
  lastStartedAt: "2026-10-12T08:00:00Z",
  running: false,
  approval: null,
  acceptance: { by: OTHER_USER, at: "2026-09-01T09:00:00Z" },
  status: "previous",
};

const RUNNING: InstalledVersionView = {
  version: "0.4.0",
  firstStartedAt: "2026-10-12T10:00:00Z",
  lastStartedAt: "2026-10-13T08:00:00Z",
  running: true,
  approval: APPROVAL,
  acceptance: null,
  status: "awaiting_acceptance",
};

const ALL = ["manage_configuration", "approve_configuration", "backup_restore"];

const PATH = "/operations/versions";

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ALL,
  path = PATH,
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_authentication_status: () => ({ method: "password_totp", credentials: [] }),
    list_installed_versions: () => [PREVIOUS, RUNNING],
    list_update_approvals: () => [PENDING, WITHDRAWN, APPROVAL],
    list_backup_runs: () => [backupRun({ kind: "pre_update", updateApprovalEligible: true })],
    get_host_health: () => null,
    list_work_queues: () => [],
    ...commands,
  });
  return renderRoutes(
    [
      { path: PATH, element: <VersionsPage /> },
      { path: `${PATH}/:tab`, element: <VersionsPage /> },
    ],
    path,
  );
}

const panel = (name: string) => screen.getByRole("region", { name });

async function openApprovalWithoutRun(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Record update approval" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
  await user.type(within(dialog).getByRole("textbox", { name: /^Version/ }), "0.5.0");
  await user.type(within(dialog).getByRole("textbox", { name: /^Risk class/ }), "Minor");
  await user.type(within(dialog).getByRole("textbox", { name: /^Approval notes/ }), "Reviewed");
  await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
  return dialog;
}

async function openApproval(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Record update approval" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
  await user.type(within(dialog).getByRole("textbox", { name: /^Version/ }), "0.5.0");
  await user.type(within(dialog).getByRole("textbox", { name: /^Risk class/ }), "Minor");
  await user.click(
    await within(dialog).findByRole("button", { name: /Backup before this update$/ }),
  );
  await user.click(
    await screen.findByRole("option", { name: /Verified on Synthetic network share/ }),
  );
  await user.type(within(dialog).getByRole("textbox", { name: /^Approval notes/ }), "Reviewed");
  await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
  return dialog;
}

describe("VersionsPage (REQ-OPS-006)", () => {
  beforeEach(mockGridLayout);

  it("lists versions, opens the running one and accepts it from the panel", async () => {
    const accept = vi.fn(() => ({
      ...RUNNING,
      status: "accepted",
      acceptance: { by: OTHER_USER, at: "2026-10-13T09:00:00Z" },
    }));
    const { user, container, router } = renderPage({ accept_installed_version: accept });
    expect(await screen.findByText("Awaiting acceptance")).toBeTruthy();
    expect(screen.getByText("Previous")).toBeTruthy();
    expect(screen.queryByText(/Select the running version/)).toBeNull();
    expect(screen.queryByRole("button", { name: /Accept this version/ })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRow(user, "Awaiting acceptance");
    expect(router.state.location.search).toBe("?open=0.4.0");
    const open = panel("Version 0.4.0");
    expect(within(open).getByText("vibe-tox-lis-0.4.0-synthetic.msi")).toBeTruthy();
    await user.click(await within(open).findByRole("button", { name: /Accept this version/ }));
    const dialog = await screen.findByRole("alertdialog", { name: /Accept version 0.4.0/ });
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Smoke test passed");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Accept version" }));
    expect(await screen.findByText("Version 0.4.0 accepted for clinical use")).toBeTruthy();
    expect(accept).toHaveBeenCalledWith({
      input: { version: "0.4.0", note: "Smoke test passed", password: "synthetic-secret" },
    });
  });

  it("accepts the detected running version without an update package or version entry", async () => {
    const accept = vi.fn(() => ({ ...RUNNING, status: "accepted" }));
    const { user } = renderPage({ accept_installed_version: accept });
    expect(await screen.findByRole("heading", { name: "Running version 0.4.0" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Accept running version" }));
    const dialog = await screen.findByRole("alertdialog", { name: /Accept version 0.4.0/ });
    expect(within(dialog).queryByRole("textbox", { name: /^Version/ })).toBeNull();
    expect(within(dialog).queryByText(/update package/)).toBeNull();
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Checked running app");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Accept version" }));
    await waitFor(() =>
      expect(accept).toHaveBeenCalledWith({
        input: { version: "0.4.0", note: "Checked running app", password: "synthetic-secret" },
      }),
    );
  });

  it("opens a version by Enter, and Escape closes the panel with focus on the row", async () => {
    const { user } = renderPage();
    await openRowWithEnter(user, "Previous");
    const open = panel("Version 0.3.0");
    // A version that is not the running one awaiting acceptance has no action.
    expect(within(open).queryByRole("button", { name: /Accept this version/ })).toBeNull();
    await user.click(within(open).getByRole("button", { name: "Next" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: /^Version/ })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement?.closest("[role=row]")?.textContent).toContain("0.4.0"),
    );
  });

  it("opens the version named in the address, and says when none has run", async () => {
    const { router } = renderPage({}, ALL, `${PATH}?open=0.4.0`);
    const open = await screen.findByRole("region", { name: "Version 0.4.0" });
    expect(await within(open).findByRole("button", { name: /Accept this version/ })).toBeTruthy();
    await router.navigate(`${PATH}?open=9.9.9`);
    expect(await screen.findByRole("region", { name: "Record not found" })).toBeTruthy();
  });

  it("offers acceptance in the row's menu only for the running version awaiting it", async () => {
    const { user } = renderPage();
    await screen.findByRole("gridcell", { name: "Awaiting acceptance" });
    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: /^Actions for/ })).toHaveLength(1),
    );
    const menu = await openRowActions(user, "version 0.4.0");
    await user.click(within(menu).getByRole("menuitem", { name: /Accept this version/ }));
    expect(await screen.findByRole("alertdialog", { name: /Accept version 0.4.0/ })).toBeTruthy();
  });

  it("records an approval once the release notes are confirmed", async () => {
    const approve = vi.fn(() => PENDING);
    const { user } = renderPage({ record_update_approval: approve });
    const dialog = await openApproval(user);
    await user.click(within(dialog).getByRole("button", { name: "Choose the update package" }));
    expect(
      await within(dialog).findByText("Confirm that you reviewed the release notes."),
    ).toBeTruthy();
    expect(approve).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the release notes/ }));
    await user.click(within(dialog).getByRole("button", { name: "Choose the update package" }));
    expect(
      await screen.findByText("Update approved: vibe-tox-lis-0.5.0-synthetic.msi"),
    ).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: {
        version: "0.5.0",
        riskClassText: "Minor",
        releaseNotesReviewed: true,
        preUpdateRunId: RUN_ID,
        notes: "Reviewed",
        password: "synthetic-secret",
      },
    });
  });

  it("offers the engine's eligible manual and pre-update backups, never a typed run ID", async () => {
    const scheduled = backupRun({ runId: "00000000-0000-7000-8000-0000000000a1" });
    const failed = backupRun({
      runId: "00000000-0000-7000-8000-0000000000a2",
      kind: "pre_update",
      state: "failed",
    });
    const { user } = renderPage({
      list_backup_runs: () => [
        scheduled,
        failed,
        backupRun({ kind: "pre_update", updateApprovalEligible: true }),
        backupRun({ runId: "manual-run", kind: "manual", updateApprovalEligible: true }),
      ],
    });
    await user.click(await screen.findByRole("button", { name: "Record update approval" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
    expect(within(dialog).queryByRole("textbox", { name: /Backup before this update/ })).toBeNull();
    await user.click(
      await within(dialog).findByRole("button", { name: /Backup before this update$/ }),
    );
    expect(await screen.findAllByRole("option")).toHaveLength(2);
    expect(document.body.textContent).not.toContain(RUN_ID);
  });

  it("loads an eligible backup beyond the first history page", async () => {
    const older = backupRun({ kind: "manual", updateApprovalEligible: true });
    const first = Array.from({ length: 101 }, (_, index) => backupRun({ runId: `run-${index}` }));
    const list = vi.fn((args: unknown) =>
      (args as { input: { beforeRunId: string | null } }).input.beforeRunId ? [older] : first,
    );
    const { user } = renderPage({ list_backup_runs: list });
    await user.click(await screen.findByRole("button", { name: "Record update approval" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
    await user.click(await within(dialog).findByRole("button", { name: "Load older backup runs" }));
    await user.click(
      await within(dialog).findByRole("button", { name: /Backup before this update$/ }),
    );
    expect(await screen.findAllByRole("option")).toHaveLength(1);
    expect(list).toHaveBeenLastCalledWith({
      input: { limit: 100, beforeRequestedAt: null, beforeRunId: "run-99" },
    });
  });

  it("names only retained verified destinations in backup choices", async () => {
    const run = backupRun({ kind: "manual", updateApprovalEligible: true });
    const verified = run.copies[0];
    if (!verified) throw new Error("The synthetic run needs a verified copy.");
    run.copies.push(
      { ...verified, targetId: "failed", targetLabel: "Failed target", state: "failed" },
      { ...verified, targetId: "pruned", targetLabel: "Pruned target", pruned: true },
    );
    const { user } = renderPage({ list_backup_runs: () => [run] });
    await user.click(await screen.findByRole("button", { name: "Record update approval" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
    await user.click(
      await within(dialog).findByRole("button", { name: /Backup before this update$/ }),
    );
    const option = await screen.findByRole("option", {
      name: /Verified on Synthetic network share/,
    });
    expect(option.textContent).not.toMatch(/Failed target|Pruned target/);
  });

  it("requests a backup inside the approval form and refreshes its choices", async () => {
    let runs: ReturnType<typeof backupRun>[] = [];
    const request = vi.fn(() => {
      const run = backupRun({ kind: "pre_update", updateApprovalEligible: true });
      runs = [run];
      return run;
    });
    const { user } = renderPage({ list_backup_runs: () => runs, request_backup: request });
    const dialog = await openApprovalWithoutRun(user);
    expect(await within(dialog).findByText(/No eligible backup/)).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Request pre-update backup" }));
    expect(await within(dialog).findByText(/Pre-update backup requested/)).toBeTruthy();
    await waitFor(() => expect(within(dialog).queryByText(/No eligible backup/)).toBeNull());
    await user.click(within(dialog).getByRole("button", { name: /Backup before this update$/ }));
    expect(
      await screen.findByRole("option", { name: /Verified on Synthetic network share/ }),
    ).toBeTruthy();
    expect(request).toHaveBeenCalledWith({ input: { kind: "pre_update" } });
    await user.keyboard("{Escape}");
    expect(
      (within(dialog).getByRole("textbox", { name: /^Version/ }) as HTMLInputElement).value,
    ).toBe("0.5.0");
  });

  it("chooses the run with a picker when many verified backups exist", async () => {
    const runs = Array.from({ length: 16 }, (_, index) =>
      backupRun({
        runId: `00000000-0000-7000-8000-0000000001${String(index).padStart(2, "0")}`,
        kind: "pre_update",
        updateApprovalEligible: true,
      }),
    );
    const { user } = renderPage({ list_backup_runs: () => runs });
    await user.click(await screen.findByRole("button", { name: "Record update approval" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Record update approval" });
    expect(
      await within(dialog).findByRole("combobox", { name: /Backup before this update/ }),
    ).toBeTruthy();
  });

  it("asks for a pre-update backup when none is verified, and does not send an approval", async () => {
    const approve = vi.fn(() => PENDING);
    const { user } = renderPage({ list_backup_runs: () => [], record_update_approval: approve });
    const dialog = await openApprovalWithoutRun(user);
    expect(
      await within(dialog).findByText(/No eligible backup in the loaded history/),
    ).toBeTruthy();
    await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the release notes/ }));
    await user.click(within(dialog).getByRole("button", { name: "Choose the update package" }));
    expect(await within(dialog).findByText("Choose backup before this update.")).toBeTruthy();
    expect(approve).not.toHaveBeenCalled();
  });

  it("keeps the approval dialog open when no package is chosen", async () => {
    const { user } = renderPage({ record_update_approval: () => null });
    const dialog = await openApproval(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the release notes/ }));
    await user.click(within(dialog).getByRole("button", { name: "Choose the update package" }));
    expect(
      await within(dialog).findByText("No package was chosen, so no approval was recorded."),
    ).toBeTruthy();
  });

  it("keeps a wrong password for an approval in the dialog, on its field", async () => {
    const wrong = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [
        { field: "password", code: "incorrect", message: "The password is incorrect." },
      ],
    });
    const { user } = renderPage({ record_update_approval: () => Promise.reject(wrong) });
    const dialog = await openApproval(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the release notes/ }));
    await user.click(within(dialog).getByRole("button", { name: "Choose the update package" }));
    expect(await within(dialog).findByText("Not approved")).toBeTruthy();
    expect(within(dialog).getByText("The password is incorrect.")).toBeTruthy();
    const password = within(dialog).getByLabelText(/^Your password/);
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(screen.queryByText(/Update approved/)).toBeNull();
  });

  it("lists approvals on their tab and withdraws one whose version is not installed", async () => {
    const withdraw = vi.fn(() => ({ ...PENDING, withdrawn: WITHDRAWN.withdrawn, revision: 4 }));
    const { user, router } = renderPage({ withdraw_update_approval: withdraw });
    await user.click(await screen.findByRole("tab", { name: "Update approvals" }));
    await waitFor(() => expect(router.state.location.pathname).toBe(`${PATH}/approvals`));
    expect(await screen.findByText("Not yet installed")).toBeTruthy();
    expect(screen.getByText("Withdrawn")).toBeTruthy();
    expect(screen.getByText("Installed")).toBeTruthy();
    expect(screen.queryByText(/Select an approval/)).toBeNull();
    // Only the approval whose version is not installed has an action, in its row and panel.
    expect(screen.getAllByRole("button", { name: /^Actions for/ })).toHaveLength(1);
    await openRow(user, "vibe-tox-lis-0.4.1-synthetic.msi");
    expect(
      within(panel("Approval of version 0.4.1")).queryByRole("button", {
        name: "Withdraw approval",
      }),
    ).toBeNull();
    await openRow(user, "vibe-tox-lis-0.5.0-synthetic.msi");
    await user.click(
      within(panel("Approval of version 0.5.0")).getByRole("button", { name: "Withdraw approval" }),
    );
    const dialog = await screen.findByRole("alertdialog", { name: /Withdraw the approval/ });
    await user.click(within(dialog).getByRole("button", { name: "Withdraw approval" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(withdraw).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Vendor recall");
    await user.click(within(dialog).getByRole("button", { name: "Withdraw approval" }));
    expect(await screen.findByText("Approval of version 0.5.0 withdrawn")).toBeTruthy();
    expect(withdraw).toHaveBeenCalledWith({
      input: { approvalId: PENDING.approvalId, expectedRevision: 3, reason: "Vendor recall" },
    });
  });

  it("names the people on an approval and a version without showing their IDs", async () => {
    renderPage(
      {
        list_update_approvals: () => [
          { ...WITHDRAWN, withdrawn: { by: SIGNED_IN, at: "2026-10-12T12:00:00Z" } },
        ],
      },
      ALL,
      `${PATH}/approvals?open=${WITHDRAWN.approvalId}`,
    );
    const approval = within(
      await screen.findByRole("region", { name: "Approval of version 0.4.1" }),
    );
    expect(approval.getByText(/by another user/)).toBeTruthy();
    expect(approval.getByText(/by Test Operator/)).toBeTruthy();
    expect(document.body.textContent).not.toContain(OTHER_USER);
    expect(document.body.textContent).not.toContain(SIGNED_IN);
  });

  it("shows who accepted a version as another user", async () => {
    renderPage({}, ALL, `${PATH}?open=0.3.0`);
    const version = within(await screen.findByRole("region", { name: "Version 0.3.0" }));
    expect(version.getByText(/by another user/)).toBeTruthy();
    expect(document.body.textContent).not.toContain(OTHER_USER);
  });

  it("withdraws from the row's menu, and checks the approvals tab for accessibility", async () => {
    const { user, container } = renderPage({}, ALL, `${PATH}/approvals`);
    await screen.findByRole("gridcell", { name: "vibe-tox-lis-0.5.0-synthetic.msi" });
    await expectNoA11yViolations(container, { wholePage: true });
    const menu = await openRowActions(user, "approval of version 0.5.0");
    await user.click(within(menu).getByRole("menuitem", { name: "Withdraw approval" }));
    expect(await screen.findByRole("alertdialog", { name: /Withdraw the approval/ })).toBeTruthy();
  });

  it("keeps the engine's refusal of a withdrawal in the dialog", async () => {
    const { user } = renderPage(
      {
        withdraw_update_approval: () =>
          Promise.reject(engineError("conflict", "The version is already installed.")),
      },
      ALL,
      `${PATH}/approvals?open=${PENDING.approvalId}`,
    );
    const open = await screen.findByRole("region", { name: "Approval of version 0.5.0" });
    await user.click(within(open).getByRole("button", { name: "Withdraw approval" }));
    const dialog = await screen.findByRole("alertdialog", { name: /Withdraw the approval/ });
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Vendor recall");
    await user.click(within(dialog).getByRole("button", { name: "Withdraw approval" }));
    expect(await within(dialog).findByText("The version is already installed.")).toBeTruthy();
  });

  it("shows a wrong password on its field when accepting", async () => {
    const wrong = engineError("validation", "The password is incorrect.", {
      fieldErrors: [
        { field: "password", code: "incorrect", message: "The password is incorrect." },
      ],
    });
    const { user } = renderPage({ accept_installed_version: () => Promise.reject(wrong) });
    await openRow(user, "Awaiting acceptance");
    await user.click(await screen.findByRole("button", { name: /Accept this version/ }));
    const dialog = await screen.findByRole("alertdialog", { name: /Accept version 0.4.0/ });
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Checked");
    await user.type(within(dialog).getByLabelText(/^Your password/), "wrong-password");
    await user.click(within(dialog).getByRole("button", { name: "Accept version" }));
    expect(await within(dialog).findByText("Not accepted")).toBeTruthy();
    const password = within(dialog).getByLabelText(/^Your password/);
    expect(password.getAttribute("aria-invalid")).toBe("true");
  });

  it("requests a pre-update backup without claiming it succeeded", async () => {
    const request = vi.fn(() => backupRun({ kind: "pre_update", state: "requested" }));
    const { user } = renderPage({ request_backup: request }, ALL, `${PATH}/approvals`);
    await user.click(await screen.findByRole("button", { name: "Request pre-update backup" }));
    expect(await screen.findByText(/Pre-update backup requested/)).toBeTruthy();
    expect(request).toHaveBeenCalledWith({ input: { kind: "pre_update" } });
  });

  it("shows the engine's refusal of a backup request", async () => {
    const { user } = renderPage(
      {
        request_backup: () =>
          Promise.reject(engineError("conflict", "A backup is already queued.")),
      },
      ALL,
      `${PATH}/approvals`,
    );
    await user.click(await screen.findByRole("button", { name: "Request pre-update backup" }));
    expect(await screen.findByText("A backup is already queued.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("offers no actions without the capabilities, and no approvals tab", async () => {
    const { user } = renderPage({}, []);
    expect(await screen.findByText("Awaiting acceptance")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record update approval" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Accept running version" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Request pre-update backup" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Update approvals" })).toBeNull();
    await openRow(user, "Awaiting acceptance");
    const open = panel("Version 0.4.0");
    expect(within(open).queryByRole("button", { name: /Accept this version/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Actions for/ })).toBeNull();
  });

  it("explains an empty history on each tab", async () => {
    const { container, user } = renderPage({
      list_installed_versions: () => [],
      list_update_approvals: () => [],
    });
    expect(await screen.findByText("No versions recorded")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("tab", { name: "Update approvals" }));
    expect(await screen.findByText("No update approvals")).toBeTruthy();
  });

  it("shows the engine's error when the versions cannot be read", async () => {
    renderPage({
      list_installed_versions: () =>
        Promise.reject(engineError("internal", "The version history is unavailable.")),
    });
    expect(screen.getByRole("heading", { level: 1, name: "Versions and updates" })).toBeTruthy();
    expect(await screen.findByText("The version history is unavailable.")).toBeTruthy();
  });
});
