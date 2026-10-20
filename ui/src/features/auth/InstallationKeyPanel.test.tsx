import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";

const KEY = "SYN0-TEST-KEY0-0000-0000-0000";

function renderSignIn(commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({
    current_session: () => null,
    setup_status: () => ({ needsAdministrator: false }),
    ...commands,
  });
  return renderWithProviders(<AuthGate>{() => <p>Signed in</p>}</AuthGate>);
}

async function signInHeading() {
  return screen.findByRole("heading", { level: 1, name: "Sign in" });
}

describe("Sign-in installation key panel (M4 design brief §3.9)", () => {
  it.each([
    ["missing", /does not hold the installation key that protects sign-in codes/],
    ["mismatch", /does not match the one that protects the stored sign-in codes/],
  ])("restores a %s key from the recovery kit", async (state, text) => {
    let keyState = state;
    const keyQuery = vi.fn(() => ({ keyState }));
    const restore = vi.fn(() => {
      keyState = "ok";
      return { keyState };
    });
    const { user, container } = renderSignIn({
      get_installation_key_state: keyQuery,
      restore_installation_key: restore,
    });
    expect(await screen.findByText(text)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Restore the key" }));
    expect(await screen.findByText("Enter installation key.")).toBeTruthy();
    expect(restore).not.toHaveBeenCalled();

    await user.type(screen.getByRole("textbox", { name: /Installation key/ }), KEY);
    await user.click(screen.getByRole("button", { name: "Restore the key" }));
    expect(await screen.findByText(/The installation key was restored/)).toBeTruthy();
    expect(restore).toHaveBeenCalledWith({ input: { installationKey: KEY } });
    expect(keyQuery).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("button", { name: "Restore the key" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("says the key store could not be read, without a form", async () => {
    const { container } = renderSignIn({
      get_installation_key_state: () => ({ keyState: "unavailable" }),
    });
    expect(await screen.findByText("The key store could not be read")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: /Installation key/ })).toBeNull();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it.each([
    ["the key is fine", () => ({ keyState: "ok" })],
    ["the key state cannot be read", () => Promise.reject(engineError("internal", "No."))],
  ])("shows nothing extra when %s", async (_case, answer) => {
    const keyQuery = vi.fn(answer);
    renderSignIn({ get_installation_key_state: keyQuery });
    expect(await signInHeading()).toBeTruthy();
    await vi.waitFor(() => expect(keyQuery).toHaveBeenCalled());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("textbox", { name: /Installation key/ })).toBeNull();
  });

  it.each([
    ["malformed", "invalid_format", "Enter the whole key line from the recovery kit."],
    ["of another installation", "does_not_open", "This key does not belong to this installation."],
  ])("keeps a %s key on its field with the engine's reason", async (_case, code, message) => {
    const restore = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "installationKey", code, message }],
        }),
      ),
    );
    const keyQuery = vi.fn(() => ({ keyState: "missing" }));
    const { user, container } = renderSignIn({
      get_installation_key_state: keyQuery,
      restore_installation_key: restore,
    });
    const field = await screen.findByRole("textbox", { name: /Installation key/ });
    await user.type(field, KEY);
    await user.click(screen.getByRole("button", { name: "Restore the key" }));
    expect(await screen.findByText("The installation key was not restored")).toBeTruthy();
    expect(screen.getByText(message)).toBeTruthy();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(screen.queryByText(/The installation key was restored/)).toBeNull();
    expect(keyQuery).toHaveBeenCalledTimes(1);
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("says when the key is correct but could not be saved", async () => {
    const { user } = renderSignIn({
      get_installation_key_state: () => ({ keyState: "missing" }),
      restore_installation_key: () =>
        Promise.reject(
          engineError(
            "unavailable",
            "The key is correct but could not be saved in this computer's key store.",
          ),
        ),
    });
    await user.type(await screen.findByRole("textbox", { name: /Installation key/ }), KEY);
    await user.click(screen.getByRole("button", { name: "Restore the key" }));
    expect(await screen.findByText(/could not be saved in this computer's key store/)).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
