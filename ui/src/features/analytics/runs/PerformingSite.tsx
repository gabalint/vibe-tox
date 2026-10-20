import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { LaboratoryProfileView } from "@/generated/LaboratoryProfileView";
import type { RecordRunPerformingSiteInput } from "@/generated/RecordRunPerformingSiteInput";
import type { RunDetail } from "@/generated/RunDetail";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { MANAGE_QUALITY, RUN_QUERIES } from "../contract";

export function PerformingSite({ run }: { run: RunDetail }) {
  const [resolving, setResolving] = useState(false);
  const canResolve = useHasCapability(MANAGE_QUALITY);
  const profiles = useCommandQuery<LaboratoryProfileView[]>("list_laboratory_profiles");
  const known = profiles.data?.find((profile) => profile.profileId === run.laboratoryProfileId);
  return (
    <>
      <Facts
        items={[
          {
            label: "Performing site",
            value: known
              ? `${known.name}, ${known.addressLines.join(", ")} (profile ${known.version})`
              : run.laboratoryProfileId
                ? "Loading recorded site…"
                : "Unknown: resolve from retained evidence before reporting",
          },
          ...(run.performingSiteSource
            ? [
                { label: "Site evidence", value: run.performingSiteSource },
                { label: "Site resolution reason", value: run.performingSiteReason ?? "" },
              ]
            : []),
        ]}
      />
      {canResolve && !run.laboratoryProfileId && (
        <Button onPress={() => setResolving(true)}>Record performing site</Button>
      )}
      {profiles.isError && (
        <QueryContent query={profiles} what="laboratory profiles">
          {() => null}
        </QueryContent>
      )}
      {resolving && (
        <CommandDialog<
          { laboratoryProfileId: string | null; sourceReference: string; reason: string },
          RecordRunPerformingSiteInput,
          RunDetail
        >
          title="Record performing site"
          submitLabel="Record performing site"
          command={{ name: "record_run_performing_site", invalidates: RUN_QUERIES }}
          defaultValues={{ laboratoryProfileId: null, sourceReference: "", reason: "" }}
          toInput={(values) => ({
            runId: run.runId,
            expectedRevision: run.revision,
            laboratoryProfileId: checked(values.laboratoryProfileId),
            sourceReference: values.sourceReference.trim(),
            reason: values.reason.trim(),
          })}
          successTitle="Performing site recorded"
          onClose={() => setResolving(false)}
        >
          <p>
            Identify the site where testing actually occurred from retained evidence. This permanent
            resolution does not change earlier issued reports and cannot replace a known site.
          </p>
          <QueryContent query={profiles} what="laboratory profiles">
            {(rows) => (
              <ChoiceInput
                name="laboratoryProfileId"
                label="Historical performing site"
                isRequired
                options={rows.map((profile) => ({
                  id: profile.profileId,
                  label: `${profile.name} · ${profile.addressLines.join(", ")} · profile ${profile.version}`,
                }))}
              />
            )}
          </QueryContent>
          <TextInput
            name="sourceReference"
            label="Source evidence reference"
            isRequired
            description="Retained bench or laboratory record supporting the site, up to 1000 characters."
          />
          <TextInput
            name="reason"
            label="Reason"
            isRequired
            description="Explain how the evidence establishes this run’s actual site, up to 1000 characters."
          />
        </CommandDialog>
      )}
    </>
  );
}
