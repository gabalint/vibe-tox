import { TextInput } from "@/components/FormFields";
import { CommandDialog } from "@/lib/CommandDialog";
import { labelOf } from "@/lib/choices";
import {
  DESTINATION_QUERIES,
  type DestinationView,
  VERIFY_DELIVERY_DESTINATION,
  type VerifyDeliveryDestinationInput,
} from "./contract";
import { CHANNELS } from "./labels";

/**
 * Records how a person confirmed that a destination reaches the client (REQ-RPT-012): a second
 * person, or its creator where the laboratory's separation-of-duties policy allows it and the
 * creator confirms (ADR 0015).
 */
export function VerifyDestinationDialog({
  destination,
  onClose,
}: {
  destination: DestinationView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ method: string }, VerifyDeliveryDestinationInput, DestinationView>
      title={`Verify ${destination.label}`}
      submitLabel="Verify destination"
      command={{ name: VERIFY_DELIVERY_DESTINATION, invalidates: DESTINATION_QUERIES }}
      defaultValues={{ method: "" }}
      toInput={(values, selfApproval) => ({
        destinationId: destination.destinationId,
        expectedRevision: destination.revision,
        method: values.method.trim(),
        selfApproval,
      })}
      successTitle={`${destination.label} verified`}
      onClose={onClose}
    >
      <p>
        {labelOf(CHANNELS, destination.channel)}: {destination.address ?? "no address"}. A person
        other than the one who added this destination must verify it before the first report is sent
        by it, unless your laboratory's separation-of-duties policy allows self-verification.
      </p>
      <TextInput
        name="method"
        label="How it was verified"
        description="Such as: test fax confirmed by phone with the clinic."
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}
