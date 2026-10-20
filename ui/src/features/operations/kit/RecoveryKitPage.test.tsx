import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RecoveryKitStatus } from "@/generated/RecoveryKitStatus";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { RecoveryKitPage } from "./RecoveryKitPage";

const NEVER_EXPORTED: RecoveryKitStatus = {
  keyFingerprint: "a1b2-c3d4-e5f6-0718",
  keyState: "ok",
  lastExport: null,
  lastConfirmation: null,
  outdated: true,
  outdatedReasons: ["never_exported"],
};

const EXPORTED: RecoveryKitStatus = {
  ...NEVER_EXPORTED,
  lastExport: {
    kitNo: 3,
    exportedAt: "2026-10-13T14:00:00Z",
    exportedBy: "Test Operator",
    fileName: "vibe-tox-recovery-kit-2026-10-13.txt",
  },
  outdated: false,
  outdatedReasons: [],
};

function renderPage(
  status: () => unknown,
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["backup_restore"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_recovery_kit_status: status,
    get_host_health: () => null,
    ...commands,
  });
  return renderWithProviders(<RecoveryKitPage />);
}

async function openExport(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Export recovery kit" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Export recovery kit" });
  await user.type(
    within(dialog).getByRole("textbox", { name: /^Where the disk-encryption/ }),
    "Sealed envelope in the director's safe",
  );
  await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
  return dialog;
}

describe("RecoveryKitPage (REQ-OPS-002)", () => {
  it("shows a kit that was never exported and exports one with the warning", async () => {
    const exportKit = vi.fn(() => ({ kitNo: 1, fileName: "vibe-tox-recovery-kit-2026-10-13.txt" }));
    const { user, container } = renderPage(() => NEVER_EXPORTED, {
      export_recovery_kit: exportKit,
    });
    expect(await screen.findByText("Key available")).toBeTruthy();
    expect(screen.getByText("The recovery kit is outdated")).toBeTruthy();
    expect(screen.getAllByText("The recovery kit has never been exported.").length).toBe(2);
    expect(screen.queryByRole("button", { name: /Confirm where/ })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    const dialog = await openExport(user);
    expect(
      within(dialog).getByText(
        "Anyone with this file can read your backups. Store it like a disk-encryption recovery key, in two places off this computer.",
      ),
    ).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Choose where to save" }));
    expect(
      await screen.findByText("Recovery kit 1 saved as vibe-tox-recovery-kit-2026-10-13.txt"),
    ).toBeTruthy();
    expect(exportKit).toHaveBeenCalledWith({
      input: {
        diskEncryptionReference: "Sealed envelope in the director's safe",
        password: "synthetic-secret",
      },
    });
  });

  it("keeps the dialog open when the save dialog is cancelled", async () => {
    const { user } = renderPage(() => NEVER_EXPORTED, { export_recovery_kit: () => null });
    const dialog = await openExport(user);
    await user.click(within(dialog).getByRole("button", { name: "Choose where to save" }));
    expect(
      await within(dialog).findByText("No file was chosen, so the recovery kit was not saved."),
    ).toBeTruthy();
  });

  it("labels each outdated reason and shows an unknown key state as sent", async () => {
    renderPage(() => ({
      ...EXPORTED,
      keyState: "sealed",
      outdated: true,
      outdatedReasons: ["key_changed", "targets_changed", "new_reason"],
    }));
    expect(
      await screen.findByText("The installation key changed since the last export."),
    ).toBeTruthy();
    expect(
      screen.getByText("Backup targets were added or retired since the last export."),
    ).toBeTruthy();
    expect(screen.getByText("new_reason")).toBeTruthy();
    expect(screen.getByText("sealed")).toBeTruthy();
  });

  it("shows a wrong password on its field and keeps the dialog open", async () => {
    const wrong = engineError("validation", "The password is incorrect.", {
      fieldErrors: [
        { field: "password", code: "incorrect", message: "The password is incorrect." },
      ],
    });
    const { user } = renderPage(() => NEVER_EXPORTED, {
      export_recovery_kit: () => Promise.reject(wrong),
    });
    const dialog = await openExport(user);
    await user.click(within(dialog).getByRole("button", { name: "Choose where to save" }));
    expect(await within(dialog).findByText("Not exported")).toBeTruthy();
    const password = within(dialog).getByLabelText(/^Your password/);
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(within(dialog).getAllByText("The password is incorrect.").length).toBeGreaterThan(0);
    expect(screen.getByRole("alertdialog", { name: "Export recovery kit" })).toBeTruthy();
    expect(screen.queryByText(/Recovery kit \d+ saved/)).toBeNull();
  });

  it("shows the last export and confirms where the kit is stored", async () => {
    const confirm = vi.fn(() => EXPORTED);
    const { user, container } = renderPage(() => EXPORTED, {
      confirm_recovery_kit_storage: confirm,
    });
    expect(await screen.findByText("vibe-tox-recovery-kit-2026-10-13.txt")).toBeTruthy();
    expect(screen.getByText("Never confirmed")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Confirm where the kit is stored" }));
    const dialog = await screen.findByRole("dialog", { name: "Confirm where kit 3 is stored" });
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Where the kit is stored/ }),
      "Office safe and bank deposit box",
    );
    await user.click(within(dialog).getByRole("button", { name: "Confirm storage" }));
    expect(await screen.findByText("Storage of kit 3 confirmed")).toBeTruthy();
    expect(confirm).toHaveBeenCalledWith({
      input: { kitNo: 3, note: "Office safe and bank deposit box" },
    });
  });

  it("offers no action without backup and restore", async () => {
    renderPage(() => EXPORTED, {}, []);
    expect(await screen.findByText("vibe-tox-recovery-kit-2026-10-13.txt")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Export recovery kit" })).toBeNull();
  });

  it("shows the engine's error with its support reference", async () => {
    renderPage(() => Promise.reject(engineError("internal", "The key store is unavailable.")));
    expect(await screen.findByText("The key store is unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
