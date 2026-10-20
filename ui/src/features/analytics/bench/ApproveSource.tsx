import { type ReactNode, useState } from "react";
import { Button } from "@/components/Button";
import type { ApproveSourceInput } from "@/generated/ApproveSourceInput";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useHasCapability } from "@/lib/commands";
import { APPROVE_CONFIGURATION, SOURCE_QUERIES } from "../contract";

interface ApproveSourceProps {
  source: SourceRegistrationView;
  /** What approval means for this source, in laboratory terms. */
  children: ReactNode;
}

/**
 * Approves a draft source (REQ-ANA-003), offered to approvers when the engine allows it. The
 * engine refuses an approval by the source's author unless the laboratory's separation-of-duties
 * policy allows it, and then asks the author to confirm (ADR 0015).
 */
export function ApproveSource({ source, children }: ApproveSourceProps) {
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  const [isApproving, setApproving] = useState(false);
  if (!canApprove || !source.allowedActions.includes("approve_source")) return null;
  const name = `${source.senderName} ${source.senderVersion}`;
  return (
    <>
      <Button variant="primary" onPress={() => setApproving(true)}>
        Approve source
      </Button>
      {isApproving && (
        <CommandConfirmDialog<ApproveSourceInput>
          title={`Approve ${name}`}
          confirmLabel="Approve source"
          variant="primary"
          command={{ name: "approve_source", invalidates: SOURCE_QUERIES }}
          selfApprovalInput={(selfApproval) => ({
            registrationId: source.registrationId,
            expectedRevision: source.revision,
            selfApproval,
          })}
          successTitle={`${name} approved`}
          onClose={() => setApproving(false)}
        >
          {children}
        </CommandConfirmDialog>
      )}
    </>
  );
}
