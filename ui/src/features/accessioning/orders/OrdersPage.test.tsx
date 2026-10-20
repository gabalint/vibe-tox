import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OrderSummary } from "@/generated/OrderSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ORDER_ID, SUBJECT_ID, SUBJECT_VIEW, sessionWith } from "../testData";
import { OrdersPage } from "./OrdersPage";

const SUMMARY: OrderSummary = {
  orderId: ORDER_ID,
  subjectId: SUBJECT_ID,
  patientFamilyName: "Testpatient",
  patientGivenNames: "Jane",
  patientBirthDate: "1985-02-15",
  patientIdentifiers: [],
  placerOrderId: "PO-SYN-1000",
  clientName: "Synthetic Family Clinic",
  itemCount: 2,
  status: "received",
  receivedAt: "2026-10-12T14:05:00Z",
  version: 1,
  revision: 1,
  openHoldCount: 1,
};

function summary(index: number): OrderSummary {
  const id = `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`;
  return { ...SUMMARY, orderId: id, placerOrderId: `PO-SYN-${index}`, openHoldCount: 0 };
}

function renderPage(listOrders: (args: unknown) => unknown, path = "/orders") {
  // The patient's name in the banner links to the patient for people who enter orders.
  mockCommands({
    list_orders: listOrders,
    get_subject: () => SUBJECT_VIEW,
    current_session: () => sessionWith(["enter_orders"]),
  });
  return renderRoutes([{ path: "/orders", element: <OrdersPage /> }], path);
}

