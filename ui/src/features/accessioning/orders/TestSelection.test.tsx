import { screen } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { CONFIRM, LONG_CATALOG, SCREEN } from "../testData";
import { EMPTY_ORDER, type OrderFormValues } from "./orderValues";
import { TestSelection } from "./TestSelection";

function Draft({ onRead }: { onRead: (items: OrderFormValues["items"]) => void }) {
  const form = useForm<OrderFormValues>({ defaultValues: EMPTY_ORDER });
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Synthetic test selection"
        onSubmit={(event) => event.preventDefault()}
        actions={<Button onPress={() => onRead(form.getValues("items"))}>Read draft</Button>}
      >
        <TestSelection orderables={LONG_CATALOG} />
      </Form>
    </FormProvider>
  );
}

describe("TestSelection (REQ-ORD-003, REQ-ORD-005)", () => {
  it("keeps supplied sequence and reflex scope when an earlier selected test is removed", {
    timeout: WALKTHROUGH_TIMEOUT_MS,
  }, async () => {
    mockGridLayout();
    const onRead = vi.fn();
    const { user } = renderWithProviders(<Draft onRead={onRead} />);
    const picker = screen.getByRole("combobox", { name: "Find tests" });
    await user.type(picker, "SYN-SCR");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Not authorized");

    await user.type(picker, "SYN-CNF");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    await chooseWithKeyboard(user, /Testing sequence role for SYN-CNF/, "Confirmation");
    await chooseWithKeyboard(user, /Reflex testing for SYN-CNF/, "Authorized");
    await user.type(
      screen.getByRole("combobox", {
        name: "Find reflex tests for SYN-CNF Synthetic confirmation",
      }),
      "SYN-SCR",
    );
    await user.keyboard("{ArrowDown}{Enter}{Escape}");

    await user.type(picker, "SYN-T10");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    await user.click(screen.getByRole("button", { name: "Remove SYN-SCR Synthetic drug screen" }));
    expect(document.activeElement).toBe(picker);
    expect(
      screen.getByRole("button", { name: /Testing sequence role for SYN-CNF/ }).textContent,
    ).toContain("Confirmation");
    expect(
      screen.getByRole("button", { name: /Reflex testing for SYN-CNF/ }).textContent,
    ).toContain("Authorized");
    await user.click(screen.getByRole("button", { name: "Read draft" }));
    expect(onRead).toHaveBeenLastCalledWith([
      {
        orderableVersionId: CONFIRM.id,
        sequenceRole: "confirmation",
        reflexAuthorization: "authorized",
        reflexScopeOrderableIds: [SCREEN.orderableId],
      },
      {
        orderableVersionId: LONG_CATALOG[2]?.id,
        sequenceRole: null,
        reflexAuthorization: null,
        reflexScopeOrderableIds: [],
      },
    ]);
  });
});
