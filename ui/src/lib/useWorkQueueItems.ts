import type { WorkQueueCursor } from "@/generated/WorkQueueCursor";
import type { WorkQueueItem } from "@/generated/WorkQueueItem";
import { useBoundedQueue } from "./useBoundedQueue";

/** Follow the engine's checked-candidate cursor, including pages without matches. */
export function useWorkQueueItems(queue: string, limit: number) {
  return useBoundedQueue<WorkQueueItem, WorkQueueCursor>("list_work_queue_items", {
    key: { queue, limit },
    pageArgs: (after) => ({ input: { queue, limit, after } }),
    rowId: (row) => `${row.itemKind}:${row.itemId}`,
  });
}
