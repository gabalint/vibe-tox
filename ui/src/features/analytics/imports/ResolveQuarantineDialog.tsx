import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import type { QuarantineResolution } from "@/generated/QuarantineResolution";
import type { ResolveQuarantineItemInput } from "@/generated/ResolveQuarantineItemInput";
import { useCommandMutation } from "@/lib/commands";
import { IMPORT_QUERIES } from "../contract";
import { quarantineReasonText } from "../resultLabels";

const RESOLUTIONS: Record<
  QuarantineResolution,
  { title: string; confirm: string; variant: "primary" | "danger"; effect: string }
> = {
  accept_as_reprocessing: {
    title: "Accept as reprocessing",
    confirm: "Accept row",
    variant: "primary",
    effect:
      "The conflicting value is applied as reprocessing, which creates a new revision of the result line. Only rows quarantined as conflicting duplicates can be accepted.",
  },
  reject: {
    title: "Reject row",
    confirm: "Reject row",
    variant: "danger",
    effect: "The row is never imported. It stays on record with your reason.",
  },
};

interface ResolveQuarantineDialogProps {
  item: QuarantineItemView;
  resolution: QuarantineResolution;
  onClose: () => void;
}

/** Decides one quarantined row with a reason (REQ-ANA-003). */
export function ResolveQuarantineDialog({
  item,
  resolution,
  onClose,
}: ResolveQuarantineDialogProps) {
  const resolve = useCommandMutation<ImportAttemptDetail, { input: ResolveQuarantineItemInput }>(
    "resolve_quarantine_item",
    { invalidates: IMPORT_QUERIES },
  );
  const text = RESOLUTIONS[resolution];
  return (
    <ReasonConfirmDialog<ResolveQuarantineItemInput>
      title={text.title}
      confirmLabel={text.confirm}
      variant={text.variant}
      command={resolve}
      input={(reason) => ({
        itemId: item.itemId,
        expectedRevision: item.revision,
        resolution,
        reason,
      })}
      successTitle={`${text.title}: recorded`}
      onClose={onClose}
    >
      <p>
        Row {item.recordPath}: {quarantineReasonText(item.reason)}.
      </p>
      <p>{text.effect}</p>
    </ReasonConfirmDialog>
  );
}
