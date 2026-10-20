import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { KeySignInForm } from "./KeySignInForm";
import { LockScreen } from "./LockScreen";
import { RecoverKeyForm } from "./RecoverKeyForm";
import { SetupScreen } from "./SetupScreen";
import { SignedOut } from "./SignedOut";
import { TEST_SESSION } from "./testSession";

const KEYS = [
  { credentialId: "synthetic-key-1", label: "Everyday key", revision: 1 },
  { credentialId: "synthetic-key-2", label: "Spare key", revision: 1 },
];
const KEY_ACCOUNT = { method: "security_key", credentials: KEYS, recoveryAvailable: true };

describe("Personal security keys (ADR 0019)", () => {
  it("registers the first account without a password and waits for recovery-code acknowledgment", async () => {
    const signup = vi.fn(() => ({ recoveryCode: "synthetic-recovery-code" }));
    const acknowledgedCodes: string[] = [];
    const acknowledge = vi.fn((args) => {
      acknowledgedCodes.push(args.input.recoveryCode);
      return TEST_SESSION;
    });
    const passwordSignup = vi.fn();
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
      security_key_signup: signup,
      acknowledge_key_recovery: acknowledge,
      create_first_administrator: passwordSignup,
    });
    const { user } = renderWithProviders(<SetupScreen onCreated={vi.fn()} />);
    await screen.findByRole("heading", { name: "Set up this laboratory" });
    await chooseWithKeyboard(user, /Sign-in method/, "Security key");
    expect(screen.queryByLabelText(/^Password/, { selector: "input" })).toBeNull();
    await user.type(screen.getByLabelText(/^Display name/), "Synthetic Analyst");
    await user.type(screen.getByLabelText(/^Username/), "synthetic-analyst");
    await user.type(screen.getByLabelText(/^Key name/), "Everyday key");
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    await screen.findByRole("heading", { name: "Save your recovery code" });
    expect(signup).toHaveBeenCalledWith({
      input: {
        username: "synthetic-analyst",
        displayName: "Synthetic Analyst",
        onePerson: false,
        loadDemoData: null,
        keyLabel: "Everyday key",
      },
    });
    expect(passwordSignup).not.toHaveBeenCalled();
    expect(acknowledge).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Continue" })).toHaveProperty("disabled", true);
    await user.click(
      screen.getByRole("checkbox", { name: "I saved this code away from this computer" }),
    );
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(acknowledgedCodes).toEqual(["synthetic-recovery-code"]));
  });

  it("looks up the username only after submission and sends exactly the selected credential", async () => {
    const choices = vi.fn(() => KEYS);
    const signIn = vi.fn(() => TEST_SESSION);
    mockCommands({ list_key_sign_in_choices: choices, security_key_sign_in: signIn });
    const { user, container } = renderWithProviders(<KeySignInForm />);
    await user.type(screen.getByLabelText(/^Username/), "synthetic-analyst");
    expect(choices).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Find my keys" }));
    await screen.findByRole("button", { name: /Everyday key Security key/ });
    await chooseWithKeyboard(user, /Security key/, "Spare key");
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Sign in with security key" }));
    await waitFor(() =>
      expect(signIn).toHaveBeenCalledWith({
        input: { username: "synthetic-analyst", credentialId: "synthetic-key-2" },
      }),
    );
  });

  it("returns to existing-account sign-in after first-setup acknowledgment expires", async () => {
    let created = false;
    const acknowledge = vi.fn(() =>
      Promise.reject(engineError("not_found", "This sign-in step expired or was already used.")),
    );
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      setup_status: () => ({ needsAdministrator: !created, demoSetupPending: false }),
      get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
      security_key_signup: () => {
        created = true;
        return { recoveryCode: "synthetic-recovery-code" };
      },
      acknowledge_key_recovery: acknowledge,
    });
    const { user } = renderWithProviders(<SignedOut />);
    await screen.findByRole("heading", { name: "Set up this laboratory" });
    await chooseWithKeyboard(user, /Sign-in method/, "Security key");
    await user.type(screen.getByLabelText(/^Display name/), "Synthetic Analyst");
    await user.type(screen.getByLabelText(/^Username/), "synthetic-analyst");
    await user.type(screen.getByLabelText(/^Key name/), "Everyday key");
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    await screen.findByRole("heading", { name: "Save your recovery code" });
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    const signIn = await screen.findByRole("button", { name: "Continue to sign in" });
    expect(screen.getByText("synthetic-recovery-code")).toBeTruthy();
    await user.click(signIn);
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Use a security key" })).toBeTruthy();
    expect(acknowledge).toHaveBeenCalledOnce();
  });

  it("unlocks the existing key account without requesting a password", async () => {
    const unlock = vi.fn(() => TEST_SESSION);
    mockCommands({
      get_authentication_status: () => KEY_ACCOUNT,
      list_key_sign_in_choices: () => KEYS,
      security_key_unlock: unlock,
    });
    const { user } = renderWithProviders(
      <LockScreen session={{ ...TEST_SESSION, locked: true }} />,
    );
    await screen.findByRole("button", { name: "Unlock with security key" });
    expect(screen.queryByLabelText(/^Password/, { selector: "input" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Unlock with security key" }));
    await waitFor(() =>
      expect(unlock).toHaveBeenCalledWith({ input: { credentialId: "synthetic-key-1" } }),
    );
  });

  it("shows replacement recovery material before returning to sign-in", async () => {
    const recover = vi.fn(() => ({ recoveryCode: "synthetic-new-recovery-code" }));
    const close = vi.fn();
    mockCommands({ security_key_recover: recover });
    const { user } = renderWithProviders(<RecoverKeyForm onClose={close} />);
    expect(screen.getByText(/Recovery requires the replacement key's PIN/)).toBeTruthy();
    await user.type(screen.getByLabelText(/^Username/), "synthetic-analyst");
    await user.type(screen.getByLabelText(/^Recovery code/), "synthetic-old-code");
    await user.type(screen.getByLabelText(/^Key name/), "Replacement key");
    await user.click(screen.getByRole("button", { name: "Register replacement key" }));
    await screen.findByText("synthetic-new-recovery-code");
    expect(close).not.toHaveBeenCalled();
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(close).toHaveBeenCalledOnce();
  });
});
