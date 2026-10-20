import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { DataGrid } from "@/components/DataGrid";
import { ErrorState } from "@/components/ErrorState";
import { FieldGrid, Form } from "@/components/Form";
import { DATE_FORMAT, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { AddSubjectIdentifier } from "@/generated/AddSubjectIdentifier";
import type { AssertDemographics } from "@/generated/AssertDemographics";
import type { DeactivateSubjectIdentifier } from "@/generated/DeactivateSubjectIdentifier";
import type { IdentityAssertionView } from "@/generated/IdentityAssertionView";
import type { SubjectCandidates } from "@/generated/SubjectCandidates";
import type { SubjectIdentifierView } from "@/generated/SubjectIdentifierView";
import type { SubjectView } from "@/generated/SubjectView";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { useCommandMutation, useCommandQuery, useHasCapability } from "@/lib/commands";
import { SubjectPicker } from "../SubjectPicker";

const INVALIDATES = [
  "get_subject",
  "search_subjects",
  "get_report_workspace",
  "get_order",
] as const;

export function PatientMaintenance({ subject }: { subject: SubjectView }) {
  const canEdit = useHasCapability("enter_orders");
  const canCorrect = useHasCapability("manage_quality");
  const [action, setActionState] = useState<
    "demographics" | "identifier" | SubjectIdentifierView | null
  >(null);
  const [reviewed, setReviewed] = useState(subject);
  const assertion = reviewed.assertions.at(-1);
  const setAction = (next: typeof action) => {
    setReviewed(subject);
    setActionState(next);
  };
  return (
    <>
      <Section
        title="Identity maintenance"
        actions={
          <>
            {canCorrect && (
              <Button
                isDisabled={subject.assertions.length === 0}
                onPress={() => setAction("demographics")}
              >
                Correct demographics
              </Button>
            )}
            {canEdit && <Button onPress={() => setAction("identifier")}>Add identifier</Button>}
          </>
        }
      >
        <p>
          Corrections preserve previous assertions and identifiers. A genuine duplicate remains on
          hold until resolved through the laboratory's identity process; confirming distinct people
          does not merge records.
        </p>
        {subject.assertions.length === 0 && (
          <p role="alert">
            Demographic evidence is unavailable. Refresh the patient record before correcting it.
          </p>
        )}
        <DataGrid
          label="Identifier history"
          rows={subject.identifiers}
          getRowId={(row) => row.id}
          columns={[
            { id: "value", header: "Identifier", cell: (row) => row.value },
            {
              id: "authority",
              header: "Assigning authority / type",
              cell: (row) => `${row.assigningAuthority} / ${row.identifierType}`,
            },
            { id: "added", header: "Added", cell: (row) => <Timestamp value={row.addedAt} /> },
            {
              id: "state",
              header: "State",
              cell: (row) => (row.active ? "Active" : `Deactivated: ${row.deactivationReason}`),
            },
            {
              id: "ended",
              header: "Deactivated",
              cell: (row) => (row.deactivatedAt ? <Timestamp value={row.deactivatedAt} /> : "—"),
            },
            {
              id: "action",
              header: "Action",
              cell: (row) =>
                canCorrect && row.active ? (
                  <Button onPress={() => setAction(row)}>Deactivate {row.identifierType}</Button>
                ) : null,
            },
          ]}
        />
      </Section>
      {action === "demographics" && assertion && (
        <DemographicCorrection
          key={reviewed.id}
          subject={reviewed}
          initial={assertion}
          onClose={() => setAction(null)}
        />
      )}
      {action === "identifier" && (
        <CommandDialog<
          { value: string; assigningAuthority: string; identifierType: string },
          AddSubjectIdentifier
        >
          title="Add patient identifier"
          submitLabel="Add identifier"
          command={{ name: "add_subject_identifier", invalidates: INVALIDATES }}
          defaultValues={{ value: "", assigningAuthority: "", identifierType: "" }}
          toInput={(v) => ({
            subjectId: subject.id,
            expectedRevision: reviewed.revision,
            identifier: {
              value: v.value.trim(),
              assigningAuthority: v.assigningAuthority.trim(),
              identifierType: v.identifierType.trim(),
            },
          })}
          successTitle="Identifier added"
          onClose={() => setAction(null)}
        >
          <TextInput name="value" label="Identifier" isRequired />
          <TextInput name="assigningAuthority" label="Assigning authority" isRequired />
          <TextInput name="identifierType" label="Identifier type" isRequired />
          <p>
            Values must be unique within their authority and type. An identifier already held by
            another patient is refused.
          </p>
        </CommandDialog>
      )}
      {action && typeof action === "object" && (
        <CommandDialog<{ reason: string }, DeactivateSubjectIdentifier>
          title="Deactivate identifier"
          submitLabel="Deactivate identifier"
          command={{ name: "deactivate_subject_identifier", invalidates: INVALIDATES }}
          defaultValues={{ reason: "" }}
          toInput={(v) => ({
            identifierId: action.id,
            expectedSubjectRevision: reviewed.revision,
            reason: v.reason.trim(),
          })}
          successTitle="Identifier deactivated"
          onClose={() => setAction(null)}
        >
          <p>
            {action.value} · {action.assigningAuthority} · {action.identifierType}. The original
            identifier remains in history.
          </p>
          <TextInput name="reason" label="Reason" isRequired />
        </CommandDialog>
      )}
    </>
  );
}

type Correction = {
  familyName: string;
  givenNames: string;
  birthDate: string;
  administrativeSex: string;
  source: string;
  reason: string;
};
function DemographicCorrection({
  subject,
  initial,
  onClose,
}: {
  subject: SubjectView;
  initial: IdentityAssertionView;
  onClose: () => void;
}) {
  const [revision] = useState(subject.revision);
  const form = useForm<Correction>({
    defaultValues: {
      familyName: initial.familyName,
      givenNames: initial.givenNames ?? "",
      birthDate: initial.birthDate,
      administrativeSex: initial.administrativeSex ?? "",
      source: "",
      reason: "",
    },
  });
  const [draft, setDraft] = useState<Correction | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const demographics = draft && {
    familyName: draft.familyName.trim(),
    givenNames: optionalText(draft.givenNames),
    birthDate: draft.birthDate.trim(),
    administrativeSex: optionalText(draft.administrativeSex),
  };
  const matches = useCommandQuery<SubjectCandidates>(
    "match_subject_candidates",
    { input: { demographics, identifiers: [] } },
    { enabled: draft !== null },
  );
  const save = useCommandMutation<unknown, { input: AssertDemographics }>("assert_demographics", {
    invalidates: INVALIDATES,
  });
  return (
    <Section title="Correct demographics">
      {!draft ? (
        <FormProvider {...form}>
          <Form
            aria-label="Correct demographics"
            onSubmit={form.handleSubmit((v) => {
              setDraft(v);
              setAcknowledged(false);
            })}
            actions={
              <>
                <Button type="submit" variant="primary">
                  Check corrected identity
                </Button>
                <Button onPress={onClose}>Cancel</Button>
              </>
            }
          >
            <FieldGrid>
              <TextInput name="familyName" label="Family name" isRequired />
              <TextInput name="givenNames" label="Given names" />
              <TextInput name="birthDate" label="Birth date" format={DATE_FORMAT} isRequired />
              <TextInput name="administrativeSex" label="Administrative sex" />
              <TextInput name="source" label="Source of corrected details" isRequired />
              <TextInput name="reason" label="Reason for correction" isRequired />
            </FieldGrid>
          </Form>
        </FormProvider>
      ) : (
        <QueryContent query={matches} what="corrected identity check">
          {(data) => {
            const candidates = data.candidates.filter((row) => row.subjectId !== subject.id);
            return (
              <>
                <p>
                  {draft.familyName}, {draft.givenNames} · {draft.birthDate}. Source: {draft.source}
                  . Reason: {draft.reason}.
                </p>
                {candidates.length > 0 && (
                  <>
                    <SubjectPicker
                      subjects={candidates}
                      label="Possible matching patients"
                      listLabel="Identity correction"
                    />
                    <Checkbox isSelected={acknowledged} onChange={setAcknowledged}>
                      I reviewed these possible matches; preserve an identity hold for review
                    </Checkbox>
                  </>
                )}
                <p>
                  Earlier assertions and issued reports remain unchanged. Changed clinical context
                  may require review or a report correction.
                </p>
                {save.error && <ErrorState title="Correction not saved" error={save.error} />}
                <Button
                  variant="primary"
                  isPending={save.isPending}
                  isDisabled={candidates.length > 0 && !acknowledged}
                  onPress={() => {
                    if (!demographics) return;
                    save.mutate(
                      {
                        input: {
                          subjectId: subject.id,
                          expectedRevision: revision,
                          demographics,
                          source: draft.source.trim(),
                          reason: draft.reason.trim(),
                          acknowledgedCandidates: candidates.map((row) => row.subjectId),
                        },
                      },
                      {
                        onSuccess: onClose,
                        onError: () => {
                          setAcknowledged(false);
                          void matches.refetch();
                        },
                      },
                    );
                  }}
                >
                  Save corrected demographics
                </Button>
                <Button isDisabled={save.isPending} onPress={() => setDraft(null)}>
                  Change details
                </Button>
                <Button isDisabled={save.isPending} onPress={onClose}>
                  Cancel
                </Button>
              </>
            );
          }}
        </QueryContent>
      )}
    </Section>
  );
}
