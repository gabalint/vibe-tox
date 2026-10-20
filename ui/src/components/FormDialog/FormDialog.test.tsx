import { screen } from "@testing-library/react";
import { useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { ReasonField } from "@/components/ReasonField";
import type { CommandError } from "@/generated/CommandError";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { FormDialog } from "./FormDialog";

interface HarnessProps {
  onSave: (reason: string) => void;
  onClose: () => void;
  error?: CommandError | null;
  isPending?: boolean;
}

function Harness({ onSave, onClose, error = null, isPending = false }: HarnessProps) {
  const form = useForm({ defaultValues: { reason: "" } });
  return (
    <FormDialog
      title="Discard batch"
      submitLabel="Discard batch"
      form={form}
      onSubmit={form.handleSubmit(({ reason }) => onSave(reason))}
      command={{ error, isPending }}
      errorTitle="The batch was not discarded"
      onClose={onClose}
      extraActions={<Button>Reload batch</Button>}
    >
      <ReasonField />
    </FormDialog>
  );
}

describe("FormDialog", () => {
  it("submits its form, with the fields reached through the form context", async () => {
    const onSave = vi.fn();
    const { user } = renderWithProviders(<Harness onSave={onSave} onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "Discard batch" });
    await expectNoA11yViolations(dialog);
    await user.click(screen.getByRole("button", { name: "Discard batch" }));
    expect(await screen.findByText("Enter the reason for this change.")).toBeTruthy();
    expect(onSave).not.toHaveBeenCalled();
    await user.type(screen.getByRole("textbox", { name: /Reason/ }), "Expired reagent{Enter}");
    expect(onSave).toHaveBeenCalledWith("Expired reagent");
    expect(screen.getByRole("button", { name: "Reload batch" })).toBeTruthy();
  });

  it("shows the engine's refusal and closes on Cancel", async () => {
    const onClose = vi.fn();
    const { user } = renderWithProviders(
      <Harness
        onSave={vi.fn()}
        onClose={onClose}
        error={engineError("conflict", "The batch changed.")}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain("The batch was not discarded");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes on Escape only while nothing is typed and no command runs", async () => {
    const onClose = vi.fn();
    const { user, unmount } = renderWithProviders(<Harness onSave={vi.fn()} onClose={onClose} />);
    await user.type(screen.getByRole("textbox", { name: /Reason/ }), "Expired");
    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
    await user.clear(screen.getByRole("textbox", { name: /Reason/ }));
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
    unmount();

    const whileRunning = vi.fn();
    const running = renderWithProviders(
      <Harness onSave={vi.fn()} onClose={whileRunning} isPending />,
    );
    await running.user.keyboard("{Escape}");
    expect(whileRunning).not.toHaveBeenCalled();
  });
});
