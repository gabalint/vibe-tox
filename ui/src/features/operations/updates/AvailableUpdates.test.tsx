import { screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import type { UpdateFeedView } from "@/generated/UpdateFeedView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { backupRun, RUN_ID, sessionWith } from "../testData";
import { VersionsPage } from "./VersionsPage";

const PATH = "/operations/versions";
const ALL = ["manage_configuration", "approve_configuration", "backup_restore"];
const RUNNING = "0.1.2-nightly.20261027.19";

const RELEASE: AvailableBuild = {
  channel: "release",
  version: "0.2.0",
  publishedAt: "2026-10-27T18:00:00Z",
  changes: ["Show invalid field names in red (ticket #4359)"],
  newer: true,
};

const NEWER: AvailableBuild = {
  channel: "nightly",
  version: "0.1.2-nightly.20261028.20",
  publishedAt: "2026-10-28T03:00:00Z",
  changes: ["Show pop-up notices in the top-right corner (ticket #4358)"],
  commit: "f3c8d1e0123456789abcdef0123456789abcdef0",
  newer: true,
};

const OLDER: AvailableBuild = {
  ...NEWER,
  version: "0.1.2-nightly.20261026.18",
  publishedAt: "2026-10-26T13:56:00Z",
  changes: [],
  newer: false,
};

const feed = (builds: AvailableBuild[], extra: Partial<UpdateFeedView> = {}): UpdateFeedView => ({
  runningVersion: RUNNING,
  configured: true,
  offered: true,
  builds,
  ...extra,
});

const APPROVED: UpdateApprovalView = {
  approvalId: "00000000-0000-7000-8000-0000000000f4",
  version: "0.2.0",
  approvedBy: sessionWith([]).userId,
  approvedAt: "2026-10-27T19:00:00Z",
  packageFileName: "vibe-tox-lis_0.2.0_linux-amd64.AppImage",
  packageSha256: "cd".repeat(32),
  riskClassText: "Development build",
  preUpdateRunId: RUN_ID,
  withdrawn: null,
  installed: false,
  revision: 1,
};

function renderTab(commands: Record<string, (args: unknown) => unknown> = {}, capabilities = ALL) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_authentication_status: () => ({ method: "password_totp", credentials: [] }),
    list_installed_versions: () => [],
    list_update_approvals: () => [],
    list_backup_runs: () => [backupRun({ kind: "pre_update", updateApprovalEligible: true })],
    get_host_health: () => null,
    list_work_queues: () => [],
    check_release_update: () => feed([RELEASE]),
    list_nightly_builds: () => feed([NEWER, OLDER]),
    ...commands,
  });
  return renderRoutes(
    [
      { path: PATH, element: <VersionsPage /> },
      { path: `${PATH}/:tab`, element: <VersionsPage /> },
    ],
    `${PATH}/available`,
  );
}

const downloaded = {
  version: "0.2.0",
  packageFileName: APPROVED.packageFileName,
};

async function reviewApproval(user: UserEvent, dialog: HTMLElement) {
  await user.type(within(dialog).getByRole("textbox", { name: /^Risk class/ }), "Development");
  await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the release notes/ }));
  await user.click(
    await within(dialog).findByRole("button", { name: /Backup before this update$/ }),
  );
  await user.click(await screen.findByRole("option", { name: /Verified on/ }));
  await user.type(within(dialog).getByRole("textbox", { name: /^Approval notes/ }), "Tested");
}

