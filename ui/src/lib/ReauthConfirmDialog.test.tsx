import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ReauthConfirmDialog } from "./ReauthConfirmDialog";

function renderDialog(answer: (args: unknown) => unknown) {
  const onClose = vi.fn();
  const handler = vi.fn(answer);
  mockCommands({ withdraw_lot: handler });
  const view = renderWithProviders(
    <ReauthConfirmDialog
      title="Withdraw lot SYN-LOT-1?"
      confirmLabel="Withdraw lot"
      variant="danger"
      command={{ name: "withdraw_lot", invalidates: [] }}
      extraDefaults={{}}
      toInput={({ password }) => ({ password })}
      successTitle={() => "Lot withdrawn"}
      errorTitle="Not withdrawn"
      onClose={onClose}
    >
      <p>The synthetic lot can no longer be used on new runs.</p>
    </ReauthConfirmDialog>,
  );
  return { ...view, onClose, handler };
}

describe("ReauthConfirmDialog", () => {
  it("confirms on Enter in the password field", async () => {
    const { user, onClose, handler } = renderDialog(() => null);
    await expectNoA11yViolations(screen.getByRole("alertdialog"));
    await user.type(screen.getByLabelText(/^Your password/), "synthetic-secret{Enter}");
    expect(handler).toHaveBeenCalledWith({ input: { password: "synthetic-secret" } });
    expect(await screen.findByText("Lot withdrawn")).toBeTruthy();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("puts focus on a missing password instead of confirming", async () => {
    const { user, handler } = renderDialog(() => null);
    await user.click(screen.getByRole("button", { name: "Withdraw lot" }));
    expect(await screen.findByText("Enter your password.")).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByLabelText(/^Your password/));
    expect(handler).not.toHaveBeenCalled();
  });

  it("shows a wrong password on the field, focused, and stays open", async () => {
    const { user, onClose } = renderDialog(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "password", code: "incorrect", message: "The password is not correct." },
          ],
        }),
      ),
    );
    const password = await screen.findByLabelText(/^Your password/);
    await user.type(password, "wrong-secret");
    await user.click(screen.getByRole("button", { name: "Withdraw lot" }));
    expect(await screen.findByText("The password is not correct.")).toBeTruthy();
    expect(document.activeElement).toBe(password);
    expect(onClose).not.toHaveBeenCalled();
  });
});
