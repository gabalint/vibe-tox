import { CheckList, TextInput } from "@/components/FormFields";
import type { RecordUpdateApprovalInput } from "@/generated/RecordUpdateApprovalInput";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import { checked } from "@/lib/choices";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { RECORD_UPDATE_APPROVAL, VERSION_QUERIES } from "../contract";
import { PreUpdateRunField } from "./PreUpdateRunField";

const REVIEWED = "reviewed";

interface ApprovalValues {
  version: string;
  riskClassText: string;
  /** Holds `REVIEWED` once the release notes are confirmed as reviewed. */
  releaseNotes: string[];
  /** The chosen run's ID; `null` until a run is chosen. */
  preUpdateRunId: string | null;
  notes: string;
}

/**
 * Records the approval of an update before it is installed (REQ-OPS-006, design brief §7).
 * The pre-update backup run is chosen from the engine's runs, never typed. The engine opens
 * the file dialog for the package, computes its SHA-256 and checks that the chosen run is
 * verified and recent.
 */
export function RecordApprovalDialog({ onClose }: { onClose: () => void }) {
  return (
    <ReauthConfirmDialog<ApprovalValues, RecordUpdateApprovalInput, UpdateApprovalView>
      title="Record update approval"
      confirmLabel="Choose the update package"
      variant="primary"
      command={{ name: RECORD_UPDATE_APPROVAL, invalidates: VERSION_QUERIES }}
      extraDefaults={{
        version: "",
        riskClassText: "",
        releaseNotes: [],
        preUpdateRunId: null,
        notes: "",
      }}
      toInput={(values) => ({
        version: values.version.trim(),
        riskClassText: values.riskClassText.trim(),
        releaseNotesReviewed: true,
        preUpdateRunId: checked(values.preUpdateRunId),
        notes: values.notes.trim(),
        password: values.password,
      })}
      successTitle={(approval) => `Update approved: ${approval.packageFileName}`}
      onClose={onClose}
      errorTitle="Not approved"
      cancelledText="No package was chosen, so no approval was recorded."
    >
      <p>
        Approve a downloaded update before installation. After confirmation, choose its package to
        record its SHA-256 fingerprint. For the installed app, use Accept running version.
      </p>
      <TextInput
        name="version"
        label="Version to install"
        description="Copy from the update’s release notes, e.g. 1.2.0. Package version detection is not available yet."
        isRequired
      />
      <TextInput
        name="riskClassText"
        label="Risk class, as published"
        description="From the release notes. Ask the publisher if it is missing."
        isRequired
      />
      <CheckList
        name="releaseNotes"
        legend="Release notes"
        options={[{ id: REVIEWED, label: "I have reviewed the release notes for this version" }]}
        requiredMessage="Confirm that you reviewed the release notes."
      />
      <PreUpdateRunField name="preUpdateRunId" />
      <TextInput
        name="notes"
        label="Approval notes"
        description="Such as what was reviewed and who agreed. Recorded with the approval."
        isRequired
      />
    </ReauthConfirmDialog>
  );
}
