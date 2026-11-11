import { TextInput } from "@/components/FormFields";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { RejectReceivedMessage } from "@/generated/RejectReceivedMessage";
import type { ResolvePartnerRejection } from "@/generated/ResolvePartnerRejection";
import { CommandDialog } from "@/lib/CommandDialog";
import { MESSAGE_QUERIES, REJECT_RECEIVED, RESOLVE_REJECTION } from "../contract";
import { recordText } from "../labels";

/** A row's follow-up: a person rejects a waiting message, or resolves a rejection. */
export interface FollowUp {
  action: typeof REJECT_RECEIVED | typeof RESOLVE_REJECTION;
  row: PartnerMessageRow;
}

/**
 * Rejects a received order message that waits for its patient or is held for review: no order
 * is made from it, and its reason resolves it. The clinic is not told; over MLLP its EMR got AA
 * when the message arrived.
 */
function RejectDialog({ row, onClose }: { row: PartnerMessageRow; onClose: () => void }) {
  return (
    <CommandDialog<{ reason: string }, RejectReceivedMessage>
      title={`Reject ${recordText(row)}`}
      submitLabel="Reject message"
      command={{ name: REJECT_RECEIVED, invalidates: MESSAGE_QUERIES }}
      defaultValues={{ reason: "" }}
      toInput={({ reason }) => ({ receivedId: row.messageId, reason: reason.trim() })}
      successTitle="Message rejected; no order is made from it"
      errorTitle="Not rejected"
      onClose={onClose}
    >
      <p>
        No order is made from it. The clinic is not told: tell its staff yourself if they need to
        send it again.
      </p>
      <TextInput
        name="reason"
        label="Reason"
        description="For example, the clinic cancelled the order by phone"
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}

/**
 * Resolves a rejection, the LIS's or the partner's (AE, AR), with what was done about it. It
 * leaves the Rejected tab and stays under All with the note.
 */
function ResolveDialog({ row, onClose }: { row: PartnerMessageRow; onClose: () => void }) {
  const received = row.received !== null;
  return (
    <CommandDialog<{ note: string }, ResolvePartnerRejection>
      title={`Resolve the rejection of ${recordText(row)}`}
      submitLabel="Mark resolved"
      command={{ name: RESOLVE_REJECTION, invalidates: MESSAGE_QUERIES }}
      defaultValues={{ note: "" }}
      toInput={({ note }) => ({
        ...(received ? { receivedId: row.messageId } : { deliveryId: row.deliveryId ?? "" }),
        note: note.trim(),
      })}
      successTitle="Rejection resolved"
      onClose={onClose}
    >
      <TextInput
        name="note"
        label="What was done"
        description={
          received
            ? "For example, the clinic corrected the test code and sent the order again"
            : "For example, the partner registered the provider; confirmed by phone"
        }
        isRequired
        autoFocus
      />
    </CommandDialog>
  );
}

/** The open follow-up's dialog. */
export function FollowUpDialog({ followUp, onClose }: { followUp: FollowUp; onClose: () => void }) {
  return followUp.action === REJECT_RECEIVED ? (
    <RejectDialog row={followUp.row} onClose={onClose} />
  ) : (
    <ResolveDialog row={followUp.row} onClose={onClose} />
  );
}
