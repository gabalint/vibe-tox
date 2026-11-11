import { TextInput } from "@/components/FormFields";
import type { CatalogCodedName } from "@/generated/CatalogCodedName";

/**
 * The laboratory's code and display name of a new record. `autoFocus` is off where the fields
 * move after the form opened, so moving them does not take the focus.
 */
export function CodedNameFields({
  autoFocus = true,
  codeDescription = "Letters, digits, '.', '_' or '-'; it cannot be changed later.",
}: {
  autoFocus?: boolean;
  codeDescription?: string;
}) {
  return (
    <>
      <TextInput
        name="code"
        label="Code"
        description={codeDescription}
        isRequired
        autoFocus={autoFocus}
      />
      <TextInput name="name" label="Name" isRequired />
    </>
  );
}

/** The code and name as typed, without surrounding spaces; the engine validates them. */
export function trimCodedName({ code, name }: CatalogCodedName): CatalogCodedName {
  return { code: code.trim(), name: name.trim() };
}
