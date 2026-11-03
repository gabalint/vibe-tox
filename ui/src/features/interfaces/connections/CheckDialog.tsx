import { Alert } from "@/components/Alert";
import { TextInput } from "@/components/FormFields";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { RecordPartnerDestinationCheck } from "@/generated/RecordPartnerDestinationCheck";
import { CommandDialog } from "@/lib/CommandDialog";
import { CONNECTION_QUERIES, MESSAGE_QUERIES, RECORD_CHECK } from "../contract";

interface CheckValues {
  method: string;
}

/**
 * Records how the connection's current folder was confirmed as the partner's (REQ-RPT-012),
 * before any patient data is written there. A second person records it, or the person who
 * chose the folder where the laboratory's policy allows and they confirm it (ADR 0015).
 */
export function CheckDialog({
  connection,
  onClose,
}: {
  connection: PartnerConnection;
  onClose: () => void;
}) {
  return (
    <CommandDialog<CheckValues, RecordPartnerDestinationCheck>
      title="Record destination check"
      submitLabel="Record check"
      command={{ name: RECORD_CHECK, invalidates: [...CONNECTION_QUERIES, ...MESSAGE_QUERIES] }}
      defaultValues={{ method: "" }}
      toInput={(values, selfApproval) => ({
        connectionId: connection.id,
        expectedRevision: connection.revision,
        method: values.method.trim(),
        selfApproval,
      })}
      successTitle="Destination check recorded"
      onClose={onClose}
    >
      <p>
        Folder: <strong>{connection.folder}</strong>
      </p>
      <TextInput
        name="method"
        label="How it was confirmed"
        description="For example, a call-back to the partner's IT, or the partner confirmed the test message"
        isRequired
        autoFocus
      />
      <Alert tone="warning">
        The partner receiving patient data must be covered by a business associate agreement. That
        is the laboratory's responsibility.
      </Alert>
    </CommandDialog>
  );
}
