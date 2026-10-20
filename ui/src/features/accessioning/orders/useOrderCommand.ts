import type { CommandName } from "@/generated/CommandName";
import type { OrderDetail } from "@/generated/OrderDetail";
import { useCommandMutation } from "@/lib/commands";
import { ORDER_QUERIES } from "./contract";

/** An order command: it answers with the updated order and refreshes the order views. */
export function useOrderCommand<Input>(command: CommandName) {
  return useCommandMutation<OrderDetail, { input: Input }>(command, {
    invalidates: ORDER_QUERIES,
  });
}
