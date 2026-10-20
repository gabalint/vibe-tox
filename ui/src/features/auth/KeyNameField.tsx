import { TextInput } from "@/components/FormFields/TextInput";

/** A personal label distinguishes the everyday key from a spare; never collect its PIN here. */
export function KeyNameField() {
  return (
    <TextInput
      name="keyLabel"
      label="Key name"
      isRequired
      description="A name you recognize, such as Everyday key or Spare key."
    />
  );
}
