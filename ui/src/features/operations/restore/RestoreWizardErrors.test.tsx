import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { RestoreWizard } from "./RestoreWizard";
import { KEY, OTHER_DRIVE, restoreEngine, staged } from "./testData";

function renderWizard() {
  return renderWithProviders(<RestoreWizard onExit={vi.fn()} />);
}

type User = ReturnType<typeof renderWizard>["user"];

async function toSource(user: User) {
  await user.type(await screen.findByRole("textbox", { name: /Installation key/ }), KEY);
  await user.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByRole("heading", { level: 2, name: "Choose the backup folder" });
}

async function toReview(user: User) {
  await toSource(user);
  await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
  await openRow(user, "0.4.0");
  await user.click(screen.getByRole("button", { name: "Check this backup" }));
  await screen.findByRole("heading", { level: 2, name: "Result of the check" });
}

describe("RestoreWizard refusals and cancelled dialogs", () => {
  beforeEach(mockGridLayout);

  it("asks for the key, then shows the engine's refusal of it", async () => {
    const engine = restoreEngine({
      begin_restore: () =>
        Promise.reject(
          engineError("validation", "The installation key has a typing error; check each group."),
        ),
    });
    const { user, container } = renderWizard();
    await user.click(await screen.findByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Enter installation key.")).toBeTruthy();
    expect(engine.begin_restore).not.toHaveBeenCalled();

    const field = screen.getByRole("textbox", { name: /Installation key/ });
    await user.type(field, KEY);
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("The key was not accepted")).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
    expect(
      screen.getByText("The installation key has a typing error; check each group."),
    ).toBeTruthy();
    expect((field as HTMLInputElement).value).toBe(KEY);
    expect(screen.getByRole("heading", { level: 2, name: "Installation key" })).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("leaves the restore once the engine has ended the session", async () => {
    const expired = () =>
      Promise.reject(
        engineError("conflict", "The restore session expired. Enter the installation key again."),
      );
    const engine = restoreEngine({ stage_restore: expired, cancel_restore: expired });
    const onExit = vi.fn();
    const { user } = renderWithProviders(<RestoreWizard onExit={onExit} />);
    await toSource(user);
    await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
    await openRow(user, "0.4.0");
    await user.click(screen.getByRole("button", { name: "Check this backup" }));
    expect(await screen.findByText(/The restore session expired/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cancel restore" }));
    await vi.waitFor(() => expect(onExit).toHaveBeenCalledOnce());
    expect(engine.cancel_restore).toHaveBeenCalledOnce();
  });

  it("stays in the restore when cancelling fails inside the engine", async () => {
    restoreEngine({
      cancel_restore: () =>
        Promise.reject(engineError("internal", "Something went wrong. Try again.")),
    });
    const onExit = vi.fn();
    const { user } = renderWithProviders(<RestoreWizard onExit={onExit} />);
    await toSource(user);
    await user.click(screen.getByRole("button", { name: "Cancel restore" }));
    expect(await screen.findByText("The restore was not cancelled")).toBeTruthy();
    expect(onExit).not.toHaveBeenCalled();
  });

  it("stays on the folder step when the folder dialog is cancelled", async () => {
    restoreEngine({ choose_restore_source: () => null });
    const { user, container } = renderWizard();
    await toSource(user);
    await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
    expect(await screen.findByText(/The folder dialog was cancelled/)).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
    expect(
      screen.getByRole("heading", { level: 2, name: "Choose the backup folder" }),
    ).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("says when the folder holds no backups", async () => {
    restoreEngine({ choose_restore_source: () => ({ installationId: "syn", sets: [] }) });
    const { user } = renderWizard();
    await toSource(user);
    await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
    expect(await screen.findByText("No backups in this folder")).toBeTruthy();
  });

  it("shows the engine's refusal to check a backup and stays on the step", async () => {
    restoreEngine({
      stage_restore: () =>
        Promise.reject(
          engineError(
            "conflict",
            "Restore is only possible on a new installation without user accounts.",
          ),
        ),
    });
    const { user, container } = renderWizard();
    await toSource(user);
    await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
    await openRow(user, "0.4.0");
    await user.click(screen.getByRole("button", { name: "Check this backup" }));
    expect(await screen.findByText("The backup could not be checked")).toBeTruthy();
    expect(
      screen.getByText("Restore is only possible on a new installation without user accounts."),
    ).toBeTruthy();
    expect(
      screen.getByRole("heading", { level: 2, name: "Choose the backup folder" }),
    ).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("offers no restore when the engine says it cannot go ahead", async () => {
    restoreEngine({
      stage_restore: () =>
        staged({
          integrityOk: false,
          chains: [{ stream: "clinical", events: 9, intact: false, headMatchesManifest: false }],
          findings: [{ code: "integrity_failed", count: 1 }],
          canConfirm: false,
        }),
    });
    const { user, container } = renderWizard();
    await toReview(user);
    expect(screen.getByText("Integrity check failed")).toBeTruthy();
    expect(screen.getByText("Broken")).toBeTruthy();
    expect(screen.getByText(/The database copy failed its integrity check: 1/)).toBeTruthy();
    expect(screen.getByText("This backup cannot be restored")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Restore this backup" })).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("holds the other actions while the anchors are compared", async () => {
    let finish: (summary: unknown) => void = () => undefined;
    restoreEngine({
      check_restore_anchors: () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    });
    const { user, container } = renderWizard();
    await toReview(user);
    await user.click(screen.getByRole("button", { name: "Compare with anchors on another drive" }));
    expect(await screen.findByText(/checks the backup again with the anchors/)).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Restore this backup" }).hasAttribute("disabled"),
    ).toBe(true);
    expect(screen.getByRole("button", { name: "Cancel restore" }).hasAttribute("disabled")).toBe(
      true,
    );
    await expectNoA11yViolations(container);
    finish(OTHER_DRIVE);
    expect(await screen.findByRole("heading", { name: "Anchors on the other drive" })).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Restore this backup" }).hasAttribute("disabled"),
    ).toBe(false);
  });

  it("says when the anchor folder dialog is cancelled", async () => {
    restoreEngine({ check_restore_anchors: () => null });
    const { user } = renderWizard();
    await toReview(user);
    await user.click(screen.getByRole("button", { name: "Compare with anchors on another drive" }));
    expect(await screen.findByText(/No anchors were compared\./)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Anchors on the other drive" })).toBeNull();
  });

  it("keeps the confirmation open with the engine's refusal", async () => {
    restoreEngine({
      confirm_restore: () =>
        Promise.reject(
          engineError(
            "conflict",
            "Restore is only possible on a new installation without user accounts.",
          ),
        ),
    });
    const { user } = renderWizard();
    await toReview(user);
    await user.click(screen.getByRole("button", { name: "Restore this backup" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Restore this backup" });
    await user.type(within(dialog).getByRole("textbox", { name: /Confirmation/ }), "RESTORE");
    await user.click(within(dialog).getByRole("button", { name: "Restore and close the app" }));
    expect(await within(dialog).findByText("The restore was not prepared")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Restore prepared" })).toBeNull();
  });
});
