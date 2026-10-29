import { useEffect, useRef, useState } from "react";
import { useForm } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { FormDialog } from "@/components/FormDialog";
import { CheckList, TextInput } from "@/components/FormFields";
import { StatusBadge } from "@/components/StatusBadge";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { BackupRunView } from "@/generated/BackupRunView";
import type { CommandError } from "@/generated/CommandError";
import type { DownloadedUpdateView } from "@/generated/DownloadedUpdateView";
import type { DownloadUpdateInput } from "@/generated/DownloadUpdateInput";
import type { InstallUpdateInput } from "@/generated/InstallUpdateInput";
import type { RecordUpdateApprovalInput } from "@/generated/RecordUpdateApprovalInput";
import type { RequestBackupInput } from "@/generated/RequestBackupInput";
import type { TargetView } from "@/generated/TargetView";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import { useCommandMutation, useCommandQuery, useHasCapability } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import layout from "@/styles/layout.module.css";
import { AddTargetDialog } from "../backups/AddTargetDialog";
import { useExactJob } from "../backups/useExactJob";
import {
  APPROVE_DOWNLOADED_UPDATE,
  BACKUP_QUERIES,
  BACKUP_RESTORE,
  DOWNLOAD_UPDATE,
  INSTALL_UPDATE,
  LIST_BACKUP_RUNS,
  LIST_BACKUP_TARGETS,
  LIST_UPDATE_APPROVALS,
  REQUEST_BACKUP,
  VERSION_QUERIES,
} from "../contract";

const REVIEWED = "reviewed";

interface UpdateValues {
  releaseNotes: string[];
  riskClassText: string;
  notes: string;
  password: string;
}

type Step = "download" | "backup" | "approve" | "install";
const STEPS: readonly { id: Step; label: string }[] = [
  { id: "download", label: "Download the package and check its signature" },
  { id: "backup", label: "Back up this installation and verify the copy" },
  { id: "approve", label: "Record your approval of this package" },
  { id: "install", label: "Install and restart" },
];

interface UpdateDialogProps {
  build: AvailableBuild;
  onClose: () => void;
  onInstalling: () => void;
}

/**
 * Updates to `build` in one confirmation (ADR 0020; REQ-OPS-006, NFR-24). The app downloads
 * and verifies the package, takes and verifies a pre-update backup, records the approval of
 * that exact package and installs it. Each step is the engine's own command, so every rule
 * still applies; a failed step stops the update and leaves the dialog open to try again.
 */
