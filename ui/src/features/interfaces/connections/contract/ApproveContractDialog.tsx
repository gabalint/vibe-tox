import { TextInput } from "@/components/FormFields";
import type { ApprovePartnerContract } from "@/generated/ApprovePartnerContract";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import { CommandDialog } from "@/lib/CommandDialog";
import { APPROVE_CONTRACT, CONNECTION_QUERIES } from "../../contract";

/**
 * Approves the drafted contract as the connection's next version, with a reason
 * (REQ-INT-001). A second person approves, or the person who drafted it where the
 * laboratory's separation-of-duties policy allows and they confirm it (ADR 0015). The
 * acceptance tests then run again before patient data goes or comes.
 */
export function ApproveContractDialog({
  connectionId,
  expectedRevision,
  next,
  onClose,
}: {
  connectionId: string;
  /** The connection's revision: approving changes the connection. */
  expectedRevision: number;
  /** The version the approval makes. */
  next: number;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ reason: string }, ApprovePartnerContract, PartnerConnectionDetail>
      title={`Approve interface contract v${next}`}
      submitLabel="Approve"
      command={{ name: APPROVE_CONTRACT, invalidates: CONNECTION_QUERIES }}
      defaultValues={{ reason: "" }}
      toInput={(values, selfApproval) => ({
        connectionId,
        expectedRevision,
        reason: values.reason.trim(),
        selfApproval,
      })}
      successTitle={`Interface contract v${next} approved`}
      onClose={onClose}
    >
      <p>
        The draft becomes v{next}. No patient data goes or comes until the acceptance tests pass for
        it.
      </p>
      <TextInput
        name="reason"
        label="Reason"
        description="For example, terms confirmed with the partner's IT"
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}
