import { useWatch } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import {
  DELIVERY_QUERIES,
  type DeliveryEvidence,
  type DeliveryFailureCode,
  type ObligationView,
  RECORD_DELIVERY_ATTEMPT,
  type RecordDeliveryAttemptInput,
} from "../contract";
import { EVIDENCE, FAILURE_CODES } from "../labels";

interface AttemptValues {
  evidence: DeliveryEvidence | null;
  failureCode: DeliveryFailureCode | null;
  note: string;
}

function toInput(obligationId: string, values: AttemptValues): RecordDeliveryAttemptInput {
  const evidence = checked(values.evidence);
  const failureCode = evidence === "failed" ? checked(values.failureCode) : undefined;
  return { obligationId, evidence, failureCode, note: omittedIfBlank(values.note) };
}

/**
 * Records what staff did and saw for one obligation (REQ-RPT-003, REQ-RPT-011): handed off
 * (attested by the person recording), failed with a cause, or outcome unknown. A retry is
 * another attempt on the same obligation; it never creates a report. The engine records the
 * time and checks the destination and the revision again.
 */
export function RecordAttemptDialog({
  obligation,
  onRecorded,
  onClose,
}: {
  obligation: ObligationView;
  /** The obligation as the engine returns it after the attempt, with its new status. */
  onRecorded?: (obligation: ObligationView) => void;
  onClose: () => void;
}) {
  return (
    <CommandDialog<AttemptValues, RecordDeliveryAttemptInput, ObligationView>
      title="Record delivery attempt"
      submitLabel="Record attempt"
      command={{ name: RECORD_DELIVERY_ATTEMPT, invalidates: DELIVERY_QUERIES }}
      defaultValues={{ evidence: null, failureCode: null, note: "" }}
      toInput={(values) => toInput(obligation.obligationId, values)}
      successTitle="Delivery attempt recorded"
      onDone={onRecorded}
      onClose={onClose}
    >
      <p>
        {obligation.reportNumber}, revision {obligation.revisionNo}, to{" "}
        {obligation.destination.label}. Current status: {obligation.status.label}.
      </p>
      <ChoiceInput name="evidence" label="What happened" options={EVIDENCE} isRequired />
      <FailureCodeField />
      <TextInput name="note" label="Note" description="Optional; recorded with the attempt." />
    </CommandDialog>
  );
}

function FailureCodeField() {
  const evidence = choiceOf(EVIDENCE, useWatch({ name: "evidence" }));
  if (evidence !== "failed") return null;
  return (
    <ChoiceInput name="failureCode" label="Why it failed" options={FAILURE_CODES} isRequired />
  );
}
