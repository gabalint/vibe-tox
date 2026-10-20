import { TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { CloseDeviationInput } from "@/generated/CloseDeviationInput";
import type { DeviationView } from "@/generated/DeviationView";
import { CommandDialog } from "@/lib/CommandDialog";
import { QC_QUERIES } from "../contract";

/** Closes a deviation with its disposition, as the laboratory's procedure states it. */
export function CloseDeviationDialog({
  deviation,
  onClose,
}: {
  deviation: DeviationView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ disposition: string; reason: string }, CloseDeviationInput, DeviationView>
      title="Close deviation"
      submitLabel="Close deviation"
      command={{ name: "close_deviation", invalidates: QC_QUERIES }}
      defaultValues={{ disposition: "", reason: "" }}
      toInput={(values) => ({
        deviationId: deviation.deviationId,
        expectedRevision: deviation.revision,
        disposition: values.disposition.trim(),
        reason: values.reason.trim(),
      })}
      successTitle="Deviation closed"
      onClose={onClose}
    >
      <p>{deviation.description}</p>
      <TextInput name="disposition" label="Disposition" isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
