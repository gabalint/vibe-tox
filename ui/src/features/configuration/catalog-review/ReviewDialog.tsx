import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { TextInput } from "@/components/FormFields/TextInput";
import { SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import type { ApproveReviewedCatalogInput } from "@/generated/ApproveReviewedCatalogInput";
import type { CatalogPreviewSummary } from "@/generated/CatalogPreviewSummary";
import type { CatalogReviewApproved } from "@/generated/CatalogReviewApproved";
import { CommandRefusal } from "@/lib/CommandRefusal";
import { useCommandMutation } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import layout from "@/styles/layout.module.css";
import { BlankCutoffChecks, cutoffKey } from "./BlankCutoffChecks";
import { APPROVE_REVIEWED_CATALOG, REVIEW_INVALIDATES } from "./contract";
import { reviewRefusal } from "./refusals";

interface ReviewDialogProps {
  preview: CatalogPreviewSummary;
  onApproved: (approved: CatalogReviewApproved) => void;
  onClose: () => void;
}

/**
 * Approve reviewed catalog (REQ-OPS-015): approves exactly the drafts one Catalog Preview
 * lists, after each blank cutoff it lists is confirmed as not applicable (REQ-ANA-009) and the
 * reviewer's password is entered again (REQ-IDN-004). The engine checks the laboratory's
 * separation-of-duties policy, asking a reviewer who wrote listed drafts to confirm where it
 * allows that (ADR 0015), and refuses a catalog changed since the report.
 */
export function ReviewDialog({ preview, onApproved, onClose }: ReviewDialogProps) {
  const [confirmed, setConfirmed] = useState<ReadonlySet<string>>(new Set());
  const [unconfirmed, setUnconfirmed] = useState(false);
  const [selfApproval, setSelfApproval] = useState(false);
  const form = useForm<{ password: string; externalApprovalReference: string }>({
    defaultValues: { password: "", externalApprovalReference: "" },
  });
  const approve = useCommandMutation<CatalogReviewApproved, { input: ApproveReviewedCatalogInput }>(
    APPROVE_REVIEWED_CATALOG,
    { invalidates: REVIEW_INVALIDATES },
  );
  const allConfirmed = preview.blankCutoffs.every((cutoff) => confirmed.has(cutoffKey(cutoff)));
  const submit = form.handleSubmit(({ password, externalApprovalReference }) => {
    setUnconfirmed(!allConfirmed);
    if (!allConfirmed) return;
    const confirmedBlankCutoffs = preview.blankCutoffs.map(({ methodVersionId, analyteId }) => ({
      methodVersionId,
      analyteId,
    }));
    approve.mutate(
      {
        input: {
          previewId: preview.id,
          confirmedBlankCutoffs,
          selfApproval,
          password,
          externalApprovalReference: externalApprovalReference.trim() || undefined,
        },
      },
      {
        onSuccess: (result) => {
          onApproved(result);
          onClose();
        },
        onError: (error) => {
          for (const field of error.fieldErrors) {
            if (field.field === "password" || field.field === "externalApprovalReference") {
              form.setError(field.field, { message: field.message }, { shouldFocus: true });
            }
          }
        },
      },
    );
  });
  const lead = approve.error && reviewRefusal(approve.error);
  return (
    <ConfirmDialog
      title="Approve reviewed catalog"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={`Approve ${preview.drafts} drafts`}
      onConfirm={() => void submit()}
      isPending={approve.isPending}
      variant="primary"
    >
      <div className={layout.stack}>
        <p>
          Approves the {preview.drafts} drafts listed in report{" "}
          <span className={layout.code}>{preview.id}</span>: validity policies, then methods, then
          tests and panels. Approval makes these versions available for use and records your
          decision under the laboratory's separation-of-duties policy. Later changes need new
          drafts. Confirm that this is the report you reviewed.
        </p>
        <BlankCutoffChecks
          cutoffs={preview.blankCutoffs}
          confirmed={confirmed}
          onChange={setConfirmed}
        />
        {unconfirmed && !allConfirmed && (
          <Alert tone="warning">Confirm each listed blank cutoff as not applicable.</Alert>
        )}
        <FormProvider {...form}>
          <TextInput
            name="externalApprovalReference"
            label="External approval reference (optional)"
            description="Approval record identifier or location held outside the LIS. Up to 1000 characters; never included in patient reports."
            validate={(value) => Array.from(value).length <= 1000 || "Use at most 1000 characters."}
          />
          <AuthenticationField />
        </FormProvider>
        <SelfApprovalConfirm
          error={approve.error}
          isConfirmed={selfApproval}
          onChange={setSelfApproval}
        />
        {approve.error && (lead || approve.error.fieldErrors.length === 0) && (
          <CommandRefusal title="Not approved" error={approve.error} lead={lead} />
        )}
      </div>
    </ConfirmDialog>
  );
}
