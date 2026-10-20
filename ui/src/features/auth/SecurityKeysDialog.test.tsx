import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { SecurityKeysDialog } from "./SecurityKeysDialog";
import { TEST_SESSION } from "./testSession";

describe("Security key enrollment", () => {
  it.each([false, true])(
    "explains the current touch-only enrollment policy (%s)",
    async (allowed) => {
      mockCommands({
        get_authentication_status: () => ({
          method: "password_totp",
          credentials: [],
          recoveryAvailable: false,
          allowSecurityKeyTouchOnly: allowed,
        }),
      });
      const { user } = renderWithProviders(
        <SecurityKeysDialog session={TEST_SESSION} onClose={vi.fn()} />,
      );
      await user.click(await screen.findByRole("button", { name: "Switch to a security key" }));
      if (allowed) {
        expect(screen.getByText("This laboratory allows sign-in without a key PIN")).toBeTruthy();
        expect(screen.getByText(/A lost or unattended key could let someone sign in/)).toBeTruthy();
        expect(screen.getByText(/including adding another key, still need a PIN/)).toBeTruthy();
      } else {
        expect(
          screen.getByText(/This laboratory requires the key's PIN or biometric/),
        ).toBeTruthy();
        expect(screen.queryByText("This laboratory allows sign-in without a key PIN")).toBeNull();
      }
    },
  );

  it("keeps the new recovery code available even if refreshing the key inventory fails", async () => {
    let enrolled = false;
    mockCommands({
      get_authentication_status: () =>
        enrolled
          ? Promise.reject(engineError("internal", "The key inventory could not be read."))
          : {
              method: "password_totp",
              credentials: [],
              recoveryAvailable: false,
              allowSecurityKeyTouchOnly: false,
            },
      security_key_enroll: () => {
        enrolled = true;
        return { recoveryCode: "synthetic-new-code" };
      },
    });
    const { user } = renderWithProviders(
      <SecurityKeysDialog session={TEST_SESSION} onClose={vi.fn()} />,
    );
    await user.click(await screen.findByRole("button", { name: "Switch to a security key" }));
    await user.type(screen.getByLabelText(/^Key name/), "Everyday key");
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-password");
    await user.click(screen.getByRole("button", { name: "Register key and switch sign-in" }));
    expect(await screen.findByText("synthetic-new-code")).toBeTruthy();
    expect(screen.queryByText("Security keys could not be loaded")).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText("Security keys could not be loaded")).toBeTruthy();
  });
});
