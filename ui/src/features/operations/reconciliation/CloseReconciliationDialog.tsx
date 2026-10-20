import { CheckList, TextInput } from "@/components/FormFields";
import type { CloseRecoveryInput } from "@/generated/CloseRecoveryInput";
import type { RecoveryReconciliation } from "@/generated/RecoveryReconciliation";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { CLOSE_RECOVERY_RECONCILIATION, RECONCILIATION_QUERIES } from "../contract";
import { isChecklistItem } from "../engineCodes";

interface CloseValues {
  attestedItems: string[];
  note: string;
}

interface CloseReconciliationDialogProps {
  reconciliation: RecoveryReconciliation;
  onClose: () => void;
}

/**
 * Closes the recovery reconciliation (design brief §3.8). The person attests each item they
 * checked; the engine refuses the closing unless every item is attested, and a wrong password
 * shows on its field.
 */
export function CloseReconciliationDialog({
  reconciliation,
  onClose,
}: CloseReconciliationDialogProps) {
  const options = reconciliation.items.map((item) => ({
    id: item.code,
    label: item.label,
    details: [item.guidance],
  }));
  return (
    <ReauthConfirmDialog<CloseValues, CloseRecoveryInput, null>
      title="Close recovery reconciliation"
      confirmLabel="Close reconciliation"
      variant="primary"
      command={{ name: CLOSE_RECOVERY_RECONCILIATION, invalidates: RECONCILIATION_QUERIES }}
      extraDefaults={{ attestedItems: [], note: "" }}
      toInput={({ attestedItems, note, password }) => ({
        restoreId: reconciliation.restoreId,
        // The engine lists only its own checklist codes, as plain text.
        attestedItems: attestedItems.filter(isChecklistItem),
        note: note.trim(),
        password,
      })}
      successTitle={() => "Recovery reconciliation closed"}
      onClose={onClose}
      errorTitle="Not closed"
    >
      <p>
        Once closed, held reports can be released, delivered and handed to billing, and the{" "}
        {reconciliation.heldLabelJobs} held label jobs print. Tick each item the laboratory has
        checked.
      </p>
      <CheckList name="attestedItems" legend="Items checked" options={options} />
      <TextInput name="note" label="Note" description="Recorded in the audit trail." isRequired />
    </ReauthConfirmDialog>
  );
}
