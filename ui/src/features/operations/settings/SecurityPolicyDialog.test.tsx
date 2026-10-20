import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SecurityPolicyView } from "@/generated/SecurityPolicyView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { SecurityPolicyDialog } from "./SecurityPolicyDialog";

const DEFAULTS = {
  idleTimeoutMinutes: 15,
  lockoutFailures: 5,
  lockoutMinutes: 15,
  allowSecurityKeyTouchOnly: false,
};
const UNSET: SecurityPolicyView = {
  version: null,
  values: DEFAULTS,
  defaults: DEFAULTS,
  setBy: null,
  setAt: null,
};
const CHOICE = /Allow security keys without a PIN for sign-in/;
const PASSWORD = "synthetic-password";

describe("Security key sign-in policy", () => {
  it("defaults to verified keys and saves numeric settings without fresh confirmation", async () => {
    const set = vi.fn(() => UNSET);
    const auth = vi.fn(() => ({
      method: "security_key",
      credentials: [],
      recoveryAvailable: true,
    }));
    const gateway = vi.fn();
    mockCommands({
      set_security_policy: set,
      get_authentication_status: auth,
      security_key_action: gateway,
    });
    const { user } = renderWithProviders(<SecurityPolicyDialog view={UNSET} onClose={vi.fn()} />);
    expect(screen.getByRole("checkbox", { name: CHOICE })).toHaveProperty("checked", false);
    expect(screen.queryByLabelText(/^Your password/)).toBeNull();
    expect(screen.queryByLabelText(/^Reason/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(set).toHaveBeenCalledOnce());
    expect(set).toHaveBeenCalledWith({
      input: { expectedVersion: null, ...DEFAULTS, reason: null, password: null },
    });
    expect(auth).not.toHaveBeenCalled();
    expect(gateway).not.toHaveBeenCalled();
  });

  it("requires a reason even for the first enable and keeps errors attached to confirmation", async () => {
    const set = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "password", code: "incorrect", message: "Incorrect password." }],
        }),
      )
      .mockResolvedValue(UNSET);
    const auth = vi.fn(() => ({
      method: "password_totp",
      credentials: [],
      recoveryAvailable: false,
      allowSecurityKeyTouchOnly: false,
    }));
    mockCommands({ set_security_policy: set, get_authentication_status: auth });
    const close = vi.fn();
    const { user } = renderWithProviders(<SecurityPolicyDialog view={UNSET} onClose={close} />);
    await user.click(screen.getByRole("checkbox", { name: CHOICE }));
    expect(await screen.findByText("Possession of a key can be enough to sign in")).toBeTruthy();
    await user.type(await screen.findByLabelText(/^Your password/), "wrong-password");
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Enter the reason for this change.")).toBeTruthy();
    expect(set).not.toHaveBeenCalled();
    await user.type(screen.getByRole("textbox", { name: /^Reason/ }), "Reviewed physical access");
    await expectNoA11yViolations(document.body);
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Incorrect password.")).toBeTruthy();
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByRole("checkbox", { name: CHOICE })).toHaveProperty("checked", true);
    await user.clear(screen.getByLabelText(/^Your password/));
    await user.type(screen.getByLabelText(/^Your password/), PASSWORD);
    const beforeSave = auth.mock.calls.length;
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(set).toHaveBeenLastCalledWith({
      input: {
        expectedVersion: null,
        ...DEFAULTS,
        allowSecurityKeyTouchOnly: true,
        reason: "Reviewed physical access",
        password: PASSWORD,
      },
    });
    expect(auth.mock.calls.length).toBeGreaterThan(beforeSave);
  });

  it("drops confirmation when the checkbox is returned to its saved value", async () => {
    const set = vi.fn(() => UNSET);
    mockCommands({ set_security_policy: set });
    const { user } = renderWithProviders(<SecurityPolicyDialog view={UNSET} onClose={vi.fn()} />);
    const choice = screen.getByRole("checkbox", { name: CHOICE });
    await user.click(choice);
    await user.type(await screen.findByLabelText(/^Your password/), PASSWORD);
    await user.click(choice);
    expect(screen.queryByLabelText(/^Your password/)).toBeNull();
    expect(screen.queryByLabelText(/^Reason/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(set).toHaveBeenCalledOnce());
    expect(set).toHaveBeenCalledWith({
      input: { expectedVersion: null, ...DEFAULTS, reason: null, password: null },
    });
  });

  it("requires verified key confirmation to disable and permits retry after native cancellation", async () => {
    const view = { ...UNSET, version: 1, values: { ...DEFAULTS, allowSecurityKeyTouchOnly: true } };
    const gateway = vi
      .fn()
      .mockRejectedValueOnce(engineError("unavailable", "Security key confirmation was cancelled."))
      .mockResolvedValue(UNSET);
    const direct = vi.fn();
    mockCommands({
      current_session: () => sessionWith(["manage_users"]),
      get_authentication_status: () => ({
        method: "security_key",
        allowSecurityKeyTouchOnly: true,
        recoveryAvailable: true,
        credentials: [{ credentialId: "synthetic-key", label: "Everyday key", revision: 1 }],
      }),
      security_key_action: gateway,
      set_security_policy: direct,
    });
    const close = vi.fn();
    const { user } = renderWithProviders(<SecurityPolicyDialog view={view} onClose={close} />);
    const dialog = screen.getByRole("dialog", { name: "Change security policy" });
    expect(screen.getByRole("checkbox", { name: CHOICE })).toHaveProperty("checked", true);
    await user.click(screen.getByRole("checkbox", { name: CHOICE }));
    await screen.findByRole("button", { name: /Everyday key Security key/ });
    expect(screen.queryByLabelText(/^Your password/)).toBeNull();
    await user.type(screen.getByRole("textbox", { name: /^Reason/ }), "Require verified sign-in");
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Security key confirmation was cancelled.")).toBeTruthy();
    expect(close).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(direct).not.toHaveBeenCalled();
    expect(gateway).toHaveBeenLastCalledWith({
      input: {
        credentialId: "synthetic-key",
        action: {
          command: "set_security_policy",
          input: {
            expectedVersion: 1,
            ...DEFAULTS,
            reason: "Require verified sign-in",
            password: "",
          },
        },
      },
    });
  });
});
