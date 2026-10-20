import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, RecordPickerInput, TextInput } from "@/components/FormFields";
import type { ClientView } from "@/generated/ClientView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { AddressInput } from "./AddressInput";
import {
  CREATE_DELIVERY_DESTINATION,
  type CreateDeliveryDestinationInput,
  DESTINATION_QUERIES,
  type DeliveryChannel,
  type DestinationView,
} from "./contract";
import { CHANNELS } from "./labels";

interface DestinationValues {
  clientId: string | null;
  clientLocationId: string | null;
  channel: string | null;
  address: string;
  label: string;
}

interface AddDestinationDialogProps {
  clients: readonly ClientView[];
  /** The client the destination is for, when added from the client's page. */
  presetClientId?: string;
  onSaved?: (destination: DestinationView) => void;
  onClose: () => void;
}

/** Adds an unverified destination for a client or one of its locations (REQ-RPT-012). */
export function AddDestinationDialog({
  clients,
  presetClientId,
  onSaved,
  onClose,
}: AddDestinationDialogProps) {
  const preset = clients.find((client) => client.id === presetClientId);
  return (
    <CommandDialog<DestinationValues, CreateDeliveryDestinationInput, DestinationView>
      title={preset ? `Add destination for ${preset.name}` : "Add destination"}
      submitLabel="Add destination"
      command={{ name: CREATE_DELIVERY_DESTINATION, invalidates: DESTINATION_QUERIES }}
      defaultValues={{
        clientId: preset?.id ?? null,
        clientLocationId: null,
        channel: null,
        address: "",
        label: "",
      }}
      toInput={(values) => ({
        clientId: checked(values.clientId),
        clientLocationId: values.clientLocationId ?? undefined,
        channel: checked(choiceOf(CHANNELS, values.channel)),
        address: omittedIfBlank(values.address),
        label: values.label.trim(),
      })}
      successTitle={(values) => `Destination ${values.label.trim()} added`}
      onDone={onSaved}
      errorTitle="Not saved"
      onClose={onClose}
    >
      <p>
        The destination starts unverified. A person other than you must verify it before any report
        is sent by it, unless your laboratory's separation-of-duties policy allows you to verify it
        yourself.
      </p>
      <DestinationFields clients={clients} choosesClient={!preset} />
    </CommandDialog>
  );
}

const codedOption = (record: { id: string; code: string; name: string }) => ({
  id: record.id,
  label: record.name,
  description: record.code,
});

function DestinationFields({
  clients,
  choosesClient,
}: {
  clients: readonly ClientView[];
  choosesClient: boolean;
}) {
  const { control, setValue } = useFormContext<DestinationValues>();
  const clientId = useWatch({ control, name: "clientId" });
  const channel = useWatch({ control, name: "channel" });
  const locations = clients.find((client) => client.id === clientId)?.locations ?? [];
  const chosenChannel: DeliveryChannel | null = choiceOf(CHANNELS, channel);
  return (
    <>
      {choosesClient && (
        <RecordPickerInput
          name="clientId"
          label="Client"
          options={clients.map(codedOption)}
          isRequired
          onChange={() => setValue("clientLocationId", null)}
        />
      )}
      <RecordPickerInput
        name="clientLocationId"
        label="Location"
        options={locations.map(codedOption)}
        description="Optional: leave empty for a destination of the whole client."
      />
      <ChoiceInput name="channel" label="Channel" options={CHANNELS} isRequired />
      <AddressInput channel={chosenChannel} />
      <TextInput
        name="label"
        label="Label"
        description="How staff recognize it, such as Main office fax."
        isRequired
      />
    </>
  );
}
