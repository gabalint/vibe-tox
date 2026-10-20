import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { CommandError } from "@/generated/CommandError";
import type { SubjectCandidates } from "@/generated/SubjectCandidates";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { useCommandQuery } from "@/lib/commands";
import styles from "../accessioning.module.css";
import { SubjectPicker } from "../SubjectPicker";
import { MATCH_SUBJECT_CANDIDATES } from "./contract";
import { DetailsSummary } from "./DetailsSummary";
import { RegisterButton } from "./RegisterButton";
import { type PatientDetailsValues, toMatchQuery } from "./registration";

interface CandidateReviewProps {
  details: PatientDetailsValues;
  /** Back to the form, with the engine's refusal of the details if there was one. */
  onEdit: (error: CommandError | null) => void;
  onRegistered: (subjectId: string) => void;
  onChoose?: (subject: SubjectSummary) => void;
}

/**
 * Shows the existing patients that match before registering (REQ-IDN-001). An identifier
 * match is the same patient, so registration is not offered. Name and birth date candidates
 * are never linked: registering anyway needs an explicit acknowledgement, and the engine
 * then opens an identity hold.
 */
export function CandidateReview({ details, onEdit, onRegistered, onChoose }: CandidateReviewProps) {
  const matches = useCommandQuery<SubjectCandidates>(MATCH_SUBJECT_CANDIDATES, {
    input: toMatchQuery(details),
  });
  const editButton = <Button onPress={() => onEdit(null)}>Change details</Button>;
  if (matches.isPending) return <LoadingState label="Checking for existing patients…" />;
  if (matches.isError) {
    const refusedDetails = matches.error.code === "validation";
    return (
      <div className={styles.stack}>
        <ErrorState
          title="The check for existing patients could not be run"
          error={matches.error}
          onRetry={refusedDetails ? undefined : () => void matches.refetch()}
        />
        <div>
          <Button onPress={() => onEdit(refusedDetails ? matches.error : null)}>
            Change details
          </Button>
        </div>
      </div>
    );
  }
  const { identifierMatches, candidates } = matches.data;
  return (
    <div className={styles.stack}>
      <DetailsSummary details={details} action={editButton} />
      {identifierMatches.length > 0 ? (
        <IdentifierMatches subjects={identifierMatches} onChoose={onChoose} />
      ) : (
        <Candidates
          details={details}
          candidates={candidates}
          onCandidatesChanged={() => void matches.refetch()}
          onRegistered={onRegistered}
          onChoose={onChoose}
        />
      )}
    </div>
  );
}

function IdentifierMatches({
  subjects,
  onChoose,
}: {
  subjects: readonly SubjectSummary[];
  onChoose?: (subject: SubjectSummary) => void;
}) {
  return (
    <>
      <Alert tone="danger" title="This identifier already belongs to a patient">
        An identifier is unique within its assigning authority and type, so this is the same
        patient. Open the existing record instead of registering a new one.
      </Alert>
      <SubjectPicker
        label="Patients holding these identifiers"
        listLabel="Register patient"
        subjects={subjects}
        onChoose={onChoose}
      />
    </>
  );
}

interface CandidatesProps {
  details: PatientDetailsValues;
  candidates: readonly SubjectSummary[];
  onCandidatesChanged: () => void;
  onRegistered: (subjectId: string) => void;
  onChoose?: (subject: SubjectSummary) => void;
}

function Candidates({
  details,
  candidates,
  onCandidatesChanged,
  onRegistered,
  onChoose,
}: CandidatesProps) {
  const [acknowledged, setAcknowledged] = useState(false);
  const register = (
    <RegisterButton
      details={details}
      candidates={candidates}
      isDisabled={candidates.length > 0 && !acknowledged}
      onRegistered={onRegistered}
      onCandidatesChanged={() => {
        setAcknowledged(false);
        onCandidatesChanged();
      }}
    />
  );
  // The button stays in one place in the tree, so a refusal stays visible after the
  // candidates are shown again.
  return (
    <>
      {candidates.length === 0 ? (
        <Alert tone="info" title="No existing patient matches these details">
          No patient has these identifiers, or this name and birth date.
        </Alert>
      ) : (
        <>
          <Alert
            tone="warning"
            title={`${candidates.length} possible matches by name and birth date`}
          >
            These records share the name and birth date. They are not linked. If this is a different
            person, the new record is placed on an identity hold until someone with quality
            authority confirms that the records belong to different people.
          </Alert>
          <SubjectPicker
            label="Possible matches"
            listLabel="Register patient"
            subjects={candidates}
            onChoose={onChoose}
          />
          <Checkbox isSelected={acknowledged} onChange={setAcknowledged}>
            I compared these records; this is a different person
          </Checkbox>
        </>
      )}
      {register}
    </>
  );
}
