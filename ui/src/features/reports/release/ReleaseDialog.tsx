import { Controller, useFormContext } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Checkbox } from "@/components/Checkbox";
import { useFormatTime } from "@/lib/format";
import { ReauthConfirmDialog, type ReauthValues } from "@/lib/ReauthConfirmDialog";
import {
  type CandidateView,
  RELEASE_REPORT,
  REPORT_QUERIES,
  type ReleaseReportInput,
  type RevisionSummary,
} from "../contract";
import { COMPLETENESS, revisionText } from "../labels";

/** The partial-report acknowledgement (REQ-RPT-005), beside the password. */
interface Acknowledgement {
  acknowledgePartial: boolean;
}

type ReleaseValues = Acknowledgement & ReauthValues;

interface ReleaseDialogProps {
  candidate: CandidateView;
  onReleased: (revision: RevisionSummary) => void;
  onClose: () => void;
}

/**
 * Releases the candidate exactly as reviewed (REQ-RPT-001): the command carries the content
 * digest the releaser saw and the password, and the engine checks every gate again in the
 * release transaction. A partial report needs an explicit acknowledgement (REQ-RPT-005). A
 * releaser who reviewed results on the report confirms the self-release when the laboratory's
 * separation-of-duties policy allows it and the engine asks (ADR 0015).
 */
export function ReleaseDialog({ candidate, onReleased, onClose }: ReleaseDialogProps) {
  const formatTime = useFormatTime();
  const partial = candidate.completeness === "partial";
  const toInput = (
    { password, acknowledgePartial }: ReleaseValues,
    selfApproval: boolean,
  ): ReleaseReportInput => ({
    candidateId: candidate.candidateId,
    expectedRevision: candidate.revision,
    contentSha256: candidate.contentSha256,
    acknowledgePartial: partial && acknowledgePartial,
    password,
    selfApproval,
  });
  return (
    <ReauthConfirmDialog<Acknowledgement, ReleaseReportInput, RevisionSummary>
      title={`Release report ${candidate.reportNumber}`}
      confirmLabel="Release report"
      variant="primary"
      command={{ name: RELEASE_REPORT, invalidates: REPORT_QUERIES }}
      extraDefaults={{ acknowledgePartial: false }}
      toInput={toInput}
      successTitle={(revision) =>
        `Report ${candidate.reportNumber} revision ${revision.revisionNo} released`
      }
      onDone={onReleased}
      onClose={onClose}
      errorTitle="Not released"
    >
      <p>
        You release {revisionText(candidate.proposedRevisionNo, candidate.relationship)} as a{" "}
        {COMPLETENESS[candidate.completeness].label.toLowerCase()} report, signed by you, with
        exactly the content you reviewed. Once released it is an issued record: it can be
        supplemented, corrected or withdrawn only by a new revision. Its delivery obligations are
        created at once.
      </p>
      <p>
        If the engine refuses stale content, cancel this confirmation, Refresh the candidate, and
        review its gates and content again. A stale candidate needs a newly prepared candidate;
        refreshing does not authorize release.
      </p>
      <p>This candidate expires {formatTime(candidate.expiresAt)}.</p>
      {partial && <PartialAcknowledgement />}
    </ReauthConfirmDialog>
  );
}

function PartialAcknowledgement() {
  const { control, formState } = useFormContext<ReleaseValues>();
  const error = formState.errors.acknowledgePartial?.message;
  return (
    <>
      <Controller
        control={control}
        name="acknowledgePartial"
        rules={{ validate: (value) => value || "Acknowledge that the report is partial." }}
        render={({ field }) => (
          <Checkbox name={field.name} isSelected={field.value} onChange={field.onChange}>
            I acknowledge this is a partial report: the tests listed as not reported will follow in
            a later revision.
          </Checkbox>
        )}
      />
      {error && <Alert tone="danger">{error}</Alert>}
    </>
  );
}
