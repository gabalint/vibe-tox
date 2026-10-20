import type { Tone } from "@/components/tones";
import type { Choice } from "@/lib/choices";
import type { DeliveryChannel, DestinationState, DestinationView } from "./contract";

export const CHANNELS: readonly Choice<DeliveryChannel>[] = [
  { id: "fax", label: "Fax" },
  { id: "email", label: "Email" },
  { id: "mail", label: "Mail" },
  { id: "courier", label: "Courier" },
  { id: "pickup", label: "Pickup at the laboratory" },
];

/**
 * What the address field holds for each channel, and whether it must be filled in. A
 * presence check only: the engine checks the format again.
 */
export const ADDRESS_FIELD: Readonly<
  Record<DeliveryChannel, { isRequired: boolean; description: string }>
> = {
  fax: { isRequired: true, description: "The fax number, with area code, such as 312-555-0100." },
  email: { isRequired: true, description: "One email address, such as results@clinic.example." },
  mail: { isRequired: true, description: "The full postal address, as printed on the envelope." },
  courier: { isRequired: false, description: "Optional: where the courier hands the report over." },
  pickup: {
    isRequired: false,
    description: "Leave empty: the report is collected at the laboratory.",
  },
};

/** The state filter; retired destinations show only with "Show retired". */
export const STATE_FILTERS: readonly Choice<DestinationState | "all">[] = [
  { id: "all", label: "Any state" },
  { id: "unverified", label: "Not verified" },
  { id: "verified", label: "Verified" },
  { id: "reported_wrong", label: "Reported wrong" },
];

const STATUS: Readonly<Record<DestinationState, { label: string; tone: Tone }>> = {
  unverified: { label: "Not verified", tone: "warning" },
  verified: { label: "Verified", tone: "neutral" },
  // A delivery attempt found the address wrong: replace or retire it (REQ-RPT-012).
  reported_wrong: { label: "Reported wrong", tone: "danger" },
  retired: { label: "Retired", tone: "neutral" },
};

/** The destination's state, as the engine computes it, in words with its tone. */
export function destinationStatus(destination: DestinationView): { label: string; tone: Tone } {
  return STATUS[destination.state];
}
