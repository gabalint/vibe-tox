import type { InvokeArgs } from "@tauri-apps/api/core";
import { useState } from "react";
import type { CommandName } from "@/generated/CommandName";
import { useCommandQuery } from "@/lib/commands";

/** How often a screen asks again while a job it shows is waiting or running. */
export const JOB_POLL_MS = 5_000;

/** Backup runs, restore tests and exit exports run in the background in these states. */
export function isUnfinished(state: string): boolean {
  return state === "requested" || state === "running";
}

/**
 * An engine query asked again every few seconds while `hasUnfinished` says it shows a job the
 * engine runs in the background, so its progress and result appear without a reload. It stops
 * once every job shown has finished.
 */
export function useJobQuery<T>(
  command: CommandName,
  args: InvokeArgs | undefined,
  hasUnfinished: (data: T) => boolean,
) {
  const [polling, setPolling] = useState(false);
  const query = useCommandQuery<T>(command, args, polling ? { refetchInterval: JOB_POLL_MS } : {});
  const unfinished = query.data !== undefined && hasUnfinished(query.data);
  if (unfinished !== polling) setPolling(unfinished);
  return query;
}
