import { FieldGrid } from "@/components/Form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { YES_NO } from "@/lib/choices";

/**
 * Step 1: the software that writes the files, as declared and shown on every import (not
 * checked against the file), and whether it reports dilution-corrected values (F2, F11).
 */
export function SoftwareSection() {
  return (
    <Section title="1. Software">
      <FieldGrid>
        <TextInput
          name="softwareName"
          label="Software name"
          description="Such as MassHunter Quantitative Analysis."
          isRequired
        />
        <TextInput name="softwareVersion" label="Software version" isRequired />
      </FieldGrid>
      <ChoiceInput
        name="reportsFinalConcentration"
        label="Reports final (dilution-corrected) concentrations"
        options={YES_NO}
        isRequired
      />
    </Section>
  );
}
