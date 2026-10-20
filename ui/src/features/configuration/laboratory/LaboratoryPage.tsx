import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { LaboratoryProfileView } from "@/generated/LaboratoryProfileView";
import { GET_LABORATORY_PROFILE, useCommandQuery, useHasCapability } from "@/lib/commands";
import { MANAGE_CONFIGURATION } from "../contract";
import { useDialog } from "../useDialog";
import { ProfileDialog } from "./ProfileDialog";

/**
 * The performing laboratory's profile (REQ-RPT-001): what reports print about the
 * laboratory, and the time zone every screen shows times in. Any session reads it; people
 * with `manage_configuration` set a new version, with a reason after the first (L15).
 */
export function LaboratoryPage() {
  const query = useCommandQuery<LaboratoryProfileView | null>(GET_LABORATORY_PROFILE);
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const dialog = useDialog();
  return (
    <Page>
      <PageHeader
        title="Laboratory profile"
        description="The laboratory's details as reports print them, and its time zone."
        actions={
          canManage &&
          query.data && (
            <Button variant="primary" onPress={dialog.open}>
              Change profile
            </Button>
          )
        }
      />
      <QueryContent query={query} what="laboratory profile">
        {(profile) =>
          profile ? (
            <ProfileSection profile={profile} />
          ) : (
            <EmptyState
              title="No laboratory profile"
              description="Reports cannot be prepared, and times show in UTC, until the laboratory profile is set."
              action={
                canManage && (
                  <Button variant="primary" onPress={dialog.open}>
                    Set laboratory profile
                  </Button>
                )
              }
            />
          )
        }
      </QueryContent>
      {dialog.isOpen && <ProfileDialog profile={query.data ?? null} onClose={dialog.close} />}
    </Page>
  );
}

function ProfileSection({ profile }: { profile: LaboratoryProfileView }) {
  return (
    <Section title="Current profile">
      <Facts
        layout="compact"
        items={[
          { label: "Name", value: profile.name },
          { label: "Address", value: profile.addressLines.join(", ") },
          { label: "CLIA number", value: profile.cliaId },
          { label: "Director", value: profile.director },
          { label: "Phone", value: profile.phone },
          { label: "Time zone", value: profile.timeZone },
          { label: "Billing NPI", value: profile.billingNpi ?? "Not set" },
          { label: "Tax ID", value: profile.taxId ?? "Not set" },
          { label: "Version", value: String(profile.version) },
          {
            label: "Set",
            value: (
              <>
                <Timestamp value={profile.setAt} /> by {profile.setBy}
              </>
            ),
          },
          { label: "Reason", value: profile.reason ?? "None (first version)" },
        ]}
      />
    </Section>
  );
}
