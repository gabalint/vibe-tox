import { screen, within } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import type { CommandError } from "@/generated/CommandError";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { RecordPickerInput } from "./RecordPickerInput";

const CLIENTS = [
  { id: "c1", label: "Synthetic Clinic North", description: "SYN-CL-01" },
  { id: "c2", label: "Synthetic Clinic South", description: "SYN-CL-02" },
];
const LOCATIONS = [
  { id: "l1", clientId: "c1", label: "North main office" },
  { id: "l2", clientId: "c1", label: "North annex" },
  { id: "l3", clientId: "c2", label: "South main office" },
];

interface Values {
  clientId: string | null;
  locationId: string | null;
}

function OrderForm({ onSubmit, error }: { onSubmit: (v: Values) => void; error?: CommandError }) {
  const form = useForm<Values>({ defaultValues: { clientId: null, locationId: null } });
  const clientId = form.watch("clientId");
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Synthetic order"
        onSubmit={form.handleSubmit(onSubmit)}
        error={error}
        actions={<Button type="submit">Save</Button>}
      >
        <RecordPickerInput
          name="clientId"
          label="Client"
          options={CLIENTS}
          isRequired
          onChange={() => form.setValue("locationId", null)}
        />
        <RecordPickerInput
          name="locationId"
          label="Location"
          options={LOCATIONS.filter((location) => location.clientId === clientId)}
          isRequired
        />
      </Form>
    </FormProvider>
  );
}

async function choose(
  user: ReturnType<typeof renderWithProviders>["user"],
  label: RegExp,
  text: string,
) {
  const field = screen.getByRole("combobox", { name: label });
  await user.clear(field);
  await user.type(field, text);
  const [option] = within(screen.getByRole("listbox")).getAllByRole("option");
  if (!option) throw new Error(`no option for ${text}`);
  await user.click(option);
}

describe("RecordPickerInput", () => {
  it("requires a choice, and clears a dependent choice when its parent changes", async () => {
    const onSubmit = vi.fn();
    const { user, container } = renderWithProviders(<OrderForm onSubmit={onSubmit} />);
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Choose client.")).toBeTruthy();
    expect(screen.getByText("Choose location.")).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("combobox", { name: /Client/ }));
    await expectNoA11yViolations(container);

    await choose(user, /Client/, "north");
    await choose(user, /Location/, "annex");
    await choose(user, /Client/, "south");
    expect(screen.getByRole("combobox", { name: /Location/ })).toHaveProperty("value", "");
    await user.click(screen.getByRole("combobox", { name: /Location/ }));
    await user.keyboard("{ArrowDown}");
    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["South main office"]);
    await user.keyboard("{Escape}");

    await choose(user, /Location/, "main");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit).toHaveBeenCalledWith({ clientId: "c2", locationId: "l3" }, expect.anything());
  });

  it("shows the engine's field error on the picker", () => {
    const error = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [{ field: "clientId", code: "retired", message: "This client is retired." }],
    });
    renderWithProviders(<OrderForm onSubmit={() => undefined} error={error} />);
    expect(screen.getByText("This client is retired.")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: /Client/ }).getAttribute("aria-invalid")).toBe(
      "true",
    );
  });
});
