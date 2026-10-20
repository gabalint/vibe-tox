import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OrderDetail } from "@/generated/OrderDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRowActions } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { pickRecord } from "@/lib/test/picker";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import {
  ACCESSION_CONTEXT,
  ACCESSION_ID,
  CLIENTS,
  CONFIRM,
  ITEM_ID,
  ORDER,
  ORDER_ID,
  ORDERABLES,
  orderAccessionContext,
  PRACTITIONERS,
  SUBJECT_VIEW,
  sessionWith,
} from "../testData";
import { OrderDetailPage } from "./OrderDetailPage";
import { orderPath } from "./orderColumns";

const ACCEPTED: OrderDetail = {
  ...ORDER,
  status: "accepted",
  revision: 2,
  items: ORDER.items.map((item) => ({ ...item, state: "accepted" })),
  allowedActions: ["amend_evidence", "place_hold", "request_follow_on"],
};

const HOLD = {
  holdId: "hold-1",
  deficiency: "signature_or_intent_required" as const,
  origin: "user",
  state: "open",
  note: "Signature line empty",
  openedAt: "2026-10-12T14:10:00Z",
  resolvedAt: null,
  resolutionReason: null,
  revision: 1,
};

const POC_RESULT = {
  evidenceId: "poc-1",
  deviceOrTest: "Synthetic cup",
  analyteText: "SYN",
  resultText: "POS",
  performedAtText: null,
  source: "Clinic nurse",
  addedInVersion: 1,
  removedInVersion: null,
};

function renderPage(
  order: OrderDetail | (() => unknown),
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["enter_orders"],
  path = `/orders/${ORDER_ID}`,
) {
  const getOrder = vi.fn(typeof order === "function" ? order : () => order);
  mockCommands({
    get_order: getOrder,
    get_order_accession_context: () => orderAccessionContext({ items: [], allowedActions: [] }),
    get_subject: () => SUBJECT_VIEW,
    list_clients: () => CLIENTS,
    list_practitioners: () => PRACTITIONERS,
    list_active_orderables: () => ORDERABLES,
    current_session: () => sessionWith(capabilities),
    ...commands,
  });
  const result = renderRoutes([{ path: "/orders/:orderId", element: <OrderDetailPage /> }], path);
  return { ...result, getOrder };
}