describe("OrdersPage (REQ-ORD-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it.each(["/orders", "/orders?filter=unknown"])(
    "opens %s with all orders, including accepted orders",
    async (path) => {
      const list = vi.fn(() => [{ ...SUMMARY, status: "accepted" }]);
      renderPage(list, path);
      expect(await screen.findByRole("gridcell", { name: "Accepted" })).toBeTruthy();
      expect(screen.getByText("1 order shown")).toBeTruthy();
      expect(screen.queryByRole("button", { name: "Clear filter" })).toBeNull();
      expect(list).toHaveBeenCalledWith({
        input: {
          filter: "all",
          subjectId: null,
          limit: 200,
          beforeReceivedAt: null,
          beforeOrderId: null,
        },
      });
    },
  );

  it("names each order by patient, order number, client and tests, and opens it with Enter", async () => {
    const list = vi.fn(() => [SUMMARY, { ...summary(2), placerOrderId: null }]);
    const { user, container } = renderPage(list);
    expect(screen.getByText("Loading orders…")).toBeTruthy();
    const grid = await screen.findByRole("grid", { name: "Orders" });
    const headers = within(grid)
      .getAllByRole("columnheader")
      .map((header) => header.textContent);
    expect(headers.slice(0, 6)).toEqual([
      "Patient",
      "Birth date",
      "Patient identifiers",
      "Order number",
      "Client",
      "Tests",
    ]);
    const row = within(grid).getByRole("gridcell", { name: "PO-SYN-1000" }).closest("[role=row]");
    if (!(row instanceof HTMLElement)) throw new Error("no row");
    expect(within(row).getByRole("gridcell", { name: "Testpatient, Jane" })).toBeTruthy();
    expect(within(row).getByText("Synthetic Family Clinic")).toBeTruthy();
    expect(within(row).getByText("2026-10-12 14:05 UTC")).toBeTruthy();
    expect(within(row).getByText("1 open")).toBeTruthy();
    expect(within(grid).getByText("Not supplied")).toBeTruthy();
    // Stable ID continuation reaches later orders even when timestamps tie.
    expect(list).toHaveBeenCalledWith({
      input: {
        filter: "all",
        subjectId: null,
        limit: 200,
        beforeReceivedAt: null,
        beforeOrderId: null,
      },
    });
    expect(screen.queryByText(/Showing the newest/)).toBeNull();
    expect(screen.getByText("2 orders shown")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await openRowWithEnter(user, "PO-SYN-1000");
    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}`)).toBeTruthy();
  });

  it("opens an order with a single click, passing the list for the breadcrumb and Previous and Next", async () => {
    const second = summary(2);
    const { user, router } = renderPage(() => [SUMMARY, second], "/orders?filter=all");
    // The patient is the identifier column, styled as a link but not a link of its own.
    expect(screen.queryByRole("link", { name: "Testpatient, Jane" })).toBeNull();
    await openRow(user, "PO-SYN-2");
    expect(await screen.findByText(`Navigated to /orders/${second.orderId}`)).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Orders", href: "/orders?filter=all" },
        ids: [ORDER_ID, second.orderId],
      },
    });
  });

  it("continues beyond a full page at equal timestamps without losing the last order", async () => {
    const firstPage = Array.from({ length: 200 }, (_, index) => summary(index + 1));
    const last = firstPage.at(-1);
    if (!last) throw new Error("Expected a full synthetic order page");
    const list = vi.fn((args: unknown) => {
      const input = (args as { input: { beforeOrderId: string | null } }).input;
      return input.beforeOrderId ? [summary(201)] : firstPage;
    });
    const { user } = renderPage(list);
    await screen.findByRole("grid", { name: "Orders" });
    expect(screen.getByText("200 orders shown · more available")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load older orders" }));
    await vi.waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          filter: "all",
          subjectId: null,
          limit: 200,
          beforeReceivedAt: null,
          beforeOrderId: last.orderId,
        },
      }),
    );
    expect(await screen.findByText("201 orders shown")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load older orders" })).toBeNull();
    expect(screen.getByRole("grid", { name: "Orders" }).getAttribute("aria-rowcount")).toBe("202");
  });

  it("refreshes on request and says when the list was read", async () => {
    const list = vi.fn(() => [SUMMARY]);
    const { user } = renderPage(list);
    await screen.findByRole("grid", { name: "Orders" });
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("keeps the filter in the address and lists one patient's orders", async () => {
    const list = vi.fn(() => [SUMMARY]);
    const { user, router } = renderPage(list, `/orders?subjectId=${SUBJECT_ID}`);
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    await chooseWithKeyboard(user, /Show orders$/, "Accepted");
    expect(router.state.location.search).toBe(`?subjectId=${SUBJECT_ID}&filter=accepted`);
    expect(list).toHaveBeenLastCalledWith({
      input: {
        filter: "accepted",
        subjectId: SUBJECT_ID,
        limit: 200,
        beforeReceivedAt: null,
        beforeOrderId: null,
      },
    });
    await user.click(screen.getByRole("button", { name: "Clear filter" }));
    expect(router.state.location.search).toBe(`?subjectId=${SUBJECT_ID}&filter=all`);
    await vi.waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          filter: "all",
          subjectId: SUBJECT_ID,
          limit: 200,
          beforeReceivedAt: null,
          beforeOrderId: null,
        },
      }),
    );
  });

  it("explains the first step when there are no orders and opens patient selection", async () => {
    const { user } = renderPage(() => []);
    expect(await screen.findByText("No orders yet")).toBeTruthy();
    expect(
      screen.getByText("Use New order to choose a patient and enter their order."),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "New order" }));
    expect(await screen.findByText("Navigated to /orders/new")).toBeTruthy();
  });

  it("offers a new order for the scoped patient and can return to all patients", async () => {
    const { user, router } = renderPage(() => [], `/orders?subjectId=${SUBJECT_ID}`);
    expect(await screen.findByText("No orders for this patient yet")).toBeTruthy();
    expect(screen.getByText("Use New order to enter an order for this patient.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Show orders for all patients" }));
    expect(router.state.location.search).toBe("");
    expect(await screen.findByText("No orders yet")).toBeTruthy();
  });

  it("starts a new order with the scoped patient", async () => {
    const { user } = renderPage(() => [], `/orders?subjectId=${SUBJECT_ID}`);
    await screen.findByText("No orders for this patient yet");
    await user.click(screen.getByRole("button", { name: "New order" }));
    expect(
      await screen.findByText(`Navigated to /orders/new?subjectId=${SUBJECT_ID}`),
    ).toBeTruthy();
  });

  it("says when no order needs a decision, without calling it a search, and shows all", async () => {
    const list = vi.fn(() => []);
    const { user, router } = renderPage(list, "/orders?filter=needs_decision");
    expect(await screen.findByText("No orders need a decision")).toBeTruthy();
    expect(screen.queryByText(/match/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show all orders" }));
    expect(router.state.location.search).toBe("?filter=all");
    expect(await screen.findByText("No orders yet")).toBeTruthy();
  });

  it("says who can help, without a retry, when the roles do not allow the list", async () => {
    renderPage(() => Promise.reject(engineError("forbidden", "You cannot view orders.")));
    expect(await screen.findByText("You cannot view orders.")).toBeTruthy();
    expect(screen.getByText(/A laboratory administrator can change roles/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "Orders" })).toBeTruthy();
  });

  it("retries after another engine error", async () => {
    const list = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "The orders could not be read."))
      .mockReturnValue([SUMMARY]);
    const { user } = renderPage(list);
    await user.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("grid", { name: "Orders" })).toBeTruthy();
  });
});
