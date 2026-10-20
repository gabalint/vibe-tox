import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { TEST_SESSION } from "./testSession";
import { UnlockPinDialog } from "./UnlockPinDialog";

const PIN = "042759";
type User = ReturnType<typeof renderWithProviders>["user"];

async function fillPin(user: User, confirm = PIN) {
  await user.type(await screen.findByLabelText(/^New unlock PIN/), PIN);
  await user.type(screen.getByLabelText(/^Confirm unlock PIN/), confirm);
}

describe("Own unlock PIN settings", () => {
  it("starts disabled, preserves leading zeros, and clears submitted secrets", async () => {
    const submitted: string[] = [];
    const set = vi.fn((args) => {
      submitted.push(args.input.pin);
      return null;
    });
    const close = vi.fn();
    mockCommands({ set_unlock_pin: set });
    const { user } = renderWithProviders(<UnlockPinDialog onClose={close} />);
    await user.click(await screen.findByRole("button", { name: "Enable PIN" }));
    expect(screen.queryByRole("button", { name: "Disable PIN" })).toBeNull();
    await fillPin(user);
    const input = screen.getByLabelText(/^New unlock PIN/);
    expect(input.getAttribute("type")).toBe("password");
    expect(input.getAttribute("inputmode")).toBe("numeric");
    expect(input.getAttribute("autocomplete")).toBe("off");
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await expectNoA11yViolations(document.body);
    await user.click(screen.getByRole("button", { name: "Enable PIN" }));
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(submitted).toEqual([PIN]);
    expect(set.mock.calls[0]?.[0].input.pin).toBe("");
    expect(screen.getByLabelText<HTMLInputElement>(/^New unlock PIN/).value).toBe("");
    expect(screen.getByLabelText<HTMLInputElement>(/^Confirm unlock PIN/).value).toBe("");
  });

  it("blocks a mismatching confirmation before any command", async () => {
    const set = vi.fn();
    mockCommands({ set_unlock_pin: set });
    const { user } = renderWithProviders(<UnlockPinDialog onClose={vi.fn()} />);
    await user.click(await screen.findByRole("button", { name: "Enable PIN" }));
    await fillPin(user, "842751");
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "Enable PIN" }));
    expect(await screen.findByText("The PINs do not match.")).toBeTruthy();
    expect(set).not.toHaveBeenCalled();
  });

  it("shows engine PIN-policy rejection without keeping the PIN in fields", async () => {
    mockCommands({
      set_unlock_pin: () =>
        Promise.reject(
          engineError("validation", "Choose a less predictable PIN.", {
            fieldErrors: [
              { field: "pin", code: "common", message: "This PIN pattern is not allowed." },
            ],
          }),
        ),
    });
    const { user } = renderWithProviders(<UnlockPinDialog onClose={vi.fn()} />);
    await user.click(await screen.findByRole("button", { name: "Enable PIN" }));
    await fillPin(user);
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "Enable PIN" }));
    expect(await screen.findByText("This PIN pattern is not allowed.")).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
    expect(screen.getByLabelText<HTMLInputElement>(/^New unlock PIN/).value).toBe("");
    expect(screen.getByRole("dialog", { name: "Enable unlock PIN" })).toBeTruthy();
  });

  it.each(["Change PIN", "Disable PIN"])(
    "confirms %s through the security-key action gateway",
    async (action) => {
      const submitted: unknown[] = [];
      const gateway = vi.fn((args) => {
        submitted.push(structuredClone(args));
        return null;
      });
      const direct = vi.fn();
      mockCommands({
        current_session: () => TEST_SESSION,
        get_unlock_pin_status: () => ({ enabled: true, attemptsRemaining: 5 }),
        get_authentication_status: () => ({
          method: "security_key",
          credentials: [{ credentialId: "synthetic-key", label: "Everyday key", revision: 1 }],
          recoveryAvailable: true,
        }),
        security_key_action: gateway,
        set_unlock_pin: direct,
        disable_unlock_pin: direct,
      });
      const close = vi.fn();
      const { user } = renderWithProviders(<UnlockPinDialog onClose={close} />);
      await user.click(await screen.findByRole("button", { name: action }));
      if (action === "Change PIN") await fillPin(user);
      await screen.findByRole("button", { name: /Everyday key Security key/ });
      expect(screen.queryByLabelText(/^Your password/)).toBeNull();
      await user.click(screen.getByRole("button", { name: action }));
      await waitFor(() => expect(close).toHaveBeenCalledOnce());
      expect(submitted).toEqual([
        {
          input: {
            credentialId: "synthetic-key",
            action: {
              command: action === "Change PIN" ? "set_unlock_pin" : "disable_unlock_pin",
              input: action === "Change PIN" ? { pin: PIN, password: "" } : { password: "" },
            },
          },
        },
      ]);
      expect(direct).not.toHaveBeenCalled();
      if (action === "Change PIN")
        expect(gateway.mock.calls[0]?.[0].input.action.input.pin).toBe("");
    },
  );

  it("shows status errors without offering an unverified setting change", async () => {
    const status = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "Try checking again."))
      .mockResolvedValue({ enabled: false, attemptsRemaining: 0 });
    mockCommands({ get_unlock_pin_status: status });
    const { user } = renderWithProviders(<UnlockPinDialog onClose={vi.fn()} />);
    expect(await screen.findByText("The unlock PIN status could not be checked")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Enable PIN" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("button", { name: "Enable PIN" })).toBeTruthy();
  });
});
