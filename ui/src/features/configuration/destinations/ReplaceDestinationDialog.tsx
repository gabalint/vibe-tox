import { TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { CommandDialog } from "@/lib/CommandDialog";
import { labelOf, omittedIfBlank } from "@/lib/choices";
import { AddressInput } from "./AddressInput";
import {
  DESTINATION_QUERIES,
  type DestinationView,
  REPLACE_DELIVERY_DESTINATION,
  type ReplaceDeliveryDestinationInput,
} from "./contract";
import { CHANNELS } from "./labels";

interface ReplaceValues {
  address: string;
  label: string;
  reason: string;
}

/**
 * Replaces a destination with a new one for the same client and channel (REQ-RPT-012): the old
 * one is retired, and the new one must be verified again before it is used.
 */
export function ReplaceDestinationDialog({
  destination,
  onClose,
}: {
  destination: DestinationView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<ReplaceValues, ReplaceDeliveryDestinationInput, DestinationView>
      title={`Replace ${destination.label}`}
      submitLabel="Replace destination"
      command={{ name: REPLACE_DELIVERY_DESTINATION, invalidates: DESTINATION_QUERIES }}
      defaultValues={{ address: "", label: "", reason: "" }}
      toInput={(values) => ({
        destinationId: destination.destinationId,
        expectedRevision: destination.revision,
        address: omittedIfBlank(values.address),
        label: values.label.trim(),
        reason: values.reason.trim(),
      })}
      successTitle={`${destination.label} replaced`}
      onClose={onClose}
    >
      <p>
        {destination.label} ({labelOf(CHANNELS, destination.channel)}) will be retired. The new
        destination starts unverified and must be verified again, by a second person unless your
        laboratory's separation-of-duties policy allows self-verification. Open deliveries to the
        old destination must be redirected to the new one.
      </p>
      <AddressInput channel={destination.channel} />
      <TextInput name="label" label="Label" isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
