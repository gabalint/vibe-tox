import { Facts } from "@/components/Facts";
import { Timestamp } from "@/components/Timestamp";
import type { PatientBillingProfile } from "@/generated/PatientBillingProfile";
import { ContactFacts } from "./BillingFacts";

export function PatientProfileFacts({ profile }: { profile: PatientBillingProfile | null }) {
  if (!profile) return <p>No patient billing profile recorded.</p>;
  return (
    <>
      <Facts
        items={[
          { label: "Profile revision", value: profile.version },
          { label: "Guarantor", value: profile.content.guarantor?.name ?? "Not supplied" },
          { label: "Source", value: profile.source },
          { label: "Change reason", value: profile.reason },
          { label: "Recorded", value: <Timestamp value={profile.recordedAt} /> },
        ]}
      />
      <ContactFacts contact={profile.content.contact} />
    </>
  );
}
