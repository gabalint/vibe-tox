import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OrderDetail } from "@/generated/OrderDetail";
import { checked } from "@/lib/choices";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ORDER, sessionWith } from "../testData";
import { OrderMaintenance } from "./OrderMaintenance";

const ACCEPTED: OrderDetail = {
  ...ORDER,
  status: "accepted",
  items: ORDER.items.map((item) => ({ ...item, state: "accepted" })),
  allowedActions: ["amend_evidence"],
};
function renderMaintenance(
  order = ACCEPTED,
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({ current_session: () => sessionWith(["enter_orders"]), ...commands });
  return renderRoutes([{ path: "/", element: <OrderMaintenance order={order} /> }], "/");
}

describe("Order maintenance mocked IPC", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);
  it("requires explicit evidence supersession and preserves exact newly supplied medication", async () => {
    const amend = vi.fn(() => ACCEPTED);
    const { user } = renderMaintenance(ACCEPTED, { amend_order_evidence: amend });
    await user.click(await screen.findByRole("button", { name: "Amend supplied evidence" }));
    const dialog = await screen.findByRole("dialog", { name: "Amend supplied evidence" });
    await chooseWithKeyboard(user, /Authority to test$/, "Not supplied");
    await user.click(within(dialog).getByRole("button", { name: "Add medication" }));
    await user.type(
      within(dialog).getByRole("textbox", { name: /Medication as written/ }),
      "Synthetic supplied medication B",
    );
    await chooseWithKeyboard(user, /Reported by$/, "Ordering clinic");
    await user.click(
      within(dialog).getByRole("checkbox", { name: /Medication: Synthetic medication A/ }),
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason and source for amendment/ }),
      "Corrected clinic list, synthetic source 42",
    );
    await user.click(within(dialog).getByRole("button", { name: "Save evidence amendment" }));
    expect(await screen.findByText("Evidence amendment saved")).toBeTruthy();
    expect(amend).toHaveBeenCalledWith({
      input: {
        orderId: ACCEPTED.orderId,
        expectedRevision: ACCEPTED.revision,
        removeEvidenceIds: ["ev-2"],
        reason: "Corrected clinic list, synthetic source 42",
        add: {
          authority: { kind: "not_supplied", documentRef: null },
          diagnoses: [],
          medications: [
            {
              receivedText: "Synthetic supplied medication B",
              code: null,
              codeSystem: null,
              source: "ordering_clinic",
              reporter: null,
              providedOn: null,
              asOfDate: null,
            },
          ],
          pointOfCareResults: [],
        },
      },
    });
  });
  it("records a cancellation request without treating it as the cancellation decision", async () => {
    const request = vi.fn(() => ACCEPTED);
    const { user } = renderMaintenance(ACCEPTED, { request_cancellation: request });
    await user.click(await screen.findByRole("button", { name: "Request cancellation" }));
    const dialog = await screen.findByRole("dialog", { name: "Request test cancellation" });
    const item = checked(ACCEPTED.items[0] ?? null);
    await chooseWithKeyboard(
      user,
      /Test to cancel$/,
      `${item.orderableCode} ${item.orderableName}`,
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Requester as supplied/ }),
      "Synthetic ordering clinician",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason/ }),
      "Duplicate supplied request",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record cancellation request" }));
    expect(await screen.findByText("Cancellation requested")).toBeTruthy();
    expect(request).toHaveBeenCalledWith({
      input: {
        itemId: item.itemId,
        requestedBy: "Synthetic ordering clinician",
        reason: "Duplicate supplied request",
      },
    });
  });
  it("requires an explicit accept/decline and submits the displayed request revision", async () => {
    const request = {
      requestId: "cancellation-1",
      itemId: checked(ACCEPTED.items[0] ?? null).itemId,
      requestedBy: "Synthetic clinic",
      reason: "Requested cancellation",
      state: "requested",
      decisionReason: null,
      revision: 4,
    };
    const decide = vi.fn(() => ACCEPTED);
    const { user } = renderMaintenance(
      { ...ACCEPTED, cancellations: [request] },
      { decide_cancellation: decide },
    );
    await user.click(await screen.findByRole("button", { name: "Decide cancellation" }));
    const dialog = await screen.findByRole("dialog", { name: "Decide cancellation" });
    await user.click(within(dialog).getByRole("button", { name: "Record decision" }));
    expect(decide).not.toHaveBeenCalled();
    await chooseWithKeyboard(user, /Decision$/, "Decline cancellation");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Decision reason/ }),
      "Source requested continued testing",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record decision" }));
    expect(await screen.findByText("Cancellation decision recorded")).toBeTruthy();
    expect(decide).toHaveBeenCalledWith({
      input: {
        requestId: "cancellation-1",
        expectedRevision: 4,
        decision: "decline",
        reason: "Source requested continued testing",
      },
    });
  });
});
