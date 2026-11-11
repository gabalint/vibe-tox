import { TextInput } from "@/components/FormFields";
import type { ApprovePartnerMappings } from "@/generated/ApprovePartnerMappings";
import type { PartnerMappings } from "@/generated/PartnerMappings";
import { CommandDialog } from "@/lib/CommandDialog";
import { APPROVE_MAPPINGS, MAPPING_QUERIES } from "../../contract";

/**
 * Approves every drafted change as the connection's next mapping version, with a reason
 * (REQ-INT-004). A second person approves, or the person who drafted where the laboratory's
 * separation-of-duties policy allows and they confirm it (ADR 0015). The new version applies to
 * messages not written yet.
 */
export function ApproveMappingsDialog({
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
    <CommandDialog<{ reason: string }, ApprovePartnerMappings, PartnerMappings>
      title={`Approve code mappings v${next}`}
      submitLabel="Approve"
      command={{ name: APPROVE_MAPPINGS, invalidates: MAPPING_QUERIES }}
      defaultValues={{ reason: "" }}
      toInput={(values, selfApproval) => ({
        connectionId,
        expectedRevision,
        reason: values.reason.trim(),
        selfApproval,
      })}
      successTitle={(_, result) => `Code mappings v${result.version} approved`}
      onClose={onClose}
    >
      <p>
        Every draft becomes part of v{next}. Messages not written yet use it; messages already
        written never change.
      </p>
      <TextInput
        name="reason"
        label="Reason"
        description="For example, codes confirmed with the partner's IT"
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}
