import { CheckList, TextInput } from "@/components/FormFields";
import type { DownloadedUpdateView } from "@/generated/DownloadedUpdateView";
import type { RecordUpdateApprovalInput } from "@/generated/RecordUpdateApprovalInput";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import { checked } from "@/lib/choices";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { APPROVE_DOWNLOADED_UPDATE, RECORD_UPDATE_APPROVAL, VERSION_QUERIES } from "../contract";
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

interface RecordApprovalDialogProps {
  onClose: () => void;
  /**
   * The package the app downloaded and verified (ADR 0020). Its version is fixed and no file
   * is chosen; without it, the package is chosen in a file dialog.
   */
  downloaded?: DownloadedUpdateView;
  onApproved?: (approval: UpdateApprovalView) => void;
}

/**
 * Records the approval of an update before it is installed (REQ-OPS-006, design brief §7).
 * The pre-update backup run is chosen from the engine's runs, never typed. The engine reads the
 * package, from the file dialog it opens or from the app's download, computes its SHA-256 and
 * checks that the chosen run is verified and recent.
 */
export function RecordApprovalDialog({
  onClose,
  downloaded,
  onApproved,
}: RecordApprovalDialogProps) {
  return (
    <ReauthConfirmDialog<ApprovalValues, RecordUpdateApprovalInput, UpdateApprovalView>
      title={downloaded ? `Approve version ${downloaded.version}` : "Record update approval"}
      confirmLabel={downloaded ? "Approve and install" : "Choose the update package"}
      variant="primary"
      command={{
        name: downloaded ? APPROVE_DOWNLOADED_UPDATE : RECORD_UPDATE_APPROVAL,
        invalidates: VERSION_QUERIES,
      }}
      extraDefaults={{
        version: downloaded?.version ?? "",
        riskClassText: "",
        releaseNotes: [],
        preUpdateRunId: null,
        notes: "",
      }}
      toInput={(values) => ({
        version: downloaded?.version ?? values.version.trim(),
        riskClassText: values.riskClassText.trim(),
        releaseNotesReviewed: true,
        preUpdateRunId: checked(values.preUpdateRunId),
        notes: values.notes.trim(),
        password: values.password,
      })}
      successTitle={(approval) => `Update approved: ${approval.packageFileName}`}
      onDone={onApproved}
      onClose={onClose}
      errorTitle="Not approved"
      cancelledText="No package was chosen, so no approval was recorded."
    >
      {downloaded ? (
        <p>
          Approve the downloaded package {downloaded.packageFileName}. Its signature was verified
          when it was downloaded; its SHA-256 fingerprint is recorded with the approval. The app
          installs it and restarts once the approval is recorded.
        </p>
      ) : (
        <p>
          Approve a downloaded update before installation. After confirmation, choose its package to
          record its SHA-256 fingerprint. For the installed app, use Accept running version.
        </p>
      )}
      {!downloaded && (
        <TextInput
          name="version"
          label="Version to install"
          description="Copy from the update’s release notes, e.g. 1.2.0. Package version detection is not available yet."
          isRequired
        />
      )}
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
