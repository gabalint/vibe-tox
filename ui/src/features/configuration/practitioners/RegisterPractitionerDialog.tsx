import { NPI_FORMAT, TextInput } from "@/components/FormFields";
import type { RegisterPractitioner } from "@/generated/RegisterPractitioner";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { LIST_PRACTITIONERS } from "../contract";

interface PractitionerValues {
  npi: string;
  familyName: string;
  givenNames: string;
}

/** Registers an ordering practitioner; the engine checks the NPI's check digit. */
export function RegisterPractitionerDialog({
  onSaved,
  onClose,
}: {
  onSaved: (practitionerId: string) => void;
  onClose: () => void;
}) {
  return (
    <CommandDialog<PractitionerValues, RegisterPractitioner, string>
      title="Register practitioner"
      submitLabel="Register practitioner"
      command={{ name: "register_practitioner", invalidates: [LIST_PRACTITIONERS] }}
      defaultValues={{ npi: "", familyName: "", givenNames: "" }}
      toInput={(values) => ({
        npi: values.npi.trim(),
        familyName: values.familyName.trim(),
        givenNames: optionalText(values.givenNames),
      })}
      successTitle={(values) => `${values.familyName.trim()} registered`}
      onDone={onSaved}
      errorTitle="Not saved"
      onClose={onClose}
    >
      <TextInput name="npi" label="NPI" format={NPI_FORMAT} isRequired autoFocus />
      <TextInput name="familyName" label="Family name" isRequired />
      <TextInput name="givenNames" label="Given names" />
    </CommandDialog>
  );
}
