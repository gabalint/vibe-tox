import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { Tone } from "@/components/tones";
import type { AuditEventView } from "@/generated/AuditEventView";
import styles from "./audit.module.css";

const OUTCOMES: Record<AuditEventView["outcome"], { tone: Tone; label: string }> = {
  succeeded: { tone: "neutral", label: "Succeeded" },
  denied: { tone: "warning", label: "Denied" },
  failed: { tone: "danger", label: "Failed" },
};

const PRINCIPAL_KINDS: Record<AuditEventView["principalKind"], string> = {
  user: "User",
  instrument: "Instrument",
  partner: "Partner",
  system: "System",
};

/** Shown for a field the event does not have. */
export const NONE = "—";

/** Who acted: the engine's display name with the kind of principal, or the kind and ID. */
export function principalText(event: AuditEventView): string {
  const kind = PRINCIPAL_KINDS[event.principalKind];
  return event.principalDisplay
    ? `${event.principalDisplay} (${kind.toLowerCase()})`
    : `${kind} ${event.principalId}`;
}

/** What was done, in the engine's words; an action without them shows as recorded. */
export function actionText(event: AuditEventView): string {
  return event.actionLabel ?? event.action;
}

/**
 * The record the event is about: its type and its human identifier, in the engine's words
 * where it has them, otherwise as recorded.
 */
export function recordText(event: AuditEventView): string {
  if (event.recordType === null) return NONE;
  const type = event.recordTypeLabel ?? event.recordType;
  const id = event.recordLabel ?? event.recordId;
  return id === null ? type : `${type} ${id}`;
}

export function OutcomeBadge({ outcome }: { outcome: AuditEventView["outcome"] }) {
  return <StatusBadge tone={OUTCOMES[outcome].tone}>{OUTCOMES[outcome].label}</StatusBadge>;
}

export const AUDIT_COLUMNS: readonly DataGridColumn<AuditEventView>[] = [
  {
    id: "seq",
    header: "Seq.",
    cell: (event) => event.seq,
    sortValue: (event) => event.seq,
    align: "end",
    width: 90,
    identifier: true,
  },
  {
    id: "occurredAt",
    header: "Occurred",
    cell: (event) => <Timestamp value={event.occurredAt} seconds />,
    sortValue: (event) => event.occurredAt,
    width: 200,
  },
  { id: "principal", header: "By", cell: principalText, sortValue: principalText },
  {
    id: "action",
    header: "Action",
    cell: actionText,
    sortValue: actionText,
  },
  {
    id: "outcome",
    header: "Outcome",
    cell: (event) => <OutcomeBadge outcome={event.outcome} />,
    sortValue: (event) => event.outcome,
    width: 130,
  },
  { id: "record", header: "Record", cell: recordText, sortValue: recordText },
  {
    id: "reason",
    header: "Reason",
    // Clamped to two lines, so one long reason does not make one row fill the grid.
    cell: (event) =>
      event.reason === null ? (
        NONE
      ) : (
        <span className={styles.reason} title={event.reason}>
          {event.reason}
        </span>
      ),
    wrap: true,
  },
];
