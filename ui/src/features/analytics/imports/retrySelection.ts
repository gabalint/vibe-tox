import { useCallback, useMemo, useState } from "react";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import { canRetry } from "./RetryImportsDialog";

/**
 * The attempts checked for Retry selected: only those with something to retry can be, and
 * "Select all quarantined" checks every such attempt the current filter has loaded.
 */
export function useRetrySelection(rows: readonly ImportAttemptSummary[]) {
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const retryable = useMemo(
    () => new Set(rows.filter(canRetry).map((row) => row.importAttemptId)),
    [rows],
  );
  const isSelectable = useCallback((id: string) => retryable.has(id), [retryable]);
  return {
    selectedIds,
    setSelectedIds,
    isSelectable,
    /** Whether any loaded attempt can be retried. */
    hasRetryable: retryable.size > 0,
    selectAll: () => setSelectedIds(new Set(retryable)),
    selected: rows.filter((row) => selectedIds.has(row.importAttemptId) && canRetry(row)),
    clear: () => setSelectedIds(new Set()),
  };
}

export type RetrySelection = ReturnType<typeof useRetrySelection>;
