import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { RetireBackupTargetInput } from "@/generated/RetireBackupTargetInput";
import type { TargetView } from "@/generated/TargetView";
import { useCommandMutation } from "@/lib/commands";
import { BACKUP_QUERIES, RETIRE_BACKUP_TARGET } from "../contract";

/** Retires a target with a reason; the copies already on it stay and can still be restored. */
export function RetireTargetDialog({
  target,
  onClose,
}: {
  target: TargetView;
  onClose: () => void;
}) {
  const retire = useCommandMutation<TargetView, { input: RetireBackupTargetInput }>(
    RETIRE_BACKUP_TARGET,
    { invalidates: BACKUP_QUERIES },
  );
  return (
    <ReasonConfirmDialog
      title={`Retire ${target.label}?`}
      confirmLabel="Retire target"
      variant="danger"
      command={retire}
      input={(reason) => ({
        targetId: target.targetId,
        expectedRevision: target.revision,
        reason,
      })}
      successTitle={`${target.label} retired`}
      onClose={onClose}
    >
      <p>
        No new backups or audit anchors go to this target. The copies already there stay where they
        are and can still be used for a restore.
      </p>
    </ReasonConfirmDialog>
  );
}
