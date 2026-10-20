import { screen } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { ReasonField } from "./ReasonField";

function Harness({ onSubmit }: { onSubmit: (reason: string) => void }) {
  const form = useForm({ defaultValues: { reason: "" } });
  return (
    <FormProvider {...form}>
      <form aria-label="Change" onSubmit={form.handleSubmit(({ reason }) => onSubmit(reason))}>
        <ReasonField />
        <button type="submit">Save</button>
      </form>
    </FormProvider>
  );
}

describe("ReasonField", () => {
  it("requires a reason that is not only spaces", async () => {
    const onSubmit = vi.fn();
    const { user, container } = renderWithProviders(<Harness onSubmit={onSubmit} />);
    await expectNoA11yViolations(container);
    await user.type(screen.getByRole("textbox", { name: /Reason/ }), "   ");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Enter the reason for this change.")).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
    await user.type(screen.getByRole("textbox", { name: /Reason/ }), "Corrected label");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).toHaveBeenCalledWith("   Corrected label");
    expect(screen.getByText("Recorded in the audit trail.")).toBeTruthy();
  });
});
