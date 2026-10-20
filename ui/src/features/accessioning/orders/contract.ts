// Order entry and decisions (REQ-ORD-001, REQ-ORD-005, REQ-ORD-007; design brief §4). Every
// command needs `enter_orders`; changes carry the order's or item's `revision` and fail with
// `conflict` when stale. Each order command answers with the updated `OrderDetail`. Types are
// generated from `lis-orders` into `@/generated`: records carry IDs, and screens show names
// from the client, practitioner and catalog lists.
import type { CommandName } from "@/generated/CommandName";
import { ORDER_ACCESSION_CONTEXT } from "../useOrderAccessionContext";

/** `list_clients() → ClientView[]` and `list_practitioners() → PractitionerView[]`. */
export const LIST_CLIENTS = "list_clients";
export const LIST_PRACTITIONERS = "list_practitioners";

/** `list_active_orderables() → OrderableVersionView[]`: the current approved versions. */
export const LIST_ACTIVE_ORDERABLES = "list_active_orderables";

/** `register_order({input: RegisterOrderInput}) → OrderDetail`. */
export const REGISTER_ORDER = "register_order";

/** `get_order({input: GetOrderInput}) → OrderDetail` (audited read). */
export const GET_ORDER = "get_order";

/** `list_orders({input: ListOrdersInput}) → OrderSummary[]` (audited read), newest first. */
export const LIST_ORDERS = "list_orders";

/** The queries every order command makes stale. */
export const ORDER_QUERIES: readonly CommandName[] = [
  GET_ORDER,
  LIST_ORDERS,
  ORDER_ACCESSION_CONTEXT,
];

/** The `allowedActions` an order offers, by the engine's names. */
export const ORDER_ACTIONS = {
  accept: "accept",
  reject: "reject",
  placeHold: "place_hold",
  requestFollowOn: "request_follow_on",
} as const;
