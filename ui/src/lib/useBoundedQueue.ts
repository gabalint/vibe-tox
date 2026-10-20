import {
  type InfiniteData,
  type QueryKey,
  useInfiniteQuery,
  useQueryClient,
} from "@tanstack/react-query";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { useEffect, useId, useState } from "react";
import type { CommandError } from "@/generated/CommandError";
import type { CommandName } from "@/generated/CommandName";
import { call } from "./ipc";

interface QueuePage<Item, Cursor> {
  items: Item[];
  next: Cursor | null;
  scanned: number;
}

interface QueueOptions<Item, Cursor> {
  key: Record<string, unknown>;
  pageArgs: (after: Cursor | null) => InvokeArgs;
  rowId: (item: Item) => string;
}

const VISIBLE_BATCH = 50;

/** Progressive engine reads. A checked candidate cursor can advance without any matches. */
export function useBoundedQueue<Item, Cursor>(
  command: CommandName,
  { key, pageArgs, rowId }: QueueOptions<Item, Cursor>,
) {
  const client = useQueryClient();
  const scope = JSON.stringify([command, key]);
  const mount = useId();
  const [current, setControl] = useScanControl(scope);
  const queryKey = [command, key, { mount, entry: current.entry }];
  const query = useInfiniteQuery<
    QueuePage<Item, Cursor>,
    CommandError,
    InfiniteData<QueuePage<Item, Cursor>>,
    QueryKey,
    { after: Cursor | null }
  >({
    queryKey,
    // Operator-controlled reads: cache invalidation must not replay a long historical scan.
    enabled: false,
    initialPageParam: { after: null },
    queryFn: ({ pageParam }) => call<QueuePage<Item, Cursor>>(command, pageArgs(pageParam.after)),
    getNextPageParam: (page) => (page.next === null ? undefined : { after: page.next }),
  });
  const rows = [
    ...new Map(
      query.data?.pages.flatMap((page) => page.items).map((row) => [rowId(row), row]),
    ).values(),
  ];
  const scanned = query.data?.pages.reduce((sum, page) => sum + page.scanned, 0) ?? 0;
  const filling = rows.length < current.target;
  const { hasNextPage, isFetching, isError, fetchNextPage } = query;
  useEffect(() => {
    if ((!query.data || (hasNextPage && filling)) && !current.paused && !isFetching && !isError) {
      void fetchNextPage({ cancelRefetch: false });
    }
  }, [query.data, hasNextPage, filling, current.paused, isFetching, isError, fetchNextPage]);
  return {
    ...query,
    refetch: () => query.refetch({ cancelRefetch: false }),
    fetchNextPage: () => query.fetchNextPage({ cancelRefetch: false }),
    rows,
    scanned,
    paused: current.paused,
    scanning: Boolean(hasNextPage && filling && !current.paused && !isError),
    pause: () => setControl({ ...current, paused: true }),
    resume: () => setControl({ ...current, paused: false }),
    loadMore: () =>
      setControl({ ...current, scope, target: rows.length + VISIBLE_BATCH, paused: false }),
    // A new entry starts at null; no awaited observer operation can revive an older scope.
    restartScan: async () => {
      setControl((live) =>
        live.scope === scope && live.entry === current.entry
          ? { ...live, entry: live.entry + 1, target: VISIBLE_BATCH, paused: false }
          : live,
      );
      await client.cancelQueries({ queryKey, exact: true });
    },
  };
}

/** A filter return is a new scan, even when a completed older query remains cached. */
function useScanControl(scope: string) {
  const [control, setControl] = useState({ scope, entry: 0, target: VISIBLE_BATCH, paused: false });
  if (control.scope !== scope) {
    const fresh = { scope, entry: control.entry + 1, target: VISIBLE_BATCH, paused: false };
    setControl(fresh);
    return [fresh, setControl] as const;
  }
  return [control, setControl] as const;
}
