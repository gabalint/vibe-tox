import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION_ID, UNROUTED } from "../testData";
import { UnroutedPage } from "./UnroutedPage";

const ROUTES = [{ path: "/interfaces/unrouted", element: <UnroutedPage /> }];

function renderPage(handlers: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => ADMIN,
    list_unrouted_partner_messages: () => [UNROUTED],
    list_work_queues: () => [],
    ...handlers,
  });
  return renderRoutes(ROUTES, "/interfaces/unrouted");
}

describe("Unrouted messages (ticket #4555; REQ-RPT-003)", () => {
  beforeEach(mockGridLayout);

  it("shows why a message matched no connection and sends it once", async () => {
    const resolve = vi.fn(() => null);
    const { user, container } = renderPage({ resolve_unrouted_message: resolve });
    expect(await screen.findByRole("gridcell", { name: "TEST CLIENT 2" })).toBeTruthy();
    // The oldest message's details show below the table, as in the mockup.
    const details = await screen.findByRole("region", {
      name: "Billing handoff 14 – Billing JSON message",
    });
    expect(within(details).getByText("Client equals CLINIC-A (did not match)")).toBeTruthy();
    expect(
      within(details).getByRole("button", { name: "Add a rule for TEST CLIENT 2" }),
    ).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    // The closest connection is the one offered first.
    await user.click(within(details).getByRole("button", { name: "Send once" }));
    await waitFor(() =>
      expect(resolve).toHaveBeenCalledWith({
        input: {
          messageId: UNROUTED.messageId,
          decision: { action: "send_once", connection: CONNECTION_ID },
        },
      }),
    );
  });

  it("dismisses a message only with a reason", async () => {
    const resolve = vi.fn(() => null);
    const { user } = renderPage({ resolve_unrouted_message: resolve });
    await openRow(user, "Billing handoff 14");
    await user.click(await screen.findByRole("button", { name: "Dismiss with reason" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Dismiss message" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Sent by mail");
    await user.click(within(dialog).getByRole("button", { name: "Dismiss" }));
    await waitFor(() =>
      expect(resolve).toHaveBeenCalledWith({
        input: {
          messageId: UNROUTED.messageId,
          decision: { action: "dismiss", reason: "Sent by mail" },
        },
      }),
    );
  });

  it("explains an empty queue", async () => {
    renderPage({ list_unrouted_partner_messages: () => [] });
    expect(await screen.findByText("No unrouted messages")).toBeTruthy();
  });
});