describe("Available updates (ADR 0020, REQ-OPS-006)", () => {
  beforeEach(mockGridLayout);

  it.each(["password_totp", "security_key"])(
    "downloads, approves and installs with %s",
    async (method) => {
      const download = vi.fn(() => downloaded);
      const approve = vi.fn(() => APPROVED);
      const keyAction = vi.fn(() => APPROVED);
      const install = vi.fn(() => null);
      const { user, container } = renderTab({
        download_update: download,
        approve_downloaded_update: approve,
        get_authentication_status: () => ({
          method,
          credentials: [{ credentialId: "synthetic-key", label: "Everyday", revision: 1 }],
        }),
        security_key_action: keyAction,
        install_update: install,
      });
      const release = await screen.findByRole("region", { name: "Official release" });
      expect(await within(release).findByText(/Version 0.2.0 is available/)).toBeTruthy();
      expect(within(release).getByText(RELEASE.changes[0] as string)).toBeTruthy();
      await expectNoA11yViolations(container, { wholePage: true });
      await user.click(within(release).getByRole("button", { name: "Download version 0.2.0" }));
      expect(download).toHaveBeenCalledWith({ input: { channel: "release", version: "0.2.0" } });
      expect(await within(release).findByText(/its signature is verified/)).toBeTruthy();
      await user.click(within(release).getByRole("button", { name: "Approve and install" }));
      const dialog = await screen.findByRole("alertdialog", { name: "Approve version 0.2.0" });
      // The version is the downloaded package's, never typed.
      expect(within(dialog).queryByRole("textbox", { name: /^Version/ })).toBeNull();
      await reviewApproval(user, dialog);
      if (method === "password_totp") {
        await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
      } else {
        expect(await within(dialog).findByRole("button", { name: /Security key/ })).toBeTruthy();
        expect(within(dialog).queryByLabelText(/^Your password/)).toBeNull();
      }
      await user.click(within(dialog).getByRole("button", { name: "Approve and install" }));
      await waitFor(() => expect(install).toHaveBeenCalledWith({ input: { version: "0.2.0" } }));
      const input = {
        version: "0.2.0",
        riskClassText: "Development",
        releaseNotesReviewed: true,
        preUpdateRunId: RUN_ID,
        notes: "Tested",
        password: method === "password_totp" ? "synthetic-secret" : "",
      };
      if (method === "security_key") {
        expect(keyAction).toHaveBeenCalledWith({
          input: {
            credentialId: "synthetic-key",
            action: { command: "approve_downloaded_update", input },
          },
        });
        expect(approve).not.toHaveBeenCalled();
      } else {
        expect(approve).toHaveBeenCalledWith({ input });
        expect(keyAction).not.toHaveBeenCalled();
      }
      expect(await screen.findByText(/The app closes and starts version 0.2.0/)).toBeTruthy();
    },
  );

  it.each([
    "Security key operation cancelled.",
    "The security key PIN was not accepted. Check it before trying again.",
  ])("keeps the approval open and does not install when %s", async (message) => {
    const install = vi.fn();
    const approve = vi.fn();
    const { user } = renderTab({
      get_authentication_status: () => ({
        method: "security_key",
        credentials: [{ credentialId: "synthetic-key", label: "Everyday", revision: 1 }],
      }),
      download_update: () => downloaded,
      approve_downloaded_update: approve,
      security_key_action: () => Promise.reject(engineError("validation", message)),
      install_update: install,
    });
    const release = await screen.findByRole("region", { name: "Official release" });
    await user.click(await within(release).findByRole("button", { name: /^Download/ }));
    await user.click(await within(release).findByRole("button", { name: "Approve and install" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Approve version 0.2.0" });
    await reviewApproval(user, dialog);
    await user.click(within(dialog).getByRole("button", { name: "Approve and install" }));
    expect(await within(dialog).findByText(message)).toBeTruthy();
    expect(within(dialog).getByText("corr-test-0001", { exact: false })).toBeTruthy();
    expect(within(dialog).getByRole("textbox", { name: /^Approval notes/ })).toHaveProperty(
      "value",
      "Tested",
    );
    expect(approve).not.toHaveBeenCalled();
    expect(install).not.toHaveBeenCalled();
  });

  it("installs an already approved download without a second approval", async () => {
    const install = vi.fn(() => null);
    const { user } = renderTab({
      list_update_approvals: () => [APPROVED],
      download_update: () => downloaded,
      install_update: install,
    });
    const release = await screen.findByRole("region", { name: "Official release" });
    await user.click(await within(release).findByRole("button", { name: /^Download/ }));
    await user.click(
      await within(release).findByRole("button", { name: "Install approved update" }),
    );
    await waitFor(() => expect(install).toHaveBeenCalledWith({ input: { version: "0.2.0" } }));
  });

  it("shows a refused download or installation with the engine's message", async () => {
    const { user } = renderTab({
      download_update: () => {
        throw engineError("unavailable", "The update package failed its signature check.");
      },
    });
    const release = await screen.findByRole("region", { name: "Official release" });
    await user.click(await within(release).findByRole("button", { name: /^Download/ }));
    expect(await within(release).findByText(/failed its signature check/)).toBeTruthy();
    expect(within(release).getByText("Not downloaded")).toBeTruthy();
  });

  it("lists nightly builds for a nightly build; only a newer one can be installed", async () => {
    const { user } = renderTab();
    const nightly = await screen.findByRole("region", {
      name: "Nightly builds for internal testing",
    });
    await openRow(user, OLDER.version);
    const older = await screen.findByRole("region", { name: `Version ${OLDER.version}` });
    expect(within(older).getByText("No change summary was published.")).toBeTruthy();
    expect(within(older).queryByRole("button", { name: /^Download/ })).toBeNull();
    expect(within(older).getByText(/Only a version newer/)).toBeTruthy();
    await openRow(user, NEWER.version);
    const newer = await screen.findByRole("region", { name: `Version ${NEWER.version}` });
    expect(within(newer).getByText(NEWER.changes[0] as string)).toBeTruthy();
    expect(within(newer).getByText("f3c8d1e01234")).toBeTruthy();
    expect(
      within(newer).getByRole("button", { name: `Download version ${NEWER.version}` }),
    ).toBeTruthy();
    expect(nightly).toBeTruthy();
  });

  it("hides nightly builds from a release build and actions from people who cannot approve", async () => {
    renderTab(
      {
        check_release_update: () => feed([{ ...RELEASE, newer: false, version: "0.1.1" }]),
        list_nightly_builds: () => feed([], { offered: false }),
      },
      ["approve_configuration"],
    );
    expect(await screen.findByText(/The newest official release is 0.1.1/)).toBeTruthy();
    expect(screen.queryByRole("region", { name: /Nightly builds/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Download/ })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Update approvals" })).toBeNull();
  });

  it("says when this build cannot install updates", async () => {
    renderTab({
      check_release_update: () => feed([], { configured: false }),
      list_nightly_builds: () => feed([], { configured: false }),
    });
    expect(await screen.findByText("Updates are not available in this build")).toBeTruthy();
    expect(screen.queryByRole("region", { name: /Nightly builds/ })).toBeNull();
  });
});
