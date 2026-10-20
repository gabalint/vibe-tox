import { FieldGrid } from "@/components/Form";
import { ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import { RELATIONSHIPS } from "./contactValues";

export function ContactFields({ prefix }: { prefix: string }) {
  return (
    <FieldGrid>
      <TextInput name={`${prefix}.line1`} label="Address line 1" />
      <TextInput name={`${prefix}.line2`} label="Address line 2" />
      <TextInput name={`${prefix}.line3`} label="Address line 3" />
      <TextInput name={`${prefix}.city`} label="City" />
      <TextInput name={`${prefix}.region`} label="State or region" />
      <TextInput name={`${prefix}.postalCode`} label="ZIP or postal code" />
      <TextInput name={`${prefix}.country`} label="Country" />
      <TextInput name={`${prefix}.phone`} label="Phone" />
      <TextInput name={`${prefix}.email`} label="Email" />
    </FieldGrid>
  );
}

export function PartyFields({ prefix, label }: { prefix: string; label: string }) {
  return (
    <>
      <FieldGrid>
        <TextInput name={`${prefix}.name`} label={`${label} name`} isRequired />
        <ChoiceInput
          name={`${prefix}.relationship`}
          label={`Patient's relationship to ${label.toLowerCase()}`}
          options={RELATIONSHIPS}
          isRequired
        />
        <TextInput
          name={`${prefix}.birthDate`}
          label={`${label} birth date`}
          format={DATE_FORMAT}
        />
      </FieldGrid>
      <details>
        <summary>{label} address and contact</summary>
        <ContactFields prefix={`${prefix}.contact`} />
      </details>
    </>
  );
}

export function ChangeEvidenceFields() {
  return (
    <FieldGrid>
      <TextInput
        name="source"
        label="Information source"
        description="Document or external record used to confirm these details."
        isRequired
      />
      <TextInput name="reason" label="Reason for change" isRequired />
    </FieldGrid>
  );
}
