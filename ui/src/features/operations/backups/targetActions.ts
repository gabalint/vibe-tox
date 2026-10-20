import type { OfferedAction } from "@/lib/allowedActions";
import { REQUEST_RESTORE_TEST, RETIRE_BACKUP_TARGET } from "../contract";

/** An action on one target, by the engine's command name; each opens its dialog. */
export type TargetDialog = typeof REQUEST_RESTORE_TEST | typeof RETIRE_BACKUP_TARGET;

/**
 * A target's actions in the order offered; the panel and the row's menu show those its
 * `allowedActions` name.
 */
export const TARGET_ACTIONS: readonly OfferedAction<TargetDialog>[] = [
  { id: REQUEST_RESTORE_TEST, label: "Request restore test" },
  { id: RETIRE_BACKUP_TARGET, label: "Retire target", variant: "danger" },
];
