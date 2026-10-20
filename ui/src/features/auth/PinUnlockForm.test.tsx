import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";
import { LockScreen } from "./LockScreen";
import { TEST_SESSION } from "./testSession";

const LOCKED = { ...TEST_SESSION, locked: true };
const ENABLED = { enabled: true, attemptsRemaining: 5 };
const PIN = "042759";

function Workspace() {
  const data = useCommandQuery<string>("list_worklist");
  const lock = useCommandMutation<null>("lock_session", { invalidates: "all" });
  return (
    <>
      <p>{data.data}</p>
      <Button onPress={() => lock.mutate({})}>Lock workspace</Button>
    </>
  );
}

describe("Personal PIN session unlock", () => {
  it("unlocks the same session and refetches workspace data instead of showing the prior cache", async () => {
    let session = TEST_SESSION;
    let generation = 1;
    const submitted: string[] = [];
    const unlock = vi.fn((args) => {
      submitted.push(args.input.pin);
      session = TEST_SESSION;
      generation = 2;
      return session;
    });
    mockCommands({
      current_session: () => session,
      get_unlock_pin_status: () => ENABLED,
      list_worklist: () => `Synthetic workspace ${generation}`,
      lock_session: () => {
        session = LOCKED;
        return null;
      },
      unlock_with_pin: unlock,
    });
    const { user, container } = renderWithProviders(<AuthGate>{() => <Workspace />}</AuthGate>);
    await screen.findByText("Synthetic workspace 1");
    await user.click(screen.getByRole("button", { name: "Lock workspace" }));
    const pin = await screen.findByLabelText<HTMLInputElement>(/^Unlock PIN/);
    expect(screen.queryByText("Synthetic workspace 1")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.type(pin, `${PIN}{Enter}`);
    expect(await screen.findByText("Synthetic workspace 2")).toBeTruthy();
    expect(screen.queryByText("Synthetic workspace 1")).toBeNull();
    expect(submitted).toEqual([PIN]);
    expect(unlock.mock.calls[0]?.[0].input.pin).toBe("");
  });

  it("clears an incorrect PIN, displays its field error and refreshes attempts remaining", async () => {
    let attemptsRemaining = 5;
    mockCommands({
      current_session: () => LOCKED,
      get_unlock_pin_status: () => ({ enabled: true, attemptsRemaining }),
      unlock_with_pin: () => {
        attemptsRemaining -= 1;
        return Promise.reject(
          engineError("validation", "The PIN is incorrect.", {
            fieldErrors: [
              { field: "pin", code: "incorrect", message: "Enter your own unlock PIN." },
            ],
          }),
        );
      },
    });
    const { user } = renderWithProviders(<AuthGate>{() => <p>Workspace</p>}</AuthGate>);
    const pin = await screen.findByLabelText<HTMLInputElement>(/^Unlock PIN/);
    await user.type(pin, `${PIN}{Enter}`);
    expect(await screen.findByText(/4 attempts remaining/)).toBeTruthy();
    expect(screen.getByText("Enter your own unlock PIN.")).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
    expect(pin.value).toBe("");
    expect(screen.queryByText("Workspace")).toBeNull();
  });

  it("returns to full sign-in after the engine ends the session on the fifth failure", async () => {
    let session: SessionInfo | null = LOCKED;
    mockCommands({
      current_session: () => session,
      setup_status: () => ({ needsAdministrator: false }),
      get_unlock_pin_status: () => ({ enabled: true, attemptsRemaining: 1 }),
      unlock_with_pin: () => {
        session = null;
        return Promise.reject(
          engineError("unauthenticated", "Sign in again after five incorrect PIN attempts."),
        );
      },
    });
    const { user } = renderWithProviders(<AuthGate>{() => <p>Workspace</p>}</AuthGate>);
    await user.type(await screen.findByLabelText(/^Unlock PIN/), `${PIN}{Enter}`);
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByLabelText(/^Unlock PIN/)).toBeNull();
    expect(screen.queryByText("Workspace")).toBeNull();
  });

  it.each(["password_totp", "security_key"])(
    "lets a %s account use its normal method instead and clears typed PIN on switch",
    async (method) => {
      mockCommands({
        get_unlock_pin_status: () => ENABLED,
        get_authentication_status: () => ({ method, credentials: [], recoveryAvailable: true }),
        list_key_sign_in_choices: () => [
          { credentialId: "synthetic-key", label: "Everyday key", revision: 1 },
        ],
      });
      const { user } = renderWithProviders(<LockScreen session={LOCKED} />);
      await user.type(await screen.findByLabelText(/^Unlock PIN/), PIN);
      await user.click(
        screen.getByRole("button", {
          name: method === "security_key" ? "Use security key instead" : "Use password instead",
        }),
      );
      expect(screen.queryByLabelText(/^Unlock PIN/)).toBeNull();
      if (method === "security_key")
        await screen.findByRole("button", { name: "Unlock with security key" });
      else await screen.findByLabelText(/^Password/);
      await user.click(screen.getByRole("button", { name: "Use unlock PIN" }));
      expect(screen.getByLabelText<HTMLInputElement>(/^Unlock PIN/).value).toBe("");
    },
  );

  it("offers normal unlock and different-user sign-in when PIN metadata fails", async () => {
    mockCommands({
      get_unlock_pin_status: () =>
        Promise.reject(engineError("internal", "PIN status unavailable.")),
    });
    renderWithProviders(<LockScreen session={LOCKED} />);
    expect(await screen.findByLabelText(/^Password/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in as a different user" })).toBeTruthy();
    expect(screen.queryByLabelText(/^Unlock PIN/)).toBeNull();
  });

  it("does not send malformed PIN text or silently truncate digits", async () => {
    const unlock = vi.fn();
    mockCommands({ get_unlock_pin_status: () => ENABLED, unlock_with_pin: unlock });
    const { user } = renderWithProviders(<LockScreen session={LOCKED} />);
    await user.type(await screen.findByLabelText(/^Unlock PIN/), "0427591{Enter}");
    expect(await screen.findByText("Enter exactly six digits.")).toBeTruthy();
    await waitFor(() => expect(unlock).not.toHaveBeenCalled());
  });
});
