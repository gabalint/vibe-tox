import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { useToast } from "@/components/Toast";
import type { RegisterSubject } from "@/generated/RegisterSubject";
import type { SubjectRegistered } from "@/generated/SubjectRegistered";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { useCommandMutation } from "@/lib/commands";
import { MATCH_SUBJECT_CANDIDATES, REGISTER_SUBJECT, SEARCH_SUBJECTS } from "./contract";
import { type PatientDetailsValues, toMatchQuery } from "./registration";

interface RegisterButtonProps {
  details: PatientDetailsValues;
  /** Exactly the candidates shown; the engine refuses if the set changed since. */
  candidates: readonly SubjectSummary[];
  isDisabled: boolean;
  /** The engine found a different candidate set: show the current one again. */
  onCandidatesChanged: () => void;
  onRegistered: (subjectId: string) => void;
}

/** Registers the patient, acknowledging the candidates shown (REQ-IDN-001). */
export function RegisterButton({
  details,
  candidates,
  isDisabled,
  onCandidatesChanged,
  onRegistered,
}: RegisterButtonProps) {
  const toast = useToast();
  const register = useCommandMutation<SubjectRegistered, { input: RegisterSubject }>(
    REGISTER_SUBJECT,
    { invalidates: [SEARCH_SUBJECTS, MATCH_SUBJECT_CANDIDATES] },
  );
  const submit = () =>
    register.mutate(
      {
        input: {
          ...toMatchQuery(details),
          source: details.source.trim(),
          acknowledgedCandidates: candidates.map((candidate) => candidate.subjectId),
        },
      },
      {
        onSuccess: ({ subjectId, identityHoldId }) => {
          toast.show(
            identityHoldId
              ? {
                  tone: "warning",
                  title: "Patient registered on an identity hold",
                  description: "Results cannot be released until the possible match is reviewed.",
                }
              : { tone: "success", title: "Patient registered" },
          );
          onRegistered(subjectId);
        },
        onError: (error) => {
          if (error.code === "conflict") onCandidatesChanged();
        },
      },
    );
  return (
    <>
      {register.error && (
        <ErrorState title="The patient was not registered" error={register.error} />
      )}
      <div>
        <Button
          variant="primary"
          isDisabled={isDisabled}
          isPending={register.isPending}
          onPress={submit}
        >
          Register patient
        </Button>
      </div>
    </>
  );
}
