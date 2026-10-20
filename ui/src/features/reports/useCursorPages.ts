import { type InfiniteData, type QueryKey, useInfiniteQuery } from "@tanstack/react-query";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { CommandError } from "@/generated/CommandError";
import type { CommandName } from "@/generated/CommandName";
import { call } from "@/lib/ipc";

interface CursorPagesOptions<Page> {
  /** What the pages are filtered by; part of the query key. */
  key: InvokeArgs;
  /** The arguments for the page after `cursor`; `undefined` for the first page. */
  pageArgs: (cursor: string | undefined) => InvokeArgs;
  /** The engine's cursor for the page after `page`, or `undefined` when `page` is the last. */
  nextCursor: (page: Page) => string | undefined;
}

/**
 * An engine list read page by page, the next page only when asked ("Load more"). The query key
 * starts with the command name, so commands refresh it like any other read.
 *
 * `useCommandPages` in `lib/commands.ts` takes numeric cursors and array pages; the report
 * lists page by a release time or an opaque obligation cursor, so they page here until that
 * hook takes other cursors.
 */
export function useCursorPages<Page>(
  command: CommandName,
  { key, pageArgs, nextCursor }: CursorPagesOptions<Page>,
) {
  return useInfiniteQuery<Page, CommandError, InfiniteData<Page>, QueryKey, string | undefined>({
    queryKey: [command, key],
    queryFn: ({ pageParam }) => call<Page>(command, pageArgs(pageParam)),
    initialPageParam: undefined,
    getNextPageParam: nextCursor,
  });
}
