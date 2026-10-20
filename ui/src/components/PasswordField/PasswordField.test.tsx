import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { Form } from "@/components/Form";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { PasswordField } from "./PasswordField";

function Harness({ onSubmit }: { onSubmit: (password: string) => void }) {
  const form = useForm({ defaultValues: { password: "" } });
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Confirm"
        onSubmit={form.handleSubmit(({ password }) => onSubmit(password))}
        actions={<button type="submit">Confirm</button>}
      >
        <PasswordField />
      </Form>
    </FormProvider>
  );
}

describe("PasswordField", () => {
  it("requires the password and hides what is typed", async () => {
    const onSubmit = vi.fn();
    const { container } = render(<Harness onSubmit={onSubmit} />);
    const field = screen.getByLabelText(/^Password/);
    expect(field.getAttribute("type")).toBe("password");
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByText("Enter your password.")).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
    await userEvent.type(field, "synthetic-secret");
    await userEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(onSubmit).toHaveBeenCalledWith("synthetic-secret");
    await expectNoA11yViolations(container);
  });
});
