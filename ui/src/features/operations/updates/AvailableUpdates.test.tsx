import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import type { UpdateFeedView } from "@/generated/UpdateFeedView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { backupRun, RUN_ID, sessionWith, target } from "../testData";
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
    list_backup_targets: () => [target()],
    request_backup: () => backupRun({ kind: "pre_update", state: "requested" }),
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

const card = () => screen.findByRole("region", { name: "Available versions" });
const status = (name: string) => screen.findByRole("region", { name });

/** Shows the Release side of the channel switch, offered while a nightly build runs. */
async function showRelease(user: ReturnType<typeof renderTab>["user"]) {
  await user.click(await screen.findByRole("radio", { name: /^Release/ }));
}

const KEY = { credentialId: "synthetic-key", label: "Everyday", revision: 1 };

async function confirmUpdate(
  user: ReturnType<typeof renderTab>["user"],
  name: RegExp | string,
  method: "password_totp" | "security_key" = "password_totp",
) {
  await user.click(within(await card()).getByRole("button", { name }));
  const dialog = await screen.findByRole("dialog", { name: /^Update to/ });
  await user.click(within(dialog).getByRole("checkbox", { name: /reviewed the changes/ }));
  if (method === "password_totp") {
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
  } else {
    expect(await within(dialog).findByRole("button", { name: /Security key/ })).toBeTruthy();
  }
  await user.click(within(dialog).getByRole("button", { name: "Update now" }));
  return dialog;
}

