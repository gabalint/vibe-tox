import { TextInput } from "@/components/FormFields";
import type { DeliveryChannel } from "./contract";
import { ADDRESS_FIELD } from "./labels";

/**
 * The destination's address, required for fax, email and mail. Bound to the form field
 * `address`; the engine checks the format for the channel.
 */
export function AddressInput({ channel }: { channel: DeliveryChannel | null }) {
  const field = channel ? ADDRESS_FIELD[channel] : null;
  return (
    <TextInput
      key={channel ?? "none"}
      name="address"
      label="Address"
      isRequired={field?.isRequired}
      description={field?.description ?? "Choose the channel first; its address format shows here."}
    />
  );
}
