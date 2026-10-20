import { useWatch } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { RecordHandoffFollowupInput } from "@/generated/RecordHandoffFollowupInput";
import type { RecordHandoffOutcome } from "@/generated/RecordHandoffOutcome";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { HANDOFF_QUERIES } from "../contract";
import { OUTCOMES } from "../labels";

interface OutcomeValues {
  outcome: string | null;
  externalReference: string;
  reason: string;
}

type Props = { itemId: string; onClose: () => void } & (
  | { followup?: false; revision?: never }
  | { followup: true; revision: number }
);

/**
 * Records the billing partner's answer for one handoff item (REQ-BIL-002): acknowledged,
 * with its reference when given, or rejected with the reason. Follow-up preserves the prior outcome.
 */
export function RecordOutcomeDialog({ itemId, followup, revision, onClose }: Props) {
  const title = followup ? "Record follow-up outcome" : "Record outcome";
  const toInput = (values: OutcomeValues): RecordHandoffOutcome | RecordHandoffFollowupInput => {
    const outcome = checked(choiceOf(OUTCOMES, values.outcome));
    if (followup)
      return {
        itemId,
        expectedRevision: revision,
        outcome,
        externalReference: omittedIfBlank(values.externalReference),
        reason: values.reason.trim(),
      };
    return outcome === "acknowledged"
      ? { itemId, outcome, externalReference: omittedIfBlank(values.externalReference) }
      : { itemId, outcome, reason: values.reason.trim() };
  };
  return (
    <CommandDialog<OutcomeValues, RecordHandoffOutcome | RecordHandoffFollowupInput>
      title={title}
      submitLabel={title}
      command={{
        name: followup ? "record_handoff_followup" : "record_handoff_outcome",
        invalidates: HANDOFF_QUERIES,
      }}
      defaultValues={{ outcome: null, externalReference: "", reason: "" }}
      toInput={toInput}
      successTitle="Outcome recorded"
      onClose={onClose}
    >
      <ChoiceInput name="outcome" label="Outcome" options={OUTCOMES} isRequired />
      <OutcomeDetail followup={followup ?? false} />
    </CommandDialog>
  );
}

function OutcomeDetail({ followup }: { followup: boolean }) {
  const outcome = useWatch<OutcomeValues, "outcome">({ name: "outcome" });
  if (followup)
    return (
      <>
        <TextInput name="externalReference" label="Partner reference" />
        <TextInput
          name="reason"
          label="Reason for follow-up"
          description="Describe the response received and the reason this outcome follows the rejection."
          isRequired
        />
      </>
    );
  if (outcome === "acknowledged") {
    return <TextInput name="externalReference" label="Partner reference" />;
  }
  if (outcome === "rejected") return <TextInput name="reason" label="Reason" isRequired />;
  return null;
}
