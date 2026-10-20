import type { WorkQueueItem, WorkQueueSummary } from "./contract";

/** Synthetic work-queue records for the tests. No real laboratory data. */

export const NOW = "2026-10-13T12:00:00Z";

export const REVIEW_QUEUE: WorkQueueSummary = {
  queue: "review_ready",
  label: "Results ready for review",
  ownerCapability: "review_results",
  count: 3,
  countCapped: false,
  oldestSince: "2026-10-13T08:40:00Z",
  nextAction: "Review",
};

export const LABEL_QUEUE: WorkQueueSummary = {
  queue: "label_failures",
  label: "Label print failures",
  ownerCapability: "accession",
  count: 0,
  countCapped: false,
  oldestSince: null,
  nextAction: "Reprint",
};

export function item(overrides: Partial<WorkQueueItem> = {}): WorkQueueItem {
  return {
    queue: "review_ready",
    itemKind: "review_specimen",
    itemId: "00000000-0000-7000-8000-0000000000a1",
    title: "A26000001-urine",
    state: "ready",
    stateLabel: "Ready",
    since: "2026-10-11T08:00:00Z",
    nextAction: "Review",
    ...overrides,
  };
}