describe("OrderDetailPage (REQ-ORD-001, REQ-ORD-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the requisition, tests and evidence as supplied, with names resolved", async () => {
    const { container, getOrder } = renderPage(ORDER);
    // The header is in place while the order loads.
    expect(screen.getByRole("heading", { level: 1, name: "Order" })).toBeTruthy();
    expect(screen.getByText("Loading order…")).toBeTruthy();
    expect(await screen.findByRole("heading", { level: 1, name: "Order PO-0001" })).toBeTruthy();
    expect(screen.getByText("Received, not yet accepted")).toBeTruthy();
    expect(getOrder).toHaveBeenCalledWith({ input: { orderId: ORDER_ID } });
    const breadcrumb = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(breadcrumb).getByRole("link", { name: "Orders" })).toBeTruthy();
    expect(await screen.findByText("Synthetic Clinic")).toBeTruthy();
    expect(screen.getByText("Doctor, Test (NPI 0000000000)")).toBeTruthy();
    expect(screen.getByText("PO-0001 (issued by SYN-CLINIC)")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "SYN-SCR Synthetic drug screen" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", { name: "Authorized: SYN-CNF Synthetic confirmation" }),
    ).toBeTruthy();
    // REQ-ORD-007: a medication list without an as-of date says so.
    expect(
      screen.getByText(/Synthetic medication A, reported by Ordering clinic, as-of date unknown/),
    ).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("says a legacy item's sequence role was not recorded instead of assuming one", async () => {
    renderPage({
      ...ORDER,
      items: ORDER.items.map((item) => ({ ...item, sequenceRole: null })),
    });
    expect(await screen.findByRole("gridcell", { name: "Not recorded" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "First-line test" })).toBeNull();
  });

  it("keeps receiving available when the current catalog cannot load", async () => {
    const { user } = renderPage(
      ACCEPTED,
      {
        get_order_accession_context: () => ACCESSION_CONTEXT,
        list_active_orderables: () =>
          Promise.reject(engineError("internal", "The current catalog is unavailable.")),
      },
      ["enter_orders", "accession"],
    );
    expect(await screen.findByText("The current catalog is unavailable.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Request add-on or reflex" })).toBeNull();
    await user.click(await screen.findByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}/accession`)).toBeTruthy();
  });

  it("accepts the order after confirmation, sending its revision", async () => {
    const accept = vi.fn(() => ACCEPTED);
    const { user } = renderPage(ORDER, { accept_order: accept });
    await user.click(await screen.findByRole("button", { name: "Accept order" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Accept this order?" });
    await user.click(within(dialog).getByRole("button", { name: "Accept order" }));
    expect(await screen.findByText("Order accepted")).toBeTruthy();
    expect(accept).toHaveBeenCalledWith({ input: { orderId: ORDER_ID, expectedRevision: 1 } });
  });

  it("says so when acceptance placed a hold instead", async () => {
    const { user } = renderPage(ORDER, {
      accept_order: () => ({
        ...ORDER,
        revision: 2,
        holds: [{ ...HOLD, origin: "standing_order_check" }],
      }),
    });
    await user.click(await screen.findByRole("button", { name: "Accept order" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Accept order" }));
    expect(await screen.findByText("The order was not accepted")).toBeTruthy();
  });

  it("refuses to reject without a reason, then rejects with it", async () => {
    const reject = vi.fn(() => ({ ...ORDER, status: "rejected", allowedActions: [] }));
    const { user } = renderPage(ORDER, { reject_order: reject });
    await user.click(await screen.findByRole("button", { name: "Reject order" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Reject this order?" });
    await user.click(within(dialog).getByRole("button", { name: "Reject order" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(reject).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Duplicate order");
    await user.click(within(dialog).getByRole("button", { name: "Reject order" }));
    expect(await screen.findByText("Order rejected")).toBeTruthy();
    expect(reject).toHaveBeenCalledWith({
      input: { orderId: ORDER_ID, expectedRevision: 1, reason: "Duplicate order" },
    });
  });

  it("places a clarification hold with the keyboard, and resolves one with a reason", async () => {
    const place = vi.fn(() => ({ ...ORDER, holds: [HOLD] }));
    const resolve = vi.fn(() => ORDER);
    const { user } = renderPage(
      { ...ORDER, holds: [HOLD] },
      {
        place_clarification_hold: place,
        resolve_clarification_hold: resolve,
      },
    );
    expect(await screen.findByText("Signature or intent to order required")).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: "Place clarification hold" }));
    await chooseWithKeyboard(user, /What needs clarifying$/, "Requestor missing");
    await user.click(screen.getByRole("button", { name: "Place hold" }));
    expect(await screen.findByText("Clarification hold placed")).toBeTruthy();
    expect(place).toHaveBeenCalledWith({
      input: {
        orderId: ORDER_ID,
        expectedRevision: 1,
        deficiency: "requestor_missing",
        note: null,
      },
    });

    await user.click(screen.getByRole("button", { name: "Resolve" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Resolve this hold?" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Signed copy faxed");
    await user.click(within(dialog).getByRole("button", { name: "Resolve hold" }));
    expect(await screen.findByText("Hold resolved")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: { holdId: "hold-1", expectedRevision: 1, reason: "Signed copy faxed" },
    });
  });

  // REQ-ORD-003: a reflex names the client's point-of-care result that triggered it.
  it("requests a reflex test from an accepted order, and decides a requested one", async () => {
    const requested = {
      ...ACCEPTED,
      evidence: { ...ACCEPTED.evidence, pointOfCareResults: [POC_RESULT] },
      items: [
        ...ACCEPTED.items,
        {
          itemId: "item-2",
          kind: "reflex" as const,
          state: "requested" as const,
          orderableVersionId: CONFIRM.id,
          orderableCode: CONFIRM.code,
          orderableName: CONFIRM.name,
          sequenceRole: "confirmation" as const,
          parentItemId: ITEM_ID,
          reflexAuthorization: "not_stated" as const,
          reflexScopeOrderableIds: [],
          authorityReference: null,
          reason: "Screen positive",
          revision: 1,
          components: [],
        },
      ],
    };
    const request = vi.fn(() => requested);
    const decide = vi.fn(() => requested);
    const { user } = renderPage(requested, {
      request_follow_on: request,
      decide_follow_on: decide,
    });
    await user.click(await screen.findByRole("button", { name: "Request add-on or reflex" }));
    await chooseWithKeyboard(
      user,
      /Kind of request$/,
      "Reflex, under an authorized test's reflex scope",
    );
    await pickRecord(user, /^Test/, "SYN-CNF");
    await chooseWithKeyboard(user, /Testing sequence role$/, "Confirmation");
    await chooseWithKeyboard(user, /Reflex from test$/, "SYN-SCR Synthetic drug screen");
    await chooseWithKeyboard(
      user,
      /Triggering point-of-care result$/,
      "Synthetic cup: SYN POS, reported by Clinic nurse",
    );
    const dialog = screen.getByRole("dialog", { name: "Request add-on or reflex" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Screen positive");
    await user.click(within(dialog).getByRole("button", { name: "Record request" }));
    expect(await screen.findByText("Request recorded; it awaits a decision")).toBeTruthy();
    expect(request).toHaveBeenCalledWith({
      input: {
        orderId: ORDER_ID,
        expectedRevision: 2,
        kind: "reflex",
        orderableVersionId: CONFIRM.id,
        sequenceRole: "confirmation",
        parentItemId: ITEM_ID,
        authorityReference: null,
        triggerSource: "client_point_of_care",
        triggerRef: null,
        pocResultId: "poc-1",
        reason: "Screen positive",
      },
    });

    // Only the requested test offers a decision, from its row's actions.
    const grid = screen.getByRole("grid", { name: "Tests on this order" });
    expect(
      within(grid).queryByRole("button", { name: "Actions for SYN-SCR Synthetic drug screen" }),
    ).toBeNull();
    expect(screen.queryByRole("checkbox")).toBeNull();
    const menu = await openRowActions(user, "SYN-CNF Synthetic confirmation");
    expect(within(menu).getByRole("menuitem", { name: "Accept request" })).toBeTruthy();
    await user.click(within(menu).getByRole("menuitem", { name: "Decline request" }));
    const decision = await screen.findByRole("alertdialog");
    await user.type(within(decision).getByRole("textbox", { name: /Reason/ }), "Not needed");
    await user.click(within(decision).getByRole("button", { name: "Decline request" }));
    expect(await screen.findByText("Request declined")).toBeTruthy();
    expect(decide).toHaveBeenCalledWith({
      input: { itemId: "item-2", expectedRevision: 1, decision: "decline", reason: "Not needed" },
    });
  });

  // Review finding 2: the public command accepts no laboratory-result trigger.
  it("offers no request decision on an initial test, which is decided with the order", async () => {
    renderPage(ORDER);
    const grid = await screen.findByRole("grid", { name: "Tests on this order" });
    expect(ORDER.items[0]).toMatchObject({ kind: "initial", state: "requested" });
    expect(
      within(grid).queryByRole("button", { name: "Actions for SYN-SCR Synthetic drug screen" }),
    ).toBeNull();
  });

  it("offers no laboratory-result trigger, and sends an add-on without one", async () => {
    const request = vi.fn(() => ACCEPTED);
    const { user } = renderPage(ACCEPTED, { request_follow_on: request });
    await user.click(await screen.findByRole("button", { name: "Request add-on or reflex" }));
    await chooseWithKeyboard(
      user,
      /Kind of request$/,
      "Reflex, under an authorized test's reflex scope",
    );
    const dialog = screen.getByRole("dialog", { name: "Request add-on or reflex" });
    expect(
      within(dialog).getByText("This order has no point-of-care result to trigger a reflex."),
    ).toBeTruthy();
    expect(within(dialog).queryByText(/laboratory result/i)).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: /Kind of request$/ }));
    await user.click(await screen.findByRole("option", { name: "Add-on, with new authority" }));
    await pickRecord(user, /^Test/, "SYN-CNF");
    await chooseWithKeyboard(user, /Testing sequence role$/, "Confirmation");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Clinic called");
    await user.click(within(dialog).getByRole("button", { name: "Record request" }));
    await screen.findByText("Request recorded; it awaits a decision");
    expect(request).toHaveBeenCalledWith({
      input: expect.objectContaining({
        kind: "add_on",
        parentItemId: null,
        triggerSource: null,
        triggerRef: null,
        pocResultId: null,
      }),
    });
  });

  it("offers accessioning on an accepted order only to people who accession", async () => {
    const { user } = renderPage(
      ACCEPTED,
      { get_order_accession_context: () => ACCESSION_CONTEXT },
      ["enter_orders", "accession"],
    );
    await user.click(await screen.findByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}/accession`)).toBeTruthy();
  });

  it("shows the first action as the main button, two more beside it and the rest under More", async () => {
    const { user } = renderPage({
      ...ORDER,
      allowedActions: ["accept", "place_hold", "request_follow_on", "reject"],
    });
    expect(await screen.findByRole("button", { name: "Accept order" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Place clarification hold" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Request add-on or reflex" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Reject order" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More" }));
    await user.click(await screen.findByRole("menuitem", { name: "Reject order" }));
    expect(await screen.findByRole("alertdialog", { name: "Reject this order?" })).toBeTruthy();
  });

  it("offers no decision the engine does not allow", async () => {
    renderPage({ ...ORDER, status: "rejected", allowedActions: [] }, {}, [
      "enter_orders",
      "accession",
    ]);
    expect(await screen.findByText("Rejected")).toBeTruthy();
    for (const name of [
      "Accept order",
      "Reject order",
      "Receive specimens",
      "Place clarification hold",
    ]) {
      expect(screen.queryByRole("button", { name })).toBeNull();
    }
  });

  it("keeps focus on Next while the next order of the list loads", async () => {
    const ids = [ORDER_ID, "order-2", "order-3"];
    mockCommands({
      get_order: (args) => {
        const index = ids.findIndex((id) => JSON.stringify(args).includes(id));
        return { ...ORDER, orderId: ids[index], placerOrderId: `PO-000${index + 1}` };
      },
      get_subject: () => SUBJECT_VIEW,
      list_clients: () => CLIENTS,
      list_practitioners: () => PRACTITIONERS,
      list_active_orderables: () => ORDERABLES,
      current_session: () => sessionWith(["enter_orders"]),
    });
    const { user } = await renderFromList({
      route: { path: "/orders/:orderId", element: <OrderDetailPage /> },
      recordPath: orderPath,
      ids,
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Order PO-0001" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Synthetic list" })).toBeTruthy();
    await expectNextKeepsFocus(user, "Order PO-0002");
  });

  it("shows the engine error with a retry", async () => {
    renderPage(() => Promise.reject(engineError("not_found", "The order was not found.")));
    expect(await screen.findByText("The order was not found.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Order" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
  it("keeps ordered test names when their catalog version is retired", async () => {
    renderPage(ORDER, { list_active_orderables: () => [] });
    expect(
      await screen.findByRole("gridcell", { name: "SYN-SCR Synthetic drug screen" }),
    ).toBeTruthy();
    expect(screen.queryByText("A test version no longer offered")).toBeNull();
  });

  it("continues a saved order to receiving only after explicit acceptance and engine eligibility", async () => {
    let current = ORDER;
    const accept = vi.fn(() => {
      current = ACCEPTED;
      return current;
    });
    const { user } = renderPage(
      () => current,
      {
        accept_order: accept,
        get_order_accession_context: () =>
          orderAccessionContext({
            allowedActions: current.status === "accepted" ? ["create_accession"] : [],
          }),
      },
      ["enter_orders", "accession"],
      `/orders/${ORDER_ID}?next=receive`,
    );
    expect(await screen.findByText("Order saved")).toBeTruthy();
    expect(accept).not.toHaveBeenCalled();
    await user.click(await screen.findByRole("button", { name: "Accept order" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Accept order" }),
    );
    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}/accession`)).toBeTruthy();
    expect(accept).toHaveBeenCalledTimes(1);
  });

  it("keeps the saved request and its new hold visible when acceptance cannot proceed", async () => {
    let current = ORDER;
    const { user } = renderPage(
      () => current,
      {
        accept_order: () => {
          current = { ...ORDER, revision: 2, holds: [HOLD], allowedActions: ["place_hold"] };
          return current;
        },
      },
      ["enter_orders", "accession"],
      `/orders/${ORDER_ID}?next=receive`,
    );
    await user.click(await screen.findByRole("button", { name: "Accept order" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Accept order" }),
    );
    expect(await screen.findByText("The order was not accepted")).toBeTruthy();
    expect(screen.getByText("Signature or intent to order required")).toBeTruthy();
    expect(screen.getByText("Order saved")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Receive specimens" })).toBeNull();
    expect(screen.queryByText(/Navigated to/)).toBeNull();
  });

  it("links existing accessions and explains why an item cannot be received again", async () => {
    const { user } = renderPage(
      ACCEPTED,
      {
        get_order_accession_context: () =>
          orderAccessionContext({
            allowedActions: [],
            items: ACCESSION_CONTEXT.items.map((item) => ({
              ...item,
              eligible: false,
              reason: "Already accessioned",
              accessionId: ACCESSION_ID,
              accessionNumber: "A26000001",
            })),
            accessions: [
              {
                accessionId: ACCESSION_ID,
                accessionNumber: "A26000001",
                createdAt: "2026-10-12T14:15:00Z",
                specimenCount: 1,
                pendingReceiptCount: 1,
              },
            ],
          }),
      },
      ["enter_orders", "accession"],
    );
    expect(await screen.findByText(/Receipt review needed: 1/)).toBeTruthy();
    expect(screen.getByText(/Already accessioned/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Receive specimens" })).toBeNull();
    await user.click(
      screen.getAllByRole("link", { name: "Accession A26000001" })[0] ?? document.body,
    );
    expect(await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`)).toBeTruthy();
  });

  it("does not guess receiving eligibility when its context fails", async () => {
    renderPage(
      ACCEPTED,
      {
        get_order_accession_context: () =>
          Promise.reject(engineError("internal", "Receiving status is unavailable.")),
      },
      ["enter_orders", "accession"],
    );
    expect(await screen.findByText("Receiving status is unavailable.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Receive specimens" })).toBeNull();
    expect(screen.getByRole("gridcell", { name: "SYN-SCR Synthetic drug screen" })).toBeTruthy();
  });
});
