import { useState } from "react";
import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { AssessQcInput } from "@/generated/AssessQcInput";
import type { CommandError } from "@/generated/CommandError";
import { omittedIfBlank } from "@/lib/choices";
import { useCommandMutation } from "@/lib/commands";
import { toCommandError } from "@/lib/errors";
import { QC_QUERIES } from "../contract";
import { currentObservations, type QcScope } from "./qcScopes";

interface AcceptQcDialogProps {
  /** The selected analytes, each one able to be accepted. */
  scopes: readonly QcScope[];
  onDone: () => void;
  onClose: () => void;
}

const analytes = (count: number) => (count === 1 ? "1 analyte" : `${count} analytes`);

/**
 * Accepts QC for several analytes at once (REQ-ANA-004): one `assess_qc` per analyte, each
 * citing its current observations shown and with the same reason, so each is recorded and
 * audited on its own. They run one after another; at the first refusal the rest wait, those
 * recorded stay recorded, and submitting again goes on with the rest. Rejection stays one
 * analyte at a time.
 */
export function AcceptQcDialog({ scopes, onDone, onClose }: AcceptQcDialogProps) {
  const toast = useToast();
  const form = useForm<{ reason: string }>({ defaultValues: { reason: "" } });
  const assess = useCommandMutation<unknown, { input: AssessQcInput }>("assess_qc", {
    invalidates: QC_QUERIES,
  });
  const [recorded, setRecorded] = useState<ReadonlySet<string>>(new Set());
  const [failure, setFailure] = useState<{ label: string; error: CommandError } | null>(null);
  const [isPending, setPending] = useState(false);
  const pending = scopes.filter((scope) => !recorded.has(scope.key));
  const replaces = scopes.some((scope) => scope.assessments.current !== null);

  const submit = form.handleSubmit(async ({ reason }) => {
    setPending(true);
    setFailure(null);
    const done = new Set(recorded);
    for (const scope of pending) {
      try {
        await assess.mutateAsync({
          input: {
            ...scope.target,
            outcome: "accepted",
            evidenceObservationIds: currentObservations(scope).map((each) => each.observationId),
            supersedesAssessmentId: scope.assessments.current?.assessmentId,
            reason: omittedIfBlank(reason),
          },
        });
        done.add(scope.key);
      } catch (error) {
        setRecorded(done);
        setFailure({ label: scope.label, error: toCommandError(error) });
        setPending(false);
        return;
      }
    }
    toast.show({ tone: "success", title: `QC accepted for ${analytes(scopes.length)}` });
    onDone();
    onClose();
  });

  return (
    <FormDialog
      form={form}
      title={`Accept QC for ${analytes(scopes.length)}`}
      submitLabel={recorded.size === 0 ? "Accept QC" : "Accept the rest"}
      errorTitle={failure ? `Not recorded for ${failure.label}` : "Not recorded"}
      command={{ error: failure?.error ?? null, isPending }}
      onSubmit={submit}
      onClose={() => {
        if (recorded.size > 0) onDone();
        onClose();
      }}
    >
      <p>
        Records a separate acceptance for each analyte, citing its current observations shown:{" "}
        {pending.map((scope) => `${scope.label} (${currentObservations(scope).length})`).join(", ")}
        .
      </p>
      {recorded.size > 0 && (
        <p>
          Already recorded:{" "}
          {scopes
            .filter((scope) => recorded.has(scope.key))
            .map((scope) => scope.label)
            .join(", ")}
          .
        </p>
      )}
      <TextInput
        name="reason"
        label="Reason"
        description={
          replaces
            ? "Recorded with each assessment in the audit trail."
            : "Optional; recorded with each assessment in the audit trail."
        }
        isRequired={replaces}
      />
    </FormDialog>
  );
}
