import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { SetInstrumentStatusInput } from "@/generated/SetInstrumentStatusInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { READINESS_QUERIES } from "../contract";
import { INSTRUMENT_STATUSES } from "../labels";

/** Records an instrument's service status with the reason (REQ-ANA-007). */
export function InstrumentStatusDialog({
  instrument,
  onClose,
}: {
  instrument: InstrumentReadinessView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ status: string | null; reason: string }, SetInstrumentStatusInput>
      title={`Set status of ${instrument.name}`}
      submitLabel="Set status"
      command={{ name: "set_instrument_status", invalidates: READINESS_QUERIES }}
      defaultValues={{ status: null, reason: "" }}
      toInput={(values) => ({
        instrumentId: instrument.instrumentId,
        status: checked(choiceOf(INSTRUMENT_STATUSES, values.status)),
        reason: values.reason.trim(),
      })}
      successTitle={`Status of ${instrument.name} recorded`}
      onClose={onClose}
    >
      <ChoiceInput name="status" label="Status" options={INSTRUMENT_STATUSES} isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
