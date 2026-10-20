import type { CommandName } from "@/generated/CommandName";
import type { OrderAccessionContext } from "@/generated/OrderAccessionContext";
import { useCommandQuery, useHasCapability } from "@/lib/commands";

/** Audited order-to-material context; eligibility and actions come from the engine. */
export const ORDER_ACCESSION_CONTEXT = "get_order_accession_context" satisfies CommandName;

/** Load only for an explicitly opened order and a session allowed to read intake records. */
export function useOrderAccessionContext(orderId: string) {
  const canEnterOrders = useHasCapability("enter_orders");
  const canAccession = useHasCapability("accession");
  return useCommandQuery<OrderAccessionContext>(
    ORDER_ACCESSION_CONTEXT,
    { input: { orderId } },
    { enabled: orderId.length > 0 && (canEnterOrders || canAccession) },
  );
}
