import type { RecordOption } from "@/components/RecordPicker";
import type { ClientView } from "@/generated/ClientView";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import type { OrderItemView } from "@/generated/OrderItemView";
import type { PractitionerView } from "@/generated/PractitionerView";

/** The reference lists that turn the IDs on an order into names for display. */
export interface OrderReferences {
  clients: readonly ClientView[];
  practitioners: readonly PractitionerView[];
  /** The current approved catalog versions. */
  orderables: readonly OrderableVersionView[];
}

/** `CODE Name` of an orderable version, as order entry offered it. */
export function orderableLabel(orderable: Pick<OrderableVersionView, "code" | "name">): string {
  return `${orderable.code} ${orderable.name}`;
}

/** The frozen ordered version remains recognizable after it leaves the active catalog. */
export function orderedTestName(
  item: Pick<OrderItemView, "orderableCode" | "orderableName">,
): string {
  return `${item.orderableCode} ${item.orderableName}`;
}

/** An orderable to choose, with the specimen types its components need. */
export function testChoiceLabel(orderable: OrderableVersionView): string {
  const specimens = [...new Set(orderable.components.map((each) => each.specimenTypeCode))];
  return `${orderableLabel(orderable)} (${specimens.join(", ")})`;
}

/** An orderable named by its stable ID, such as one inside a reflex scope. */
export function orderableName(references: OrderReferences, orderableId: string): string {
  const orderable = references.orderables.find((each) => each.orderableId === orderableId);
  return orderable ? orderableLabel(orderable) : "A test no longer offered";
}

function fullName({ familyName, givenNames }: PractitionerView): string {
  return givenNames ? `${familyName}, ${givenNames}` : familyName;
}

/** `Family, Given (NPI n)`: the NPI tells practitioners with the same name apart. */
function practitionerName(practitioner: PractitionerView): string {
  return `${fullName(practitioner)} (NPI ${practitioner.npi})`;
}

/** A practitioner to pick in order entry, shown and found by name and NPI. */
export function practitionerOption(practitioner: PractitionerView): RecordOption {
  return {
    id: practitioner.id,
    label: fullName(practitioner),
    description: `NPI ${practitioner.npi}`,
  };
}

export function practitionerById(references: OrderReferences, practitionerId: string): string {
  const practitioner = references.practitioners.find((each) => each.id === practitionerId);
  return practitioner ? practitionerName(practitioner) : "Unknown practitioner";
}

/** The client and, when the order names one, its location. */
export function clientById(
  references: OrderReferences,
  clientId: string,
  locationId: string | null,
): { client: string; location: string | null } {
  const client = references.clients.find((each) => each.id === clientId);
  const location = client?.locations.find((each) => each.id === locationId);
  return { client: client?.name ?? "Unknown client", location: location?.name ?? null };
}
