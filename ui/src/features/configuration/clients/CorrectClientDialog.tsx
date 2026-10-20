import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ClientLocationView } from "@/generated/ClientLocationView";
import type { ClientView } from "@/generated/ClientView";
import type { UpdateClient } from "@/generated/UpdateClient";
import type { UpdateClientLocation } from "@/generated/UpdateClientLocation";
import { CommandDialog } from "@/lib/CommandDialog";
import { CodedNameFields } from "../CodedNameFields";
import { LIST_CLIENTS } from "../contract";

export const DIRECTORY_STATES = [
  { id: "active", label: "Active" },
  { id: "retired", label: "Retired" },
] as const;
interface Values {
  code: string;
  name: string;
  state: string | null;
  reason: string;
}
export function CorrectClientDialog({
  record,
  kind,
  onClose,
}: {
  record: ClientView | ClientLocationView;
  kind: "client" | "location";
  onClose: () => void;
}) {
  return (
    <CommandDialog<Values, UpdateClient | UpdateClientLocation>
      title={kind === "client" ? "Correct client" : "Correct location"}
      submitLabel="Save correction"
      command={{
        name: kind === "client" ? "update_client" : "update_client_location",
        invalidates: [LIST_CLIENTS, "directory_history"],
      }}
      defaultValues={{
        code: record.code,
        name: record.name,
        state: record.active ? "active" : "retired",
        reason: "",
      }}
      toInput={(values) => ({
        ...(kind === "client" ? { clientId: record.id } : { locationId: record.id }),
        expectedRevision: record.revision,
        code: values.code.trim(),
        name: values.name.trim(),
        active: values.state === "active",
        reason: values.reason.trim(),
      })}
      successTitle="Directory correction saved"
      onClose={onClose}
    >
      <p>
        Correct the same organization or location from supplied evidence. Retirement removes it from
        new order selections. Existing orders, earlier facts and issued reports remain retained.
      </p>
      <CodedNameFields />
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
        description="Explain the supplied correction or retirement, up to 1000 characters. Do not repurpose an existing identity for another organization."
      />
    </CommandDialog>
  );
}
