// Delivery destinations (REQ-RPT-012; design brief §2.1, §4.5 and §7 "Configuration").
// Listing needs only a session; every change needs `manage_configuration`. A destination is
// born unverified; a person other than its creator verifies it before the first report goes
// out by it, or its creator where the laboratory's separation-of-duties policy allows it. A replacement retires the old destination and creates a new, unverified one.
import type { CommandName } from "@/generated/CommandName";

export const LIST_DELIVERY_DESTINATIONS = "list_delivery_destinations";
/** A destination change also changes the work queues' counts (unverified, reported wrong). */
export const DESTINATION_QUERIES: readonly CommandName[] = [
  LIST_DELIVERY_DESTINATIONS,
  "list_work_queues",
];
export const CREATE_DELIVERY_DESTINATION = "create_delivery_destination";
export const VERIFY_DELIVERY_DESTINATION = "verify_delivery_destination";
export const REPLACE_DELIVERY_DESTINATION = "replace_delivery_destination";
export const RETIRE_DELIVERY_DESTINATION = "retire_delivery_destination";

export type { CreateDeliveryDestinationInput } from "@/generated/CreateDeliveryDestinationInput";
export type { DeliveryChannel } from "@/generated/DeliveryChannel";
export type { DestinationState } from "@/generated/DestinationState";
export type { DestinationView } from "@/generated/DestinationView";
export type { ReplaceDeliveryDestinationInput } from "@/generated/ReplaceDeliveryDestinationInput";
export type { RetireDeliveryDestinationInput } from "@/generated/RetireDeliveryDestinationInput";
export type { VerifyDeliveryDestinationInput } from "@/generated/VerifyDeliveryDestinationInput";
