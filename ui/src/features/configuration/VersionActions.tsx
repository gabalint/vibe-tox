import { useState } from "react";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { asksSelfApproval, SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import { TextField } from "@/components/TextField";
import { useToast } from "@/components/Toast";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation, useHasCapability } from "@/lib/commands";
import styles from "./configuration.module.css";
import { APPROVE_CONFIGURATION } from "./contract";

/** How one kind of catalog version is approved and retired. */
export interface VersionWorkflow<Approve, Retire> {
  /** Such as "method version"; used in titles and notices. */
  noun: string;
  approveCommand: CommandName;
  retireCommand: CommandName;
  invalidates: readonly CommandName[];
  /** `selfApproval`: the author confirmed approving their own draft (ADR 0015). */
  externalApprovalReference?: boolean;
  approveInput: (
    version: CatalogVersionSummary,
    selfApproval: boolean,
    reference?: string,
  ) => Approve;
  retireInput: (version: CatalogVersionSummary, reason: string) => Retire;
}

interface VersionActionsProps<Approve, Retire> {
  version: CatalogVersionSummary;
  workflow: VersionWorkflow<Approve, Retire>;
}

/**
 * Approve and retire, for people with `approve_configuration`. Shown by the version's status
 * as a convenience; the engine decides. It refuses an approval by the draft's author unless the
 * laboratory's separation-of-duties policy allows it, and then asks the author to confirm
 * (ADR 0015). The approver rule is stated beside Approve, not checked here.
 */
export function VersionActions<Approve, Retire>({
  version,
  workflow,
}: VersionActionsProps<Approve, Retire>) {
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  const [dialog, setDialog] = useState<"approve" | "retire" | null>(null);
  if (!canApprove) return null;
  const close = () => setDialog(null);
  return (
    <>
      {version.status === "draft" && (
        <>
          <Button variant="primary" onPress={() => setDialog("approve")}>
            Approve version
          </Button>
          <span className={styles.muted}>
            Approval needs someone other than the author, unless your laboratory's
            separation-of-duties policy allows self-approval.
          </span>
        </>
      )}
      {version.status === "approved" && (
        <Button variant="danger" onPress={() => setDialog("retire")}>
          Retire version
        </Button>
      )}
      {dialog === "approve" && (
        <ApproveDialog version={version} workflow={workflow} onClose={close} />
      )}
      {dialog === "retire" && (
        <RetireDialog version={version} workflow={workflow} onClose={close} />
      )}
    </>
  );
}

function ApproveDialog<Approve, Retire>({
  version,
  workflow,
  onClose,
}: VersionActionsProps<Approve, Retire> & { onClose: () => void }) {
  const toast = useToast();
  const [selfApproval, setSelfApproval] = useState(false);
  const [reference, setReference] = useState("");
  const approve = useCommandMutation<unknown, { input: Approve }>(workflow.approveCommand, {
    // An approval can clear the "starter content not reviewed" banner.
    invalidates: [...workflow.invalidates, "standard_catalog_status"],
  });
  const confirm = () =>
    approve.mutate(
      { input: workflow.approveInput(version, selfApproval, reference.trim() || undefined) },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Version ${version.version} approved` });
          onClose();
        },
      },
    );
  return (
    <ConfirmDialog
      title={`Approve ${workflow.noun} ${version.version}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel="Approve version"
      onConfirm={confirm}
      isPending={approve.isPending}
      variant="primary"
    >
      <p>
        The approved content can no longer change. It becomes the current version from now on, and
        the version it replaces stops being current. The person who wrote the draft cannot approve
        it unless your laboratory's separation-of-duties policy allows self-approval.
      </p>
      {workflow.externalApprovalReference && (
        <TextField
          name="externalApprovalReference"
          label="External approval reference (optional)"
          value={reference}
          onChange={setReference}
          errorMessage={
            approve.error?.fieldErrors.find((field) => field.field === "externalApprovalReference")
              ?.message
          }
          description="Record identifier or location of approval held outside the LIS; up to 1000 characters. Kept with this approval, never included in patient reports."
        />
      )}
      <SelfApprovalConfirm
        error={approve.error}
        isConfirmed={selfApproval}
        onChange={setSelfApproval}
      />
      {approve.error && !asksSelfApproval(approve.error) && (
        <ErrorState title="Approval refused" error={approve.error} />
      )}
    </ConfirmDialog>
  );
}

function RetireDialog<Approve, Retire>({
  version,
  workflow,
  onClose,
}: VersionActionsProps<Approve, Retire> & { onClose: () => void }) {
  const retire = useCommandMutation<unknown, { input: Retire }>(workflow.retireCommand, {
    invalidates: [...workflow.invalidates],
  });
  return (
    <ReasonConfirmDialog
      title={`Retire ${workflow.noun} ${version.version}?`}
      confirmLabel="Retire version"
      variant="danger"
      command={retire}
      input={(reason) => workflow.retireInput(version, reason)}
      successTitle={`Version ${version.version} retired`}
      onClose={onClose}
    >
      <p>
        It stops being current and cannot be used for new work. Records that already used it keep
        it, and the version itself is kept.
      </p>
    </ReasonConfirmDialog>
  );
}
