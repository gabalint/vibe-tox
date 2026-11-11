import { Alert } from "@/components/Alert";
import { TextInput } from "@/components/FormFields";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerProtocol } from "@/generated/PartnerProtocol";
import type { RecordPartnerDestinationCheck } from "@/generated/RecordPartnerDestinationCheck";
import { CommandDialog } from "@/lib/CommandDialog";
import { CONNECTION_QUERIES, MESSAGE_QUERIES, RECORD_CHECK } from "../contract";
import { destinationText } from "../labels";
import { partnersText } from "./listenerForm";

interface CheckValues {
  method: string;
}

/** What the check covers, by protocol; SFTP names its user and fingerprint too. */
const DESTINATION_LABELS: Record<PartnerProtocol, string> = {
  folder: "Folder",
  sftp: "SFTP server",
  mllp_connect: "MLLP partner",
  mllp_listen: "MLLP listener",
};

/**
 * Records how the connection's current folder, SFTP server with its user, folder and
 * fingerprint, MLLP partner, or MLLP listener with its allowed partners, was confirmed as the
 * partner's (REQ-RPT-012), before any patient data is written there or received. A second person records it, or the person who chose the destination where
 * the laboratory's policy allows and they confirm it (ADR 0015).
 */
export function CheckDialog({
  connection,
  onClose,
}: {
  connection: PartnerConnection;
  onClose: () => void;
}) {
  const { sftp, mllpListener } = connection.settings;
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
      {sftp === null ? (
        <p>
          {DESTINATION_LABELS[connection.settings.protocol]}:{" "}
          <strong>{destinationText(connection)}</strong>
          {mllpListener && (
            <>
              , from <strong>{partnersText(mllpListener.allowedPartners)}</strong>
            </>
          )}
        </p>
      ) : (
        <p>
          SFTP server: <strong>{destinationText(connection)}</strong> as{" "}
          <strong>{sftp.username}</strong>, fingerprint <strong>{sftp.fingerprint}</strong>
        </p>
      )}
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
