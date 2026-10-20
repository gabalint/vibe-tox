import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { AuthGate } from "./AuthGate";
import { TEST_SESSION } from "./testSession";
import { UserMenu } from "./UserMenu";

const NEW_PASSWORD = "synthetic-new-password";

function renderMenu() {
  return renderRoutes(
    [{ path: "/", element: <AuthGate>{(session) => <UserMenu session={session} />}</AuthGate> }],
    "/",
  );
}

type User = ReturnType<typeof renderMenu>["user"];

async function choose(user: User, item: string) {
  await user.click(await screen.findByRole("button", { name: "Signed in as Test Analyst" }));
  await user.click(await screen.findByRole("menuitem", { name: item }));
}

async function changePassword(user: User, current: string) {
  await choose(user, "Change password");
  const dialog = await screen.findByRole("dialog", { name: "Change password" });
  await user.type(screen.getByLabelText(/^Current password/), current);
  await user.type(screen.getByLabelText(/^New password/), NEW_PASSWORD);
  await user.type(screen.getByLabelText(/^Confirm new password/), NEW_PASSWORD);
  await user.click(within(dialog).getByRole("button", { name: "Change password" }));
  return dialog;
}

describe("UserMenu", () => {
  it("offers key management without a password-change action for key accounts", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      get_authentication_status: () => ({
        method: "security_key",
        credentials: [],
        recoveryAvailable: true,
      }),
    });
    const { user } = renderMenu();
    await user.click(await screen.findByRole("button", { name: "Signed in as Test Analyst" }));
    expect(screen.queryByRole("menuitem", { name: "Change password" })).toBeNull();
    expect(screen.getByRole("menuitem", { name: "Security keys" })).toBeTruthy();
  });
  it("shows who is signed in", async () => {
    mockCommands({ current_session: () => TEST_SESSION });
    const { container } = renderMenu();
    expect((await screen.findByText("Test Analyst")).textContent).toBe("Signed in as Test Analyst");
    await expectNoA11yViolations(container);
  });

  it("offers the account actions and About this app in one menu, from the keyboard", async () => {
    mockCommands({ current_session: () => TEST_SESSION });
    const { user } = renderMenu();
    const trigger = await screen.findByRole("button", { name: "Signed in as Test Analyst" });
    expect(screen.queryByRole("menu")).toBeNull();
    await user.tab();
    expect(document.activeElement).toBe(trigger);
    await user.keyboard("{Enter}");
    const menu = await screen.findByRole("menu", { name: "Signed in as Test Analyst" });
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual([
      "Change password",
      "Security keys",
      "Unlock PIN",
      "Lock now",
      "Sign out",
      "About this app",
    ]);
    await expectNoA11yViolations(document.body);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(trigger));
  });

  it("opens About this app", async () => {
    mockCommands({ current_session: () => TEST_SESSION });
    const { user } = renderMenu();
    await choose(user, "About this app");
    expect(await screen.findByText("Navigated to /about")).toBeTruthy();
  });

  it("enables and disables the personal PIN from the menu and refreshes its status", async () => {
    let enabled = false;
    mockCommands({
      current_session: () => TEST_SESSION,
      get_unlock_pin_status: () => ({ enabled, attemptsRemaining: enabled ? 5 : 0 }),
      set_unlock_pin: () => {
        enabled = true;
        return null;
      },
      disable_unlock_pin: () => {
        enabled = false;
        return null;
      },
    });
    const { user } = renderMenu();
    await choose(user, "Unlock PIN");
    await user.click(await screen.findByRole("button", { name: "Enable PIN" }));
    await user.type(screen.getByLabelText(/^New unlock PIN/), "042759");
    await user.type(screen.getByLabelText(/^Confirm unlock PIN/), "042759");
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "Enable PIN" }));
    await screen.findByText("Unlock PIN enabled");
    await choose(user, "Unlock PIN");
    await user.click(await screen.findByRole("button", { name: "Disable PIN" }));
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "Disable PIN" }));
    await screen.findByText("Unlock PIN disabled");
    await choose(user, "Unlock PIN");
    expect(await screen.findByRole("button", { name: "Enable PIN" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Disable PIN" })).toBeNull();
  });

  it("locks the session and shows the lock screen", async () => {
    let session: SessionInfo = TEST_SESSION;
    mockCommands({
      current_session: () => session,
      lock_session: () => {
        session = { ...TEST_SESSION, locked: true };
        return null;
      },
    });
    const { user } = renderMenu();
    await choose(user, "Lock now");
    expect(await screen.findByRole("heading", { name: "Session locked" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Signed in as Test Analyst" })).toBeNull();
  });

  it("signs out to the sign-in screen", async () => {
    let session: SessionInfo | null = TEST_SESSION;
    mockCommands({
      current_session: () => session,
      setup_status: () => ({ needsAdministrator: false }),
      sign_out: () => {
        session = null;
        return null;
      },
    });
    const { user } = renderMenu();
    await choose(user, "Sign out");
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
  });

  it("stays signed in and says so when the engine refuses to sign out", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      sign_out: () =>
        Promise.reject(engineError("internal", "The app could not complete the action.")),
    });
    const { user } = renderMenu();
    await choose(user, "Sign out");
    expect(await screen.findByText("The session did not change")).toBeTruthy();
    expect(screen.getByText("Test Analyst")).toBeTruthy();
  });

  it("changes the person's own password", async () => {
    const change = vi.fn(() => null);
    mockCommands({ current_session: () => TEST_SESSION, change_password: change });
    const { user } = renderMenu();
    const dialog = await changePassword(user, "synthetic-password");
    await expectNoA11yViolations(document.body);
    expect(await screen.findByText("Your password was changed")).toBeTruthy();
    expect(dialog.isConnected).toBe(false);
    expect(change).toHaveBeenCalledWith({
      input: { currentPassword: "synthetic-password", newPassword: NEW_PASSWORD },
    });
  });

  it("shows a wrong current password on its field and clears it (validation)", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      change_password: () =>
        Promise.reject(
          engineError("validation", "Check the highlighted fields.", {
            fieldErrors: [
              {
                field: "currentPassword",
                code: "incorrect",
                message: "The current password is incorrect.",
              },
            ],
          }),
        ),
    });
    const { user } = renderMenu();
    await changePassword(user, "wrong-password");
    const current = screen.getByLabelText<HTMLInputElement>(/^Current password/);
    await waitFor(() => expect(current.value).toBe(""));
    expect(document.activeElement).toBe(current);
    expect(current.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByText("The current password is incorrect.")).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "Change password" })).toBeTruthy();
    expect(screen.getByText("Test Analyst")).toBeTruthy();
    expect(screen.getByLabelText<HTMLInputElement>(/^New password/).value).toBe(NEW_PASSWORD);
  });
});
