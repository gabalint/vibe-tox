import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ConfigureStorageLocationInput } from "@/generated/ConfigureStorageLocationInput";
import type { StorageLocationView } from "@/generated/StorageLocationView";
import { CommandDialog } from "@/lib/CommandDialog";
import { omittedIfBlank } from "@/lib/choices";

interface Values {
  code: string;
  name: string;
  storageCondition: string;
  active: string;
}
export function LocationDialog({
  location,
  onClose,
}: {
  location?: StorageLocationView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<Values, ConfigureStorageLocationInput>
      title={location ? `Configure location ${location.code}` : "Add storage location"}
      submitLabel="Save location"
      command={{
        name: "configure_location",
        invalidates: ["list_locations", "get_accession", "find_container"],
      }}
      defaultValues={{
        code: location?.code ?? "",
        name: location?.name ?? "",
        storageCondition: location?.storageCondition ?? "",
        active: location?.active === false ? "no" : "yes",
      }}
      toInput={(v) => ({
        locationId: location?.locationId,
        expectedRevision: location?.revision,
        code: v.code.trim(),
        name: v.name.trim(),
        storageCondition: omittedIfBlank(v.storageCondition),
        active: v.active === "yes",
      })}
      successTitle="Storage location saved"
      onClose={onClose}
    >
      <TextInput
        name="code"
        label="Code"
        isRequired
        description="Unique; 1–64 lowercase letters, digits, dots, dashes or underscores."
      />
      <TextInput
        name="name"
        label="Name"
        isRequired
        description="Operator-facing location; up to 200 characters."
      />
      <TextInput
        name="storageCondition"
        label="Storage condition"
        description="Optional laboratory-defined condition, up to 1,000 characters; no monitoring threshold is inferred."
      />
      <ChoiceInput
        name="active"
        label="Available for moves"
        options={[
          { id: "yes", label: "Active" },
          { id: "no", label: "Inactive" },
        ]}
        isRequired
      />
      <p>
        Inactive locations remain in material history. Changing this record does not move containers
        or assess an excursion.
      </p>
    </CommandDialog>
  );
}
