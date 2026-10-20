import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { FieldGrid, Form } from "@/components/Form";
import { DATE_FORMAT, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import type { SubjectSearch } from "@/generated/SubjectSearch";
import { optionalText } from "@/lib/choices";
import { trimIdentifier } from "./registration";

/** One extra result tells the directory whether another page exists. */
export const SEARCH_LIMIT = 26;

const NO_IDENTIFIER = { assigningAuthority: "", identifierType: "", value: "" };
const NO_NAME = { familyName: "", givenNames: "", birthDate: "" };

/**
 * Two ways to find a patient (REQ-IDN-001): an identifier within its assigning authority and
 * type, or the start of the name with an optional birth date. Enter in any field runs that
 * search.
 */
export function PatientSearch({ onSearch }: { onSearch: (input: SubjectSearch) => void }) {
  const byIdentifier = useForm({ defaultValues: NO_IDENTIFIER });
  const byName = useForm({ defaultValues: NO_NAME });
  const searchIdentifier = byIdentifier.handleSubmit((identifier) =>
    onSearch({ identifier: trimIdentifier(identifier), limit: SEARCH_LIMIT }),
  );
  const searchName = byName.handleSubmit(({ familyName, givenNames, birthDate }) =>
    onSearch({
      familyName: familyName.trim(),
      givenNames: optionalText(givenNames),
      birthDate: optionalText(birthDate),
      limit: SEARCH_LIMIT,
    }),
  );

  return (
    <FieldGrid>
      <Section title="Find by identifier">
        <FormProvider {...byIdentifier}>
          <Form
            aria-label="Find by identifier"
            onSubmit={searchIdentifier}
            actions={
              <Button type="submit" variant="primary">
                Find by identifier
              </Button>
            }
          >
            <TextInput name="value" label="Identifier" isRequired />
            <TextInput name="assigningAuthority" label="Assigning authority" isRequired />
            <TextInput
              name="identifierType"
              label="Identifier type"
              description="Such as MR"
              isRequired
            />
          </Form>
        </FormProvider>
      </Section>
      <Section title="Find by name">
        <FormProvider {...byName}>
          <Form
            aria-label="Find by name"
            onSubmit={searchName}
            actions={
              <Button type="submit" variant="primary">
                Find by name
              </Button>
            }
          >
            <TextInput
              name="familyName"
              label="Family name"
              description="The start of the family name."
              isRequired
            />
            <TextInput
              name="givenNames"
              label="Given names"
              description="The start of the given names."
            />
            <TextInput name="birthDate" label="Birth date" format={DATE_FORMAT} />
          </Form>
        </FormProvider>
      </Section>
    </FieldGrid>
  );
}
