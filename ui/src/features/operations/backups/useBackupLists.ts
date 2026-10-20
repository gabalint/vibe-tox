import type { BackupRunView } from "@/generated/BackupRunView";
import type { RestoreTestView } from "@/generated/RestoreTestView";
import { useCommandPages } from "@/lib/commands";
import { LIST_BACKUP_RUNS, LIST_LIMIT, LIST_RESTORE_TESTS } from "../contract";
import { isUnfinished, JOB_POLL_MS } from "../useJobQuery";

const poll = (pages: { state: string }[][] | undefined) =>
  pages?.some((page) => page.some((row) => isUnfinished(row.state))) ? JOB_POLL_MS : false;

/** Timestamp-and-ID pages share the status band's cache; only unfinished jobs poll. */
export function useBackupRuns() {
  const query = useCommandPages<BackupRunView, string>(LIST_BACKUP_RUNS, {
    key: {},
    pageArgs: (beforeRunId) => ({
      input: { limit: LIST_LIMIT, beforeRequestedAt: null, beforeRunId },
    }),
    // The engine returns one lookahead row. It belongs to the next page.
    nextCursor: (page) => (page.length > LIST_LIMIT ? page[LIST_LIMIT - 1]?.runId : undefined),
    refetchInterval: poll,
  });
  const rows = query.data?.pages.flatMap((page) => page.slice(0, LIST_LIMIT)) ?? [];
  return { query, rows, hasMore: Boolean(query.hasNextPage) };
}

/** Restore-test history continues through every equal-time record. */
export function useRestoreTests() {
  const query = useCommandPages<RestoreTestView, string>(LIST_RESTORE_TESTS, {
    key: {},
    pageArgs: (beforeTestId) => ({ input: { limit: LIST_LIMIT, beforeTestId } }),
    nextCursor: (page) => (page.length === LIST_LIMIT ? page.at(-1)?.testId : undefined),
    refetchInterval: poll,
  });
  const rows = query.data?.pages.flat() ?? [];
  return { query, rows, hasMore: Boolean(query.hasNextPage) };
}
