import { useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { DataGrid } from "@/components/DataGrid";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { IntakeDeferralView } from "@/generated/IntakeDeferralView";
import type { RecordIntakeDeferralInput } from "@/generated/RecordIntakeDeferralInput";
import type { ResolveIntakeDeferralInput } from "@/generated/ResolveIntakeDeferralInput";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { useCommandPages } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { usePersonText } from "@/lib/person";
import { PatientDirectory } from "./PatientDirectory";

export function IncompleteIntakePage() {
  const navigate = useNavigate();
  const person = usePersonText();
  const [includeClosed, setIncludeClosed] = useState(false);
  const query = useCommandPages<IntakeDeferralView, string>("list_intake_deferrals", {
    key: { includeClosed },
    pageArgs: (afterId) => ({ input: { includeClosed, afterId: afterId ?? null } }),
    nextCursor: (page) => (page.length === 50 ? page.at(-1)?.intakeId : undefined),
  });
  const [recording, setRecording] = useState(false);
  const [selected, setSelected] = useState<IntakeDeferralView | null>(null);
  const [resolving, setResolving] = useState<"declined" | "resolved" | null>(null);
  const [patient, setPatient] = useState<SubjectSummary | null>(null);
  const dispositionSubjectId = resolving === "declined" ? null : patient?.subjectId;
  const close = () => {
    setResolving(null);
    setPatient(null);
    setSelected(null);
  };
  const rows = query.data?.pages.flat() ?? [];
  return (
    <Page>
      <PageHeader
        title="Incomplete identity intake"
        breadcrumbs={[{ label: "Patients", href: "/patients" }]}
        description="Retain supplied identity evidence while a complete birth date is unavailable. These records do not authorize orders or testing."
        actions={
          <Button variant="primary" onPress={() => setRecording(true)}>
            Record incomplete intake
          </Button>
        }
      />
      <Checkbox isSelected={includeClosed} onChange={setIncludeClosed}>
        Include resolved and declined intake
      </Checkbox>
      <RefreshControl
        query={{
          ...query,
          refetch: () => {
            close();
            return query.refetch();
          },
        }}
      />
      <QueryContent query={query} what="incomplete intake">
        {() => (
          <>
            <DataGrid
              label="Incomplete intake records"
              rows={rows}
              getRowId={(r) => r.intakeId}
              onRowAction={(r) => {
                setSelected(r);
                setResolving(null);
                setPatient(null);
              }}
              columns={[
                { id: "name", header: "Name as supplied", cell: (r) => r.suppliedName },
                {
                  id: "evidence",
                  header: "Birth information as supplied",
                  cell: (r) =>
                    `${r.birthEvidenceKind.replaceAll("_", " ")}: ${r.birthEvidenceText ?? "Not supplied"}`,
                },
                { id: "source", header: "Source reference", cell: (r) => r.sourceReference },
                { id: "time", header: "Received", cell: (r) => <Timestamp value={r.recordedAt} /> },
                {
                  id: "state",
                  header: "Disposition",
                  cell: (r) => r.disposition ?? "Awaiting verified identity",
                },
              ]}
            />
            {query.hasNextPage && (
              <Button
                isPending={query.isFetchingNextPage}
                onPress={() => void query.fetchNextPage()}
              >
                Load more intake records
              </Button>
            )}
          </>
        )}
      </QueryContent>
      {selected && (
        <Section
          title={`Intake: ${selected.suppliedName}`}
          actions={<Button onPress={close}>Close intake</Button>}
        >
          <p>
            Original evidence: {selected.birthEvidenceText ?? "Birth information unknown"}. Source:{" "}
            {selected.sourceReference}. Recorded by{" "}
            {person(selected.recordedBy, selected.recordedByDisplay)}. Reason: {selected.reason}.
          </p>
          {selected.disposition ? (
            <>
              <p>
                {selected.disposition}: {selected.dispositionReason}. Source:{" "}
                {selected.dispositionSource}. Recorded by{" "}
                {selected.disposedBy && person(selected.disposedBy, selected.disposedByDisplay)}{" "}
                {selected.disposedAt && <Timestamp value={selected.disposedAt} />}.
              </p>
              {selected.subjectId && (
                <Button onPress={() => navigate(`/patients/${selected.subjectId}`)}>
                  Open resolved patient
                </Button>
              )}
            </>
          ) : (
            <>
              <Button onPress={() => setResolving("resolved")}>
                Resolve with verified patient
              </Button>
              <Button onPress={() => setResolving("declined")}>Record declined intake</Button>
              {resolving === "resolved" && !patient && (
                <Section title="Choose verified patient">
                  <p>
                    First obtain source-backed complete identity. Register a patient separately if
                    needed, then return here and explicitly select the verified record. Never derive
                    a date from age or partial dates.
                  </p>
                  <Button onPress={() => navigate("/patients/register")}>
                    Register verified patient
                  </Button>
                  <PatientDirectory onChoose={setPatient} />
                </Section>
              )}
            </>
          )}
        </Section>
      )}
      {recording && (
        <CommandDialog<RecordIntakeDeferralInput, RecordIntakeDeferralInput>
          title="Record incomplete identity intake"
          submitLabel="Retain incomplete intake"
          command={{ name: "record_intake_deferral", invalidates: ["list_intake_deferrals"] }}
          defaultValues={{
            suppliedName: "",
            birthEvidenceText: "",
            sourceReference: "",
            reason: "",
          }}
          toInput={(v) => ({
            ...v,
            suppliedName: v.suppliedName.trim(),
            birthEvidenceText: optionalText(v.birthEvidenceText ?? ""),
            sourceReference: v.sourceReference.trim(),
            reason: v.reason.trim(),
          })}
          successTitle="Incomplete intake retained"
          onClose={() => setRecording(false)}
        >
          <TextInput name="suppliedName" label="Name as supplied" isRequired />
          <ChoiceInput
            name="birthEvidenceKind"
            label="Available birth information"
            options={[
              { id: "unknown", label: "Unknown" },
              { id: "partial_date", label: "Partial date" },
              { id: "supplied_age", label: "Supplied age" },
            ]}
            isRequired
          />
          <TextInput
            name="birthEvidenceText"
            label="Exact supplied date or age text"
            description="Required for partial date or age. Do not complete a date or convert age to a birth date."
          />
          <TextInput name="sourceReference" label="Source reference" isRequired />
          <TextInput name="reason" label="Reason intake is deferred" isRequired />
        </CommandDialog>
      )}
      {selected && resolving && dispositionSubjectId !== undefined && (
        <CommandDialog<{ sourceReference: string; reason: string }, ResolveIntakeDeferralInput>
          title={
            resolving === "resolved"
              ? "Resolve incomplete identity intake"
              : "Record declined intake"
          }
          submitLabel="Record disposition"
          command={{ name: "resolve_intake_deferral", invalidates: ["list_intake_deferrals"] }}
          defaultValues={{ sourceReference: "", reason: "" }}
          toInput={(v) => ({
            intakeId: selected.intakeId,
            expectedRevision: selected.revision,
            disposition: resolving,
            subjectId: dispositionSubjectId,
            sourceReference: v.sourceReference.trim(),
            reason: v.reason.trim(),
          })}
          successTitle="Intake disposition recorded"
          onClose={close}
        >
          {patient && <IdentityBanner subject={patient} />}
          <p>
            The supplied evidence remains unchanged. This records a terminal disposition; it does
            not create an order, merge patients or remove identity holds.
          </p>
          <TextInput name="sourceReference" label="Verification or decline source" isRequired />
          <TextInput name="reason" label="Disposition reason" isRequired />
        </CommandDialog>
      )}
    </Page>
  );
}
