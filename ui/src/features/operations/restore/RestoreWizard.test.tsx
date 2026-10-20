import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { SET_ID } from "../testData";
import { RestoreWizard } from "./RestoreWizard";
import { FINGERPRINT, KEY, OTHER_DRIVE, restoreEngine, SESSION_ID, staged } from "./testData";

function renderWizard() {
  const onExit = vi.fn();
  return { ...renderWithProviders(<RestoreWizard onExit={onExit} />), onExit };
}

type User = ReturnType<typeof renderWizard>["user"];

async function enterKey(user: User) {
  await user.type(await screen.findByRole("textbox", { name: /Installation key/ }), KEY);
  await user.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByRole("heading", { level: 2, name: "Choose the backup folder" });
}

async function chooseFolder(user: User) {
  await user.click(screen.getByRole("button", { name: "Choose the backup folder" }));
  await screen.findByRole("grid", { name: "Backups in the folder" });
}

async function checkBackup(user: User) {
  await openRow(user, "0.4.0");
  await user.click(screen.getByRole("button", { name: "Check this backup" }));
  await screen.findByRole("heading", { level: 2, name: "Result of the check" });
}

describe("RestoreWizard (M4 design brief §3.7)", () => {
  beforeEach(mockGridLayout);

  it("restores a checked backup through to the closing screen", async () => {
    let finishStage: (view: unknown) => void = () => undefined;
    const engine = restoreEngine({
      stage_restore: () =>
        new Promise((resolve) => {
          finishStage = resolve;
        }),
      check_restore_anchors: () => OTHER_DRIVE,
    });
    const { user, container } = renderWizard();
    expect(await screen.findByRole("heading", { name: "Restore from a backup" })).toBeTruthy();
    await expectNoA11yViolations(container);

    await enterKey(user);
    expect(engine.begin_restore).toHaveBeenCalledWith({ input: { installationKey: KEY } });
    expect(screen.getByText(FINGERPRINT)).toBeTruthy();
    expect(screen.getByText("Compare it with the fingerprint in the recovery kit.")).toBeTruthy();
    await expectNoA11yViolations(container);

    await chooseFolder(user);
    expect(screen.getByText("Not compatible: newer app version")).toBeTruthy();
    expect(screen.queryByText(/Select a compatible backup/)).toBeNull();
    const check = screen.getByRole("button", { name: "Check this backup" });
    // Nothing is chosen yet.
    expect(check.hasAttribute("disabled")).toBe(true);
    await openRow(user, "0.9.0");
    expect(screen.getByText(/It is not compatible with this app/)).toBeTruthy();
    expect(check.hasAttribute("disabled")).toBe(true);
    // Enter chooses a backup too; the chosen row is highlighted and named.
    await openRowWithEnter(user, "0.4.0");
    expect(screen.getByText(/Chosen: backup made/).textContent).toContain("app 0.4.0");
    const rows = screen.getAllByRole("row");
    expect(rows.filter((row) => row.getAttribute("aria-current") === "true")).toHaveLength(1);
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(check.hasAttribute("disabled")).toBe(false);
    await user.click(check);
    expect(await screen.findByText(/can take several minutes/)).toBeTruthy();
    expect(engine.stage_restore).toHaveBeenCalledWith({
      input: { restoreSessionId: SESSION_ID, setId: SET_ID },
    });
    await expectNoA11yViolations(container);
    finishStage(staged());

    await screen.findByRole("heading", { level: 2, name: "Result of the check" });
    expect(screen.getByText("Integrity check passed")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "clinical" })).toBeTruthy();
    expect(screen.getByText("No audit anchors were compared.")).toBeTruthy();
    expect(
      screen.getByText(/Work recorded after the backup was made \(expected after a restore\): 2/),
    ).toBeTruthy();
    await expectNoA11yViolations(container);

    await user.click(screen.getByRole("button", { name: "Compare with anchors on another drive" }));
    expect(await screen.findByRole("heading", { name: "Anchors on the other drive" })).toBeTruthy();
    expect(screen.getAllByText("12").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Restore this backup" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Restore this backup" });
    expect(
      within(dialog).getByText(/The current data folder is kept aside; the app closes; open it/),
    ).toBeTruthy();
    await expectNoA11yViolations(container);
    const confirm = within(dialog).getByRole("button", { name: "Restore and close the app" });
    await user.click(confirm);
    expect(await within(dialog).findByText("Enter confirmation.")).toBeTruthy();
    await user.type(within(dialog).getByRole("textbox", { name: /Confirmation/ }), "restore");
    await user.click(confirm);
    expect(
      (await within(dialog).findAllByText("Type RESTORE in capital letters.")).length,
    ).toBeGreaterThan(0);
    expect(engine.confirm_restore).not.toHaveBeenCalled();

    await user.clear(within(dialog).getByRole("textbox", { name: /Confirmation/ }));
    await user.type(within(dialog).getByRole("textbox", { name: /Confirmation/ }), "RESTORE");
    await user.click(confirm);
    expect(await screen.findByRole("heading", { level: 2, name: "Restore prepared" })).toBeTruthy();
    expect(engine.confirm_restore).toHaveBeenCalledWith({
      input: { restoreSessionId: SESSION_ID, confirmation: "RESTORE" },
    });
    expect(screen.queryByText(FINGERPRINT)).toBeNull();
    await expectNoA11yViolations(container);
  });

  it("goes back without starting a restore before the key is accepted", async () => {
    const engine = restoreEngine();
    const { user, onExit } = renderWizard();
    await user.click(await screen.findByRole("button", { name: "Back" }));
    expect(onExit).toHaveBeenCalledOnce();
    expect(engine.cancel_restore).not.toHaveBeenCalled();
  });

  it.each([
    ["choosing the folder", async (_user: User) => undefined],
    [
      "checking a backup",
      async (user: User) => {
        await chooseFolder(user);
        await checkBackup(user);
      },
    ],
  ])("cancels the restore session after %s", async (_step, reach) => {
    const engine = restoreEngine();
    const { user, onExit } = renderWizard();
    await enterKey(user);
    await reach(user);
    await user.click(screen.getByRole("button", { name: "Cancel restore" }));
    await vi.waitFor(() => expect(onExit).toHaveBeenCalledOnce());
    expect(engine.cancel_restore).toHaveBeenCalledWith({
      input: { restoreSessionId: SESSION_ID },
    });
  });
});
