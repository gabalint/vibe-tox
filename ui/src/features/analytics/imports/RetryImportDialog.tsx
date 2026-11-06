import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { RetryImportInput } from "@/generated/RetryImportInput";
import { useCommandMutation } from "@/lib/commands";
import { IMPORT_QUERIES } from "../contract";

/**
 * Retries the whole received message of an attempt with a reason: from the page header, or
 * from a quarantined row once its cause is fixed. Mount it only while open.
 */
export function RetryImportDialog({
  receiptId,
  onClose,
}: {
  receiptId: string;
  onClose: () => void;
}) {
  const retry = useCommandMutation<number, { input: RetryImportInput }>("retry_import", {
    invalidates: IMPORT_QUERIES,
  });
  return (
    <ReasonConfirmDialog<RetryImportInput>
      title="Retry import"
      confirmLabel="Retry import"
      variant="primary"
      command={retry}
      input={(reason) => ({ receiptId, reason })}
      successTitle="Retry queued; it appears as a new import attempt"
      onClose={onClose}
    >
      <p>
        The received message is processed again as a new attempt. Rows still open in quarantine from
        earlier attempts are replaced by the outcome of the retry.
      </p>
    </ReasonConfirmDialog>
  );
}
