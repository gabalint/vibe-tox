import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { OrderBillingView } from "@/generated/OrderBillingView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { pickRecord } from "@/lib/test/picker";
import { renderWithProviders } from "@/lib/test/render";
import { renderRoutes } from "@/lib/test/router";
import { ORDER_ID, SUBJECT_ID } from "../testData";
import { BillingOrderPage } from "./BillingOrderPage";
import { ContactDialog } from "./ContactDialog";
import { EMPTY_EVIDENCE, evidenceInput } from "./evidenceValues";
import { OrderContextDialog } from "./OrderContextDialog";
import { BILLING, PROFILE } from "./testData";

describe("Financial intake (BILL-G01, BILL-G02)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("retains other financial facts and the open form when a profile revision is stale", async () => {
    const save = vi.fn(() =>
      Promise.reject(
        engineError("conflict", "The patient billing profile changed. Reload before saving."),
      ),
    );
    const close = vi.fn();
    mockCommands({ set_patient_billing_profile: save });
    const { user, container } = renderWithProviders(
      <ContactDialog view={PROFILE} onClose={close} />,
    );
    await user.type(screen.getByRole("textbox", { name: "ZIP or postal code" }), "00123");
    await user.type(
      screen.getByRole("textbox", { name: /Information source/ }),
      "Synthetic correction",
    );
    await user.type(
      screen.getByRole("textbox", { name: /Reason for change/ }),
      "Correct supplied contact",
    );
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Save billing details" }));
    expect(
      await screen.findByText("The patient billing profile changed. Reload before saving."),
    ).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: expect.objectContaining({
        subjectId: SUBJECT_ID,
        expectedVersion: 4,
        content: {
          ...PROFILE.profile?.content,
          contact: expect.objectContaining({ postalCode: "00123" }),
        },
      }),
    });
    expect(screen.getByRole("textbox", { name: "ZIP or postal code" })).toHaveProperty(
      "value",
      "00123",
    );
    expect(close).not.toHaveBeenCalled();
  });

  it("selects a profile revision and coverage payment priority explicitly", async () => {
    const save = vi.fn(() => ({}));
    mockCommands({ get_patient_billing_profile: () => PROFILE, set_order_billing_context: save });
    const { user } = renderWithProviders(
      <OrderContextDialog view={BILLING} subjectId={SUBJECT_ID} onClose={() => undefined} />,
    );
    await screen.findByRole("dialog", { name: "Confirm order billing selection" });
    expect(screen.queryByRole("combobox", { name: "Coverage priority 1" })).toBeNull();
    await chooseWithKeyboard(user, /Who is responsible for payment/, "Third party (insurance)");
    await chooseWithKeyboard(
      user,
      /Patient financial facts to use/,
      "Profile revision 4 — Synthetic intake",
    );
    expect(screen.queryByRole("combobox", { name: "Coverage priority 1" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Add coverage to order" }));
    await pickRecord(user, /Coverage priority 1/, "Synthetic Beta");
    await user.click(screen.getByRole("button", { name: "Add coverage to order" }));
    await pickRecord(user, /Coverage priority 2/, "Synthetic Alpha");
    await user.type(
      screen.getByRole("textbox", { name: /Information source/ }),
      "Synthetic order confirmation",
    );
    await user.type(screen.getByRole("textbox", { name: /Reason for change/ }), "Confirm priority");
    await user.click(screen.getByRole("button", { name: "Save order billing selection" }));
    expect(await screen.findByText("Order billing selection saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        orderId: ORDER_ID,
        expectedRevision: 7,
        expectedBillingVersion: null,
        responsibility: "third_party",
        clientId: null,
        profileVersion: 4,
        coverageKeys: ["coverage-1", "coverage-0"],
        useProfileGuarantor: false,
        financialEvidence: [],
        source: "Synthetic order confirmation",
        reason: "Confirm priority",
      },
    });
  });

  it("opens a financial-only order without requesting clinical patient or order records", async () => {
    const get = vi.fn(() => BILLING);
    mockCommands({ get_order_billing: get });
    const { user } = renderRoutes(
      [{ path: "/billing/orders/:orderId", element: <BillingOrderPage /> }],
      `/billing/orders/${ORDER_ID}`,
    );
    expect(await screen.findByText("No order-specific billing selection confirmed.")).toBeTruthy();
    expect(get).toHaveBeenCalledWith({ input: { orderId: ORDER_ID } });
    await user.click(screen.getByRole("button", { name: "Open billing follow-up" }));
    const dialog = await screen.findByRole("dialog", { name: "Open billing follow-up" });
    expect(within(dialog).getByRole("button", { name: /Missing financial evidence/ })).toBeTruthy();
    expect(screen.queryByText(/could not be loaded/)).toBeNull();
  });

  it("can retire coverage-specific evidence when correcting an order to self-pay", async () => {
    const evidence = evidenceInput(
      {
        ...EMPTY_EVIDENCE,
        kind: "eligibility",
        status: "recorded",
        sourceRef: "Synthetic payer check",
        scope: "Synthetic coverage check",
        occurredAt: "2026-10-14T12:00:00Z",
        coverageKey: "coverage-0",
      },
      "evidence-1",
    );
    const view: OrderBillingView = {
      ...BILLING,
      context: {
        orderId: ORDER_ID,
        subjectId: SUBJECT_ID,
        version: 2,
        orderVersion: 7,
        selection: {
          responsibility: "third_party",
          clientId: null,
          profileVersion: 4,
          contact: PROFILE.profile?.content.contact ?? null,
          guarantor: null,
          coverages: PROFILE.profile?.content.coverages ?? [],
        },
        financialEvidence: [evidence],
        source: "Synthetic original",
        reason: "Original selection",
        recordedBy: "synthetic-user",
        recordedAt: "2026-10-14T12:00:00Z",
      },
    };
    const save = vi.fn(() => ({}));
    mockCommands({ get_patient_billing_profile: () => PROFILE, set_order_billing_context: save });
    const { user } = renderWithProviders(
      <OrderContextDialog view={view} subjectId={SUBJECT_ID} onClose={() => undefined} />,
    );
    await screen.findByRole("dialog", { name: "Correct order billing selection" });
    await chooseWithKeyboard(user, /Who is responsible for payment/, "Patient (self-pay)");
    expect(screen.queryByRole("combobox", { name: /Coverage priority/ })).toBeNull();
    await user.click(
      screen.getByRole("checkbox", { name: /eligibility — Synthetic coverage check/ }),
    );
    await user.type(
      screen.getByRole("textbox", { name: /Information source/ }),
      "Synthetic corrected request",
    );
    await user.type(
      screen.getByRole("textbox", { name: /Reason for change/ }),
      "Patient selected self-pay; old coverage evidence no longer applies",
    );
    await user.click(screen.getByRole("button", { name: "Save order billing selection" }));
    expect(await screen.findByText("Order billing selection saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: expect.objectContaining({
        expectedBillingVersion: 2,
        responsibility: "patient_self_pay",
        coverageKeys: [],
        financialEvidence: [],
      }),
    });
    expect(view.context?.financialEvidence).toEqual([evidence]);
  });
});
