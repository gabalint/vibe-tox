import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { Timestamp } from "@/components/Timestamp";
import type { AuditEventView } from "@/generated/AuditEventView";
import { actionText, NONE, OutcomeBadge, principalText, recordText } from "./auditColumns";
import { streamLabel } from "./streams";

interface AuditEventDetailsProps {
  event: AuditEventView;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * Every field of one event, in full, to reconstruct who did what, when and why (REQ-OPS-001),
 * in the side panel. The event comes from the list the engine returned for this view, which it
 * recorded as an access; opening one reads nothing more.
 */
export function AuditEventDetails({ event, ...navigation }: AuditEventDetailsProps) {
  return (
    <DetailPanel
      identifier={`${streamLabel(event.stream)} event ${event.seq}`}
      status={<OutcomeBadge outcome={event.outcome} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Occurred", value: <Timestamp value={event.occurredAt} seconds /> },
          { label: "Recorded", value: <Timestamp value={event.recordedAt} seconds /> },
          { label: "By", value: principalText(event) },
          { label: "Principal ID", value: event.principalId },
          { label: "Action", value: withCode(actionText(event), event.action) },
          { label: "Record", value: recordText(event) },
          { label: "Record ID", value: event.recordId ?? NONE },
          { label: "Reason", value: event.reason ?? NONE },
          { label: "Authority", value: event.authority ?? NONE },
          { label: "Correlation ID", value: event.correlationId },
        ]}
      />
    </DetailPanel>
  );
}

/** The label with the recorded code, as support and exported audit files name it. */
function withCode(text: string, code: string): string {
  return text === code ? code : `${text} (${code})`;
}