export function UpdateDialog({ build, onClose, onInstalling }: UpdateDialogProps) {
  const canBackUp = useHasCapability(BACKUP_RESTORE);
  const [addingTarget, setAddingTarget] = useState(false);
  const [step, setStep] = useState<Step | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const [failure, setFailure] = useState<CommandError | null>(null);
  const values = useRef<UpdateValues | null>(null);
  const targets = useCommandQuery<TargetView[]>(LIST_BACKUP_TARGETS);
  const approvals = useCommandQuery<UpdateApprovalView[]>(LIST_UPDATE_APPROVALS);
  const download = useCommandMutation<DownloadedUpdateView, { input: DownloadUpdateInput }>(
    DOWNLOAD_UPDATE,
    { invalidates: [] },
  );
  const backup = useCommandMutation<BackupRunView, { input: RequestBackupInput }>(REQUEST_BACKUP, {
    invalidates: BACKUP_QUERIES,
  });
  const approve = useCommandMutation<UpdateApprovalView, { input: RecordUpdateApprovalInput }>(
    APPROVE_DOWNLOADED_UPDATE,
    { invalidates: VERSION_QUERIES },
  );
  const install = useCommandMutation<null, { input: InstallUpdateInput }>(INSTALL_UPDATE, {
    invalidates: VERSION_QUERIES,
  });
  const run = useExactJob<BackupRunView>(
    LIST_BACKUP_RUNS,
    { input: { limit: 1, beforeRequestedAt: null, runId } },
    runId !== null,
  );
  const form = useForm<UpdateValues>({
    defaultValues: {
      releaseNotes: [],
      riskClassText: "Not published (development build)",
      notes: `Updated to ${build.version} from Available updates.`,
      password: "",
    },
  });
  const hasTarget = targets.data?.some(
    (target) => !target.retired && target.available && target.purpose === "backup_and_anchors",
  );
  const stop = (error: CommandError) => {
    setFailure(error);
    setStep(null);
    setRunId(null);
  };
  const installNow = () => {
    setStep("install");
    install.mutate(
      { input: { version: build.version } },
      { onSuccess: onInstalling, onError: stop },
    );
  };

  const backedUp = run.data?.[0];
  // biome-ignore lint/correctness/useExhaustiveDependencies: the backup run's state drives the next step; the mutations are stable.
  useEffect(() => {
    if (step !== "backup" || !backedUp || backedUp.runId !== runId) return;
    if (backedUp.state === "requested" || backedUp.state === "running") return;
    const entered = values.current;
    if (!backedUp.updateApprovalEligible || !entered) {
      stop(backupFailed(backedUp));
      return;
    }
    setStep("approve");
    approve.mutate(
      {
        input: {
          version: build.version,
          riskClassText: entered.riskClassText.trim(),
          releaseNotesReviewed: true,
          preUpdateRunId: backedUp.runId,
          notes: entered.notes.trim(),
          password: entered.password,
        },
      },
      { onSuccess: installNow, onError: stop },
    );
  }, [step, backedUp, runId]);

  const ready = canBackUp && hasTarget;
  const submit = form.handleSubmit((entered) => {
    if (!ready) return;
    values.current = entered;
    setFailure(null);
    setStep("download");
    download.mutate(
      { input: { channel: build.channel, version: build.version } },
      {
        onError: stop,
        onSuccess: (staged) => {
          // An earlier approval of this exact package needs no second backup or approval.
          const approved = approvals.data?.some(
            (approval) =>
              approval.version === build.version &&
              !approval.withdrawn &&
              !approval.installed &&
              approval.packageFileName === staged.packageFileName,
          );
          if (approved) {
            installNow();
            return;
          }
          setStep("backup");
          backup.mutate(
            { input: { kind: "pre_update" } },
            { onSuccess: (requested) => setRunId(requested.runId), onError: stop },
          );
        },
      },
    );
  });

  const busy = step !== null;
  return (
    <>
      <FormDialog
        form={form}
        title={`Update to version ${build.version}`}
        submitLabel={busy ? "Updating…" : "Update now"}
        errorTitle="Not updated"
        command={{ error: failure, isPending: busy }}
        onSubmit={(event) => void submit(event)}
        onClose={onClose}
      >
        <div className={layout.stack}>
          <p>
            The app downloads this version, backs up your data first, records your approval and
            installs it. It then restarts, and the new version awaits acceptance for clinical use.
          </p>
          {!canBackUp && (
            <Alert tone="warning" title="Your account cannot start backups">
              An update needs a verified backup first. Ask someone with Backup and restore authority
              to update, or to give your account that authority.
            </Alert>
          )}
          {canBackUp && targets.isSuccess && !hasTarget && (
            <Alert tone="warning" title="Choose where backups are saved">
              This installation has no backup folder yet, and an update needs a verified backup
              first. <Button onPress={() => setAddingTarget(true)}>Add backup folder</Button>
            </Alert>
          )}
          {busy ? (
            <Progress step={step} />
          ) : (
            <>
              <CheckList
                name="releaseNotes"
                legend="Changes"
                options={[{ id: REVIEWED, label: "I have reviewed the changes in this version" }]}
                requiredMessage="Confirm that you reviewed the changes."
              />
              <TextInput
                name="riskClassText"
                label="Risk class, as published"
                description="Development builds publish none. Change it if the publisher gave one."
                isRequired
              />
              <TextInput
                name="notes"
                label="Approval notes"
                description="Recorded with the approval, such as what was checked."
                isRequired
              />
              <AuthenticationField />
            </>
          )}
        </div>
      </FormDialog>
      {addingTarget && <AddTargetDialog onClose={() => setAddingTarget(false)} />}
    </>
  );
}

function Progress({ step }: { step: Step | null }) {
  const current = STEPS.findIndex((each) => each.id === step);
  return (
    <ol aria-label="Update progress" className={layout.list}>
      {STEPS.map((each, index) => (
        <li key={each.id}>
          {each.label} {index < current && <StatusBadge tone="success">Done</StatusBadge>}
          {index === current && <StatusBadge tone="info">In progress</StatusBadge>}
        </li>
      ))}
    </ol>
  );
}

function backupFailed(run: BackupRunView): CommandError {
  return {
    code: "conflict",
    message:
      run.state === "succeeded"
        ? "The backup finished without a verified copy. Check the backup folder in System → Backups, then try again."
        : "The backup failed. Check the backup folder in System → Backups, then try again.",
    fieldErrors: [],
    gateReasons: [],
    correlationId: run.runId,
  };
}
