// BILL-G01–G08: audited financial record sections on explicitly opened patient/order records.
// EnterOrders or Billing is checked by the engine; actions come from each read's allowedActions.
// Profile child lists describe this patient; financial details are fetched on explicit access.
// All changes retain prior versions, invalidate dependent records and expose typed engine errors.
import type { CommandName } from "@/generated/CommandName";

export const GET_PATIENT_BILLING = "get_patient_billing_profile";
export const GET_ORDER_BILLING = "get_order_billing";
export const FINANCIAL_QUERIES: readonly CommandName[] = [
  GET_PATIENT_BILLING,
  GET_ORDER_BILLING,
  "get_order",
  "get_accession",
  "list_billing_review_items",
  "list_work_queues",
  "list_work_queue_items",
  "list_billing_work",
  "get_service_derivation",
  "get_billing_handoff",
];