describe("Available updates (ADR 0020, REQ-OPS-006)", () => {
  beforeEach(mockGridLayout);

  it("updates in one step: download, backup, approval of that package, install", async () => {
    const download = vi.fn(() => downloaded);
    const backup = vi.fn(() => backupRun({ kind: "pre_update", state: "requested" }));
    const approve = vi.fn(() => APPROVED);
    const install = vi.fn(() => null);
    const { user, container } = renderTab({
      download_update: download,
      request_backup: backup,
      approve_downloaded_update: approve,
      install_update: install,
    });
    await showRelease(user);
    const shown = await card();
    // An official release shows its version and release date: no build details or change lines.
    expect(within(shown).getByText(RELEASE.version)).toBeTruthy();
    expect(within(shown).getByText(/2026-10-27/)).toBeTruthy();
    expect(within(shown).queryByText(RELEASE.changes[0] as string)).toBeNull();
    expect(within(shown).queryByRole("heading", { name: "What’s new" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    const dialog = await confirmUpdate(user, "Update to version 0.2.0");
    // No backup to pick and no version to type: the app takes both from this update.
    expect(within(dialog).queryByRole("button", { name: /Backup before this update/ })).toBeNull();
    await waitFor(() => expect(install).toHaveBeenCalledWith({ input: { version: "0.2.0" } }), {
      timeout: 4000,
    });
    expect(download).toHaveBeenCalledWith({ input: { channel: "release", version: "0.2.0" } });
    expect(backup).toHaveBeenCalledWith({ input: { kind: "pre_update" } });
    expect(approve).toHaveBeenCalledWith({
      input: expect.objectContaining({
        version: "0.2.0",
        releaseNotesReviewed: true,
        preUpdateRunId: RUN_ID,
        riskClassText: "Not published (development build)",
        password: "synthetic-secret",
      }),
    });
    expect(await screen.findByText(/The app closes and starts version 0.2.0/)).toBeTruthy();
  });

  it("approves the downloaded package with a security key in the same step", async () => {
    const approve = vi.fn();
    const keyAction = vi.fn(() => APPROVED);
    const install = vi.fn(() => null);
    const { user } = renderTab({
      get_authentication_status: () => ({ method: "security_key", credentials: [KEY] }),
      download_update: () => downloaded,
      approve_downloaded_update: approve,
      security_key_action: keyAction,
      install_update: install,
    });
    await showRelease(user);
    const dialog = await confirmUpdate(user, "Update to version 0.2.0", "security_key");
    await waitFor(() => expect(install).toHaveBeenCalledWith({ input: { version: "0.2.0" } }), {
      timeout: 4000,
    });
    expect(keyAction).toHaveBeenCalledWith({
      input: {
        credentialId: KEY.credentialId,
        action: {
          command: "approve_downloaded_update",
          input: expect.objectContaining({
            version: "0.2.0",
            preUpdateRunId: RUN_ID,
            password: "",
          }),
        },
      },
    });
    expect(approve).not.toHaveBeenCalled();
    expect(within(dialog).queryByLabelText(/^Your password/)).toBeNull();
  });

  it.each([
    "Security key operation cancelled.",
    "The security key PIN was not accepted. Check it before trying again.",
  ])("keeps the dialog open and does not install when %s", async (message) => {
    const install = vi.fn();
    const { user } = renderTab({
      get_authentication_status: () => ({ method: "security_key", credentials: [KEY] }),
      download_update: () => downloaded,
      security_key_action: () => Promise.reject(engineError("validation", message)),
      install_update: install,
    });
    await showRelease(user);
    const dialog = await confirmUpdate(user, "Update to version 0.2.0", "security_key");
    expect(await within(dialog).findByText(message, {}, { timeout: 4000 })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Update now" })).toBeTruthy();
    expect(install).not.toHaveBeenCalled();
  });

  it("installs an already approved package without a second backup or approval", async () => {
    const backup = vi.fn();
    const install = vi.fn(() => null);
    const { user } = renderTab({
      list_update_approvals: () => [APPROVED],
      download_update: () => downloaded,
      request_backup: backup,
      install_update: install,
    });
    await showRelease(user);
    await confirmUpdate(user, /^Update to/);
    await waitFor(() => expect(install).toHaveBeenCalledWith({ input: { version: "0.2.0" } }));
    expect(backup).not.toHaveBeenCalled();
  });

  it("stops at a failed step with the engine's message and lets the person try again", async () => {
    const { user } = renderTab({
      download_update: () => {
        throw engineError("unavailable", "The update package failed its signature check.");
      },
    });
    await showRelease(user);
    const dialog = await confirmUpdate(user, /^Update to/);
    expect(await within(dialog).findByText(/failed its signature check/)).toBeTruthy();
    expect(within(dialog).getByText("Not updated")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Update now" })).toBeTruthy();
  });

  it("asks for a backup folder before updating when the installation has none", async () => {
    const download = vi.fn();
    const { user } = renderTab({ list_backup_targets: () => [], download_update: download });
    const dialog = await confirmUpdate(user, /^Update to/);
    expect(await within(dialog).findByText("Choose where backups are saved")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Add backup folder" })).toBeTruthy();
    expect(download).not.toHaveBeenCalled();
  });

  it("shows only the newest version, with three changes until asked", async () => {
    const changes = Array.from({ length: 15 }, (_, index) => `Synthetic change ${index + 1}`);
    const NEWEST = { ...NEWER, version: "0.1.2-nightly.20261029.21", changes };
    const { user } = renderTab({ list_nightly_builds: () => feed([NEWEST, NEWER, OLDER]) });
    const versions = await within(await card()).findAllByRole("article");
    expect(versions.map((version) => version.getAttribute("aria-label"))).toEqual([
      `Version ${NEWEST.version}`,
    ]);
    const newest = versions[0] as HTMLElement;
    expect(within(newest).getByRole("button", { name: `Update to version ${NEWEST.version}` }));
    expect(within(newest).getAllByRole("listitem")).toHaveLength(3);
    await user.click(within(newest).getByRole("button", { name: "Show all 15 changes" }));
    expect(within(newest).getAllByRole("listitem")).toHaveLength(15);
    await user.click(within(newest).getByRole("button", { name: "Show fewer changes" }));
    expect(within(newest).getAllByRole("listitem")).toHaveLength(3);
  });

  it("shows the newest nightly build first, with its build details", async () => {
    renderTab();
    const summary = await status("An update is available");
    expect(within(summary).getByText(RUNNING)).toBeTruthy();
    const shown = await card();
    // A nightly build runs, so the nightly channel is shown first.
    expect(
      within(shown)
        .getByRole("radio", { name: /^Nightly/ })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(within(shown).getByText(NEWER.changes[0] as string)).toBeTruthy();
    expect(within(shown).getByText(/commit f3c8d1e01234/)).toBeTruthy();
    expect(
      within(shown).getByRole("button", { name: `Update to version ${NEWER.version}` }),
    ).toBeTruthy();
    expect(within(shown).queryByText(new RegExp(OLDER.version))).toBeNull();
  });

  it("says when the running build is already the newest of its channel", async () => {
    renderTab({
      check_release_update: () => feed([{ ...RELEASE, newer: false }]),
      list_nightly_builds: () => feed([{ ...NEWER, version: RUNNING, newer: false }]),
    });
    expect(await status("This app is up to date")).toBeTruthy();
    expect(await within(await card()).findByText("Running")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Update to/ })).toBeNull();
  });

  it("says when updates could not be checked", async () => {
    renderTab({
      check_release_update: () => {
        throw engineError("unavailable", "The release repository could not be reached.");
      },
      list_nightly_builds: () => {
        throw engineError("unavailable", "The release repository could not be reached.");
      },
    });
    expect(await status("Updates could not be checked")).toBeTruthy();
    expect(await within(await card()).findByText(/could not be reached/)).toBeTruthy();
  });

  it("refreshes both channels from one Refresh button", async () => {
    const release = vi.fn(() => feed([RELEASE]));
    const nightly = vi.fn(() => feed([NEWER, OLDER]));
    const { user } = renderTab({ check_release_update: release, list_nightly_builds: nightly });
    await user.click(
      within(await status("An update is available")).getByRole("button", { name: "Refresh" }),
    );
    await waitFor(() => expect(release).toHaveBeenCalledTimes(2));
    expect(nightly).toHaveBeenCalledTimes(2);
  });

  it("hides the nightly channel from a release build and actions from people who cannot approve", async () => {
    renderTab(
      {
        check_release_update: () =>
          feed([{ ...RELEASE, newer: false, version: "0.1.1" }], { runningVersion: "0.2.1" }),
        list_nightly_builds: () => feed([], { offered: false, runningVersion: "0.2.1" }),
      },
      ["approve_configuration"],
    );
    expect(within(await status("This app is up to date")).getByText("0.2.1")).toBeTruthy();
    expect(screen.queryByRole("radio", { name: /^Nightly/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /^Update to/ })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Update approvals" })).toBeNull();
  });

  it("says when this build cannot install updates", async () => {
    renderTab({
      check_release_update: () => feed([], { configured: false }),
      list_nightly_builds: () => feed([], { configured: false }),
    });
    expect(await screen.findByText("Updates are not available in this build")).toBeTruthy();
    expect(screen.queryByRole("radio", { name: /^Nightly/ })).toBeNull();
  });
});
