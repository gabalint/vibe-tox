import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ReasonConfirmDialog } from "./ReasonConfirmDialog";

describe("ReasonConfirmDialog", () => {
  it("requires a reason, sends it trimmed, and closes once recorded", async () => {
    const onClose = vi.fn();
    const mutate = vi.fn(
      (_args: { input: { reason: string } }, options: { onSuccess: () => void }) =>
        options.onSuccess(),
    );
    const { user } = renderWithProviders(
      <ReasonConfirmDialog
        title="Retire version 2?"
        confirmLabel="Retire version"
        variant="danger"
        command={{ mutate, error: null, isPending: false }}
        input={(reason) => ({ reason })}
        successTitle="Version retired"
        onClose={onClose}
      >
        <p>New orders can no longer use it.</p>
      </ReasonConfirmDialog>,
    );
    const dialog = screen.getByRole("alertdialog", { name: "Retire version 2?" });
    await expectNoA11yViolations(dialog);
    await user.click(screen.getByRole("button", { name: "Retire version" }));
    expect(await screen.findByText("Enter the reason for this change.")).toBeTruthy();
    expect(mutate).not.toHaveBeenCalled();
    // Focus moves to the field to fix, and Enter there confirms.
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: /Reason/ }));
    await user.keyboard(" Replaced {Enter}");
    expect(mutate).toHaveBeenCalledWith({ input: { reason: "Replaced" } }, expect.anything());
    expect(await screen.findByText("Version retired")).toBeTruthy();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("shows the engine's refusal", () => {
    renderWithProviders(
      <ReasonConfirmDialog
        title="Retire version 2?"
        confirmLabel="Retire version"
        variant="danger"
        command={{
          mutate: vi.fn(),
          error: engineError("conflict", "Only an approved version can be retired."),
          isPending: false,
        }}
        input={(reason) => ({ reason })}
        successTitle="Version retired"
        onClose={vi.fn()}
      >
        <p>New orders can no longer use it.</p>
      </ReasonConfirmDialog>,
    );
    expect(screen.getByRole("alert").textContent).toContain(
      "Only an approved version can be retired.",
    );
  });
});
