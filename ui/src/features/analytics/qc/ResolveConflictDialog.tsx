import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { QcConflictResolution } from "@/generated/QcConflictResolution";
import type { QcConflictView } from "@/generated/QcConflictView";
import type { ResolveQcConflictInput } from "@/generated/ResolveQcConflictInput";
import { useCommandMutation } from "@/lib/commands";
import { QC_QUERIES, RESULT_QUERIES } from "../contract";
import { receivedValueText } from "../labels";

interface ResolveConflictDialogProps {
  conflict: QcConflictView;
  resolution: QcConflictResolution;
  onClose: () => void;
}

/**
 * Accepting applies the changed row as a correction that replaces the stored observation;
 * rejecting keeps the stored one. Both need a reason, and the received text is kept either way.
 */
export function ResolveConflictDialog({
  conflict,
  resolution,
  onClose,
}: ResolveConflictDialogProps) {
  const resolve = useCommandMutation<QcConflictView, { input: ResolveQcConflictInput }>(
    "resolve_qc_conflict",
    { invalidates: [...QC_QUERIES, ...RESULT_QUERIES] },
  );
  const accepting = resolution === "accept_as_correction";
  return (
    <ReasonConfirmDialog
      title={accepting ? "Accept the changed QC row?" : "Reject the changed QC row?"}
      confirmLabel={accepting ? "Accept as correction" : "Reject changed row"}
      variant={accepting ? "primary" : "danger"}
      command={resolve}
      input={(reason) => ({
        conflictId: conflict.conflictId,
        expectedRevision: conflict.revision,
        resolution,
        reason,
      })}
      successTitle={accepting ? "Changed QC row accepted" : "Changed QC row rejected"}
      onClose={onClose}
    >
      <p>
        {accepting
          ? `${receivedValueText(conflict)} replaces the stored observation. QC assessed on the stored value must be assessed again.`
          : "The stored observation stays. The changed row is kept as received, marked rejected."}
      </p>
    </ReasonConfirmDialog>
  );
}
