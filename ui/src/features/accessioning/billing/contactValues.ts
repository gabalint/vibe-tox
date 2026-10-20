import type { BillingContact } from "@/generated/BillingContact";
import type { BillingParty } from "@/generated/BillingParty";
import type { BillingRelationship } from "@/generated/BillingRelationship";
import { type Choice, checked, choiceOf, optionalText } from "@/lib/choices";

export const RELATIONSHIPS: readonly Choice<BillingRelationship>[] = [
  { id: "self", label: "Self" },
  { id: "spouse", label: "Spouse" },
  { id: "child", label: "Child" },
  { id: "parent", label: "Parent" },
  { id: "other", label: "Other" },
  { id: "unknown", label: "Unknown" },
];

export function contactValues(contact?: BillingContact | null) {
  return {
    line1: contact?.addressLines[0] ?? "",
    line2: contact?.addressLines[1] ?? "",
    line3: contact?.addressLines[2] ?? "",
    city: contact?.city ?? "",
    region: contact?.region ?? "",
    postalCode: contact?.postalCode ?? "",
    country: contact?.country ?? "",
    phone: contact?.phone ?? "",
    email: contact?.email ?? "",
  };
}

export function contactInput(values: ReturnType<typeof contactValues>): BillingContact {
  return {
    addressLines: [values.line1, values.line2, values.line3]
      .map((line) => line.trim())
      .filter(Boolean),
    city: optionalText(values.city),
    region: optionalText(values.region),
    postalCode: optionalText(values.postalCode),
    country: optionalText(values.country),
    phone: optionalText(values.phone),
    email: optionalText(values.email),
  };
}

export function partyValues(party?: BillingParty | null) {
  return {
    name: party?.name ?? "",
    relationship: party?.relationship ?? null,
    birthDate: party?.birthDate ?? "",
    contact: contactValues(party?.contact),
  };
}

export function partyInput(values: ReturnType<typeof partyValues>): BillingParty {
  return {
    name: values.name.trim(),
    relationship: checked(choiceOf(RELATIONSHIPS, values.relationship)),
    birthDate: optionalText(values.birthDate),
    contact: contactInput(values.contact),
  };
}
