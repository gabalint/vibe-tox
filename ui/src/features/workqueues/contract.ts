// Work queues (REQ-OPS-004; design brief §6 and §7 "Work queues"). Any signed-in session may
// list them; the engine filters the queues by the caller's capabilities and sorts each
// queue's items oldest first. Titles carry no PHI (accession, report or order numbers).

export type { WorkQueueItem } from "@/generated/WorkQueueItem";
export type { WorkQueueSummary } from "@/generated/WorkQueueSummary";

/** `list_work_queues() → WorkQueueSummary[]`. */
export const LIST_WORK_QUEUES = "list_work_queues";
/** `list_work_queue_items({ input: { queue, limit } }) → WorkQueuePage`. */

/** How many items of one queue are shown, oldest first. */
export const ITEM_LIMIT = 200;

/** The item kinds a screen exists for. */
export type WorkItemKind =
  | "deviation"
  | "order"
  | "subject"
  | "accession"
  | "collection_review"
  | "review_specimen"
  | "specimen_report"
  | "run_analyte_qc"
  | "run_position"
  | "import_attempt"
  | "report_revision"
  | "obligation"
  | "delivery_destination"
  | "preservation_incident"
  | "billing_derivation"
  | "billing_subject"
  | "billing_handoff_item"
  | "health_check"
  | "app_run"
  | "recovery_reconciliation"
  | "app_version";
