import { TextInput } from "@/components/FormFields";
import type { RecordExcursionInput } from "@/generated/RecordExcursionInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import { LocationChoice } from "./LocationChoice";
import { ObservedTimeInput, observedInstant } from "./ObservedTimeInput";
export function RecordExcursionDialog({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: (id: string) => void;
}) {
  return (
    <CommandDialog<
      { locationId: string | null; observedFrom: string; observedTo: string; description: string },
      RecordExcursionInput,
      { excursionId: string }
    >
      title="Record storage excursion"
      submitLabel="Record excursion"
      command={{ name: "record_excursion", invalidates: ["list_excursions", "get_excursion"] }}
      defaultValues={{ locationId: null, observedFrom: "", observedTo: "", description: "" }}
      toInput={(v) => ({
        locationId: checked(v.locationId),
        observedFrom: checked(observedInstant(v.observedFrom)),
        observedTo: v.observedTo.trim() ? checked(observedInstant(v.observedTo)) : undefined,
        description: v.description.trim(),
      })}
      successTitle="Storage excursion recorded"
      onDone={(result) => onDone(result.excursionId)}
      onClose={onClose}
    >
      <LocationChoice />
      <ObservedTimeInput name="observedFrom" label="Observed start" required />
      <ObservedTimeInput name="observedTo" label="Observed end" />
      <TextInput
        name="description"
        label="Observed condition"
        isRequired
        description="What happened, up to 1,000 characters. Recording creates an open incident for quality assessment."
      />
    </CommandDialog>
  );
}
