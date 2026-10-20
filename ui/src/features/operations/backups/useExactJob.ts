import type { InvokeArgs } from "@tauri-apps/api/core";
import { useState } from "react";
import type { CommandName } from "@/generated/CommandName";
import { useCommandQuery } from "@/lib/commands";
import { isUnfinished, JOB_POLL_MS } from "../useJobQuery";

/** Exact links follow a background job until terminal, even outside every loaded page. */
export function useExactJob<Row extends { state: string }>(
  command: CommandName,
  args: InvokeArgs,
  enabled: boolean,
) {
  const [polling, setPolling] = useState(false);
  const query = useCommandQuery<Row[]>(command, args, {
    enabled,
    refetchInterval: polling ? JOB_POLL_MS : undefined,
  });
  const unfinished =
    enabled && !query.isError && Boolean(query.data?.some((row) => isUnfinished(row.state)));
  if (unfinished !== polling) setPolling(unfinished);
  return query;
}
