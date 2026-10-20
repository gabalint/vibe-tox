import { screen } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { CheckList } from "./CheckList";
import { ChoiceInput } from "./ChoiceInput";
import { DATE_FORMAT, TextInput } from "./TextInput";

interface Values {
  code: string;
  date: string;
  matrix: string | null;
  site: string | null;
  panels: string[];
}

function Harness({ onSubmit }: { onSubmit: (values: Values) => void }) {
  const form = useForm<Values>({
    defaultValues: { code: "", date: "", matrix: null, site: null, panels: [] },
  });
  return (
    <FormProvider {...form}>
      <form aria-label="Synthetic form" onSubmit={form.handleSubmit(onSubmit)}>
        <TextInput name="code" label="Code" isRequired />
        <TextInput name="date" label="Collection date" format={DATE_FORMAT} />
        <ChoiceInput
          name="matrix"
          label="Matrix"
          options={[{ id: "urine", label: "Urine" }]}
          isRequired
        />
        <ChoiceInput name="site" label="Site" options={[]} isDisabled />
        <CheckList
          name="panels"
          legend="Panels"
          options={[{ id: "a", label: "Synthetic panel A" }]}
          requiredMessage="Choose at least one panel."
        />
        <button type="submit">Save</button>
      </form>
    </FormProvider>
  );
}

describe("FormFields", () => {
  it("checks presence and format before submitting, then submits the typed values", async () => {
    const onSubmit = vi.fn();
    const { user, container } = renderWithProviders(<Harness onSubmit={onSubmit} />);
    await expectNoA11yViolations(container);
    await user.type(screen.getByRole("textbox", { name: /Collection date/ }), "12/10/2026");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Enter code.")).toBeTruthy();
    expect(screen.getByText("Enter the date as YYYY-MM-DD.")).toBeTruthy();
    expect(screen.getByText("Choose matrix.")).toBeTruthy();
    expect(screen.getByText("Choose at least one panel.")).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(screen.getByRole("textbox", { name: /Code/ }));

    await user.type(screen.getByRole("textbox", { name: /Code/ }), "SYN-1");
    await user.clear(screen.getByRole("textbox", { name: /Collection date/ }));
    await user.type(screen.getByRole("textbox", { name: /Collection date/ }), "2026-10-12");
    await user.click(screen.getByRole("button", { name: "Save" }));
    // A choice takes focus too when it is the first invalid field.
    expect(document.activeElement).toBe(screen.getByRole("button", { name: /Matrix$/ }));
    await chooseWithKeyboard(user, /Matrix$/, "Urine");
    await user.click(screen.getByRole("checkbox", { name: "Synthetic panel A" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).toHaveBeenCalledWith(
      { code: "SYN-1", date: "2026-10-12", matrix: "urine", site: null, panels: ["a"] },
      expect.anything(),
    );
  });

  it("disables a choice that has nothing to choose yet", () => {
    renderWithProviders(<Harness onSubmit={vi.fn()} />);
    expect(screen.getByRole("button", { name: /Site$/ })).toHaveProperty("disabled", true);
  });
});
