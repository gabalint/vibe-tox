import { Facts } from "@/components/Facts";
import type { BillingContact } from "@/generated/BillingContact";
import type { BillingParty } from "@/generated/BillingParty";
import { labelOf } from "@/lib/choices";
import { useFormatBirthDate } from "@/lib/format";
import { RELATIONSHIPS } from "./contactValues";

export function ContactFacts({ contact }: { contact: BillingContact }) {
  return (
    <Facts
      items={[
        {
          label: "Address",
          value:
            [
              ...contact.addressLines,
              contact.city,
              contact.region,
              contact.postalCode,
              contact.country,
            ]
              .filter(Boolean)
              .join(", ") || "Not supplied",
        },
        { label: "Phone", value: contact.phone ?? "Not supplied" },
        { label: "Email", value: contact.email ?? "Not supplied" },
      ]}
    />
  );
}

export function PartyFacts({ party }: { party: BillingParty }) {
  const birthDate = useFormatBirthDate();
  return (
    <>
      <Facts
        items={[
          { label: "Name", value: party.name },
          { label: "Patient relationship", value: labelOf(RELATIONSHIPS, party.relationship) },
          {
            label: "Birth date",
            value: party.birthDate ? birthDate(party.birthDate) : "Not supplied",
          },
        ]}
      />
      <ContactFacts contact={party.contact} />
    </>
  );
}
