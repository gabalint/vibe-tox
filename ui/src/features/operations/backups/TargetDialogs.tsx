import type { TargetView } from "@/generated/TargetView";
import { REQUEST_RESTORE_TEST, RETIRE_BACKUP_TARGET } from "../contract";
import { AddTargetDialog } from "./AddTargetDialog";
import { RestoreTestDialog } from "./RestoreTestDialog";
import { RetireTargetDialog } from "./RetireTargetDialog";
import type { TargetDialog } from "./targetActions";

/** The dialog open on the targets screen: adding a target, or an action on one target. */
export type TargetDialogState = { kind: "add" } | { kind: TargetDialog; target: TargetView } | null;

/** Mounts the open dialog only while it is open. */
export function TargetDialogs({
  dialog,
  onClose,
}: {
  dialog: TargetDialogState;
  onClose: () => void;
}) {
  if (dialog?.kind === "add") return <AddTargetDialog onClose={onClose} />;
  if (dialog?.kind === REQUEST_RESTORE_TEST)
    return <RestoreTestDialog target={dialog.target} onClose={onClose} />;
  if (dialog?.kind === RETIRE_BACKUP_TARGET) {
    return <RetireTargetDialog target={dialog.target} onClose={onClose} />;
  }
  return null;
}
