import { screen, within } from "@testing-library/react";
import { FormProvider, useForm } from "react-hook-form";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { checked } from "@/lib/choices";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { pickRecord } from "@/lib/test/picker";
import { renderRoutes } from "@/lib/test/router";
import { CLIENTS, ORDERABLES, PRACTITIONERS, SCREEN, SUBJECT_ID, sessionWith } from "../testData";
import { EMPTY_ORDER, type OrderFormValues } from "./orderValues";
import { StandingOrderChoice, StandingOrders } from "./StandingOrders";

const practitioner = checked(PRACTITIONERS[0] ?? null);
const AUTHORITY = {
  standingOrderId: "standing-synthetic",
  subjectId: SUBJECT_ID,
  practitionerId: practitioner.id,
  validFrom: "2020-01-01",
  validUntil: "2020-12-31",
  visitLimit: 2,
  acceptedUses: 1,
  documentRef: "Synthetic patient authority",
  recordedAt: "2026-10-14T10:00:00Z",
  scopeOrderableIds: [SCREEN.orderableId],
};
function mocks(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["enter_orders"]),
    list_clients: () => CLIENTS,
    list_practitioners: () => PRACTITIONERS,
    list_active_orderables: () => ORDERABLES,
    list_standing_orders: () => [AUTHORITY],
    ...commands,
  });
}
function Selection({ read }: { read: (values: OrderFormValues) => void }) {
  const form = useForm<OrderFormValues>({
    defaultValues: { ...EMPTY_ORDER, practitionerId: practitioner.id },
  });
  return (
    <FormProvider {...form}>
      <Form
        aria-label="Standing selection"
        onSubmit={(e) => e.preventDefault()}
        actions={<Button onPress={() => read(form.getValues())}>Read supplied choices</Button>}
      >
        <StandingOrderChoice subjectId={SUBJECT_ID} />
      </Form>
    </FormProvider>
  );
}
describe("Standing authority mocked IPC", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);
  it("offers earlier collection authority without auto-selecting tests or substituting intake dates", async () => {
    mocks();
    const read = vi.fn();
    const { user } = renderRoutes([{ path: "/", element: <Selection read={read} /> }], "/");
    const picker = await screen.findByRole("combobox", { name: /Patient-specific standing order/ });
    expect(picker).toHaveProperty("value", "");
    await user.click(screen.getByRole("button", { name: "Read supplied choices" }));
    expect(read.mock.calls[0]?.[0]).toMatchObject({ standingOrderId: null, items: [] });
    await pickRecord(user, /Patient-specific standing order/, "Synthetic patient authority");
    await user.click(screen.getByRole("button", { name: "Read supplied choices" }));
    expect(read.mock.calls.at(-1)?.[0]).toMatchObject({
      standingOrderId: AUTHORITY.standingOrderId,
      practitionerId: practitioner.id,
      items: [],
    });
    expect(
      screen.getByText(/Delayed intake does not expire valid earlier collection/),
    ).toBeTruthy();
  });
  it("preserves supplied patient scope and dates and refuses a non-finite visit limit", async () => {
    const save = vi.fn(() => AUTHORITY);
    mocks({ register_standing_order: save });
    const { user } = renderRoutes(
      [{ path: "/", element: <StandingOrders subjectId={SUBJECT_ID} /> }],
      "/",
    );
    await user.click(await screen.findByRole("button", { name: "Record standing authority" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Record patient-specific standing authority",
    });
    await pickRecord(user, /Ordering practitioner/, practitioner.familyName);
    await user.click(
      within(dialog).getByRole("checkbox", { name: `${SCREEN.code} ${SCREEN.name}` }),
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /First valid collection date/ }),
      "2020-01-01",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Last valid collection date/ }),
      "2020-12-31",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Source approval reference/ }),
      AUTHORITY.documentRef,
    );
    const limit = within(dialog).getByRole("textbox", { name: /Visit limit/ });
    await user.type(limit, "Infinity");
    await user.click(within(dialog).getByRole("button", { name: "Record standing authority" }));
    expect(save).not.toHaveBeenCalled();
    expect(limit.getAttribute("aria-invalid")).toBe("true");
    await user.clear(limit);
    await user.type(limit, "2");
    await user.click(within(dialog).getByRole("button", { name: "Record standing authority" }));
    expect(await screen.findByText("Standing authority recorded")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        subjectId: SUBJECT_ID,
        practitionerId: practitioner.id,
        scopeOrderableIds: [SCREEN.orderableId],
        validFrom: "2020-01-01",
        validUntil: "2020-12-31",
        visitLimit: 2,
        documentRef: AUTHORITY.documentRef,
      },
    });
  });
});
