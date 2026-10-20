import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ACCESSION_CONTEXT, ORDER_ID, sessionWith } from "./testData";
import { useOrderAccessionContext } from "./useOrderAccessionContext";

function ContextProbe({ orderId }: { orderId: string }) {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const context = useOrderAccessionContext(orderId);
  if (session.isPending) return <p>Loading session</p>;
  return <p>{context.data ? "Context loaded" : "No context"}</p>;
}

describe("Order accession context", () => {
  it.each(["enter_orders", "accession"])(
    "loads for an explicit order with %s",
    async (capability) => {
      const read = vi.fn(() => ACCESSION_CONTEXT);
      mockCommands({
        current_session: () => sessionWith([capability]),
        get_order_accession_context: read,
      });
      renderWithProviders(<ContextProbe orderId={ORDER_ID} />);
      await screen.findByText("Context loaded");
      expect(read).toHaveBeenCalledWith({ input: { orderId: ORDER_ID } });
    },
  );

  it.each([
    { capabilities: [], orderId: ORDER_ID },
    { capabilities: ["accession"], orderId: "" },
  ])(
    "does not issue an incidental audited read for $orderId",
    async ({ capabilities, orderId }) => {
      const session = vi.fn(() => sessionWith(capabilities));
      const read = vi.fn(() => ACCESSION_CONTEXT);
      mockCommands({ current_session: session, get_order_accession_context: read });
      renderWithProviders(<ContextProbe orderId={orderId} />);
      await screen.findByText("No context");
      expect(read).not.toHaveBeenCalled();
    },
  );
});
