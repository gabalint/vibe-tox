import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { ExcursionPage } from "@/generated/ExcursionPage";
import type { ExcursionView } from "@/generated/ExcursionView";
import type { StorageLocationView } from "@/generated/StorageLocationView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import { AssessExcursionDialog } from "./AssessExcursionDialog";
import { EXCURSION_OUTCOMES, EXCURSION_STATES } from "./labels";
import { RecordExcursionDialog } from "./RecordExcursionDialog";
export function StorageExcursionsPage() {
  const client = useQueryClient();
  const [params, setParams] = useSearchParams();
  const raw = Number(params.get("offset") ?? 0);
  const offset = Number.isSafeInteger(raw) && raw >= 0 ? raw : 0;
  const excursionId = params.get("excursionId");
  const snapshot = params.get("snapshot");
  const [record, setRecord] = useState(false);
  const canRecord = useHasCapability("accession");
  const canConfigure = useHasCapability("manage_configuration");
  const query = useCommandQuery<ExcursionPage>("list_excursions", { input: { offset, snapshot } });
  const locations = useCommandQuery<StorageLocationView[]>("list_locations");
  const locationName = (id: string) =>
    locations.data?.find((r) => r.locationId === id)?.name ?? "Location details unavailable";
  const open = (id: string) =>
    setParams((old) => {
      const next = new URLSearchParams(old);
      next.set("excursionId", id);
      return next;
    });
  const page = (nextOffset: number, ceiling: string | null) =>
    setParams((old) => {
      const next = new URLSearchParams(old);
      next.set("offset", String(nextOffset));
      if (ceiling === null) next.delete("snapshot");
      else next.set("snapshot", ceiling);
      return next;
    });
  const refresh = () => {
    void client.invalidateQueries({ queryKey: ["list_excursions"], refetchType: "none" });
    if (offset === 0 && snapshot === null) return query.refetch();
    page(0, null);
  };
  return (
    <Page>
      <PageHeader
        title="Storage excursions"
        description="Record observed storage incidents, review affected material and preserve assessments."
        actions={
          <>
            {canConfigure && <Link to="/setup/material?tab=locations">Storage locations</Link>}
            {canRecord && <Button onPress={() => setRecord(true)}>Record excursion</Button>}
          </>
        }
      />
      <RefreshControl query={{ ...query, refetch: refresh }} />
      <QueryContent query={query} what="storage excursions">
        {(data) => (
          <>
            {data.items.length === 0 ? (
              <EmptyState
                title="No excursions on this page"
                description={
                  offset > 0
                    ? "Return to the previous page or refresh to see current incidents."
                    : canRecord
                      ? "Record an observed incident when source evidence is available."
                      : "No recorded incidents are available for quality review."
                }
              />
            ) : (
              <DataGrid
                label="Storage excursions"
                rows={data.items}
                getRowId={(r) => r.excursionId}
                onRowAction={(r) => open(r.excursionId)}
                columns={[
                  { id: "location", header: "Location", cell: (r) => locationName(r.locationId) },
                  {
                    id: "observed",
                    header: "Observed start",
                    cell: (r) => <Timestamp value={r.observedFrom} />,
                  },
                  { id: "description", header: "Condition", cell: (r) => r.description },
                  {
                    id: "state",
                    header: "State",
                    cell: (r) => labelOf(EXCURSION_STATES, r.state),
                  },
                ]}
              />
            )}
            <Button
              isDisabled={offset === 0}
              onPress={() => page(Math.max(0, offset - 50), data.snapshot)}
            >
              Previous excursions
            </Button>
            <Button isDisabled={!data.hasMore} onPress={() => page(offset + 50, data.snapshot)}>
              Next excursions
            </Button>
          </>
        )}
      </QueryContent>
      {excursionId && (
        <ExcursionDetail key={excursionId} id={excursionId} locationName={locationName} />
      )}
      {record && <RecordExcursionDialog onClose={() => setRecord(false)} onDone={open} />}
    </Page>
  );
}
function ExcursionDetail({
  id,
  locationName,
}: {
  id: string;
  locationName: (id: string) => string;
}) {
  const query = useCommandQuery<ExcursionView>("get_excursion", { input: { excursionId: id } });
  const quality = useHasCapability("manage_quality");
  const person = usePersonText();
  const [assess, setAssess] = useState(false);
  return (
    <Section title="Excursion details">
      <RefreshControl query={query} />
      <QueryContent query={query} what="excursion">
        {(entry) => (
          <>
            <p>
              {locationName(entry.locationId)}: {entry.description}
            </p>
            <p>
              Observed <Timestamp value={entry.observedFrom} /> to{" "}
              {entry.observedTo ? <Timestamp value={entry.observedTo} /> : "end unknown"}. Recorded
              by {person(entry.recordedBy, entry.recordedByDisplay)},{" "}
              <Timestamp value={entry.recordedAt} />.
            </p>
            <p>
              State: {labelOf(EXCURSION_STATES, entry.state)}.{" "}
              {entry.outcome && `Assessment: ${labelOf(EXCURSION_OUTCOMES, entry.outcome)}.`}{" "}
              {entry.assessmentReason}
            </p>
            {entry.assessedAt && (
              <p>
                Assessed
                {entry.assessedBy && ` by ${person(entry.assessedBy, entry.assessedByDisplay)}`},{" "}
                <Timestamp value={entry.assessedAt} />.
              </p>
            )}
            {entry.state === "open" && quality && (
              <Button onPress={() => setAssess(true)}>Assess excursion</Button>
            )}
            {entry.holds.length > 0 && (
              <p>
                {entry.holds.length} material holds recorded. Open each affected container by its
                barcode to review and release individual holds when authorized.
              </p>
            )}
            <ul aria-label="Affected material">
              {Object.entries(entry.affectedContainerLabels).map(([id, label]) => (
                <li key={id}>
                  <Link to={`/containers?labelCode=${encodeURIComponent(label)}`}>
                    {label}: material holds and history
                  </Link>
                </li>
              ))}
            </ul>
            {assess && <AssessExcursionDialog excursion={entry} onClose={() => setAssess(false)} />}
          </>
        )}
      </QueryContent>
    </Section>
  );
}
