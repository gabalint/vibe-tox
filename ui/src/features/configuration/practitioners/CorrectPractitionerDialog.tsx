import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { PractitionerView } from "@/generated/PractitionerView";
import type { UpdatePractitioner } from "@/generated/UpdatePractitioner";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { DIRECTORY_STATES } from "../clients/CorrectClientDialog";
import { LIST_PRACTITIONERS } from "../contract";

interface Values {
  npi: string;
  familyName: string;
  givenNames: string;
  state: string | null;
  reason: string;
}
export function CorrectPractitionerDialog({
  practitioner,
  onClose,
}: {
  practitioner: PractitionerView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<Values, UpdatePractitioner>
      title="Correct practitioner"
      submitLabel="Save correction"
      command={{
        name: "update_practitioner",
        invalidates: [LIST_PRACTITIONERS, "directory_history"],
      }}
      defaultValues={{
        npi: practitioner.npi,
        familyName: practitioner.familyName,
        givenNames: practitioner.givenNames ?? "",
        state: practitioner.active ? "active" : "retired",
        reason: "",
      }}
      toInput={(values) => ({
        practitionerId: practitioner.id,
        expectedRevision: practitioner.revision,
        npi: values.npi.trim(),
        familyName: values.familyName.trim(),
        givenNames: optionalText(values.givenNames),
        active: values.state === "active",
        reason: values.reason.trim(),
      })}
      successTitle="Practitioner correction saved"
      onClose={onClose}
    >
      <p>
        Correct the same practitioner from supplied evidence. Do not transfer this identity to
        another person. Retiring removes the practitioner from new order selections; accepted orders
        and issued reports retain their records.
      </p>
      <TextInput
        name="npi"
        label="NPI"
        isRequired
        description="10 digits with a valid check digit; must not belong to another registered practitioner."
      />
      <TextInput
        name="familyName"
        label="Family name"
        isRequired
        description="Supplied name, up to 200 characters."
      />
      <TextInput
        name="givenNames"
        label="Given names"
        description="Optional supplied names, up to 200 characters."
      />
      <ChoiceInput
        name="state"
        label="Availability for new orders"
        isRequired
        options={DIRECTORY_STATES}
      />
      <TextInput
        name="reason"
        label="Reason"
        isRequired
        description="Evidence-backed correction or retirement explanation, up to 1000 characters."
      />
    </CommandDialog>
  );
}
