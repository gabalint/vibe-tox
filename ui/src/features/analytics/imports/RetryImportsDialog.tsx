import { useState } from "react";
import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { CommandError } from "@/generated/CommandError";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { RetryImportInput } from "@/generated/RetryImportInput";
import { useCommandMutation } from "@/lib/commands";
import { toCommandError } from "@/lib/errors";
import { IMPORT_QUERIES } from "../contract";

/** Whether an attempt left anything to retry: a fully processed one did not. */
export const canRetry = (attempt: ImportAttemptSummary) => attempt.outcome !== "processed";

const retries = (count: number) => (count === 1 ? "1 retry" : `${count} retries`);
const imports = (count: number) => (count === 1 ? "1 import" : `${count} imports`);

interface RetryImportsDialogProps {
  /** The selected attempts, each one with something to retry. */
  attempts: readonly ImportAttemptSummary[];
  /** Called with how many retries were queued, once any was. */
  onDone: (queued: number) => void;
  onClose: () => void;
}

/**
 * Retries the received messages of several attempts at once (REQ-ANA-003): one `retry_import`
 * per received message, each with the same reason, so each is queued and audited on its own.
 * Two attempts of one message retry it once. They run one after another; at the first refusal
 * the rest wait, those queued stay queued, and submitting again goes on with the rest.
 */
export function RetryImportsDialog({ attempts, onDone, onClose }: RetryImportsDialogProps) {
  const toast = useToast();
  const form = useForm({ defaultValues: { reason: "" } });
  const retry = useCommandMutation<number, { input: RetryImportInput }>("retry_import", {
    invalidates: IMPORT_QUERIES,
  });
  const receipts = [...new Set(attempts.map((attempt) => attempt.receiptId))];
  const [queued, setQueued] = useState<ReadonlySet<string>>(new Set());
  const [failure, setFailure] = useState<CommandError | null>(null);
  const [isPending, setPending] = useState(false);
  const pending = receipts.filter((receiptId) => !queued.has(receiptId));

  const submit = form.handleSubmit(async ({ reason }) => {
    setPending(true);
    setFailure(null);
    const done = new Set(queued);
    for (const receiptId of pending) {
      try {
        await retry.mutateAsync({ input: { receiptId, reason: reason.trim() } });
        done.add(receiptId);
      } catch (error) {
        setQueued(done);
        setFailure(toCommandError(error));
        setPending(false);
        return;
      }
    }
    toast.show({
      tone: "success",
      title: `${retries(done.size)} queued`,
      description: "Each appears as a new import attempt once it is processed.",
    });
    onDone(done.size);
    onClose();
  });

  return (
    <FormDialog
      form={form}
      title={`Retry ${imports(receipts.length)}`}
      submitLabel={queued.size === 0 ? "Retry selected" : "Retry the rest"}
      errorTitle={`Not queued: ${retries(pending.length)}`}
      command={{ error: failure, isPending }}
      onSubmit={submit}
      onClose={() => {
        if (queued.size > 0) onDone(queued.size);
        onClose();
      }}
    >
      <p>
        Each received message is processed again as a new attempt, with the reason below recorded
        for each one. Rows still open in quarantine from earlier attempts are replaced by the
        outcome of the retry.
        {receipts.length < attempts.length &&
          ` ${attempts.length} attempts were selected from ${receipts.length} received messages; each message is retried once.`}
      </p>
      {queued.size > 0 && <p>Already queued: {retries(queued.size)}.</p>}
      <ReasonField />
    </FormDialog>
  );
}
