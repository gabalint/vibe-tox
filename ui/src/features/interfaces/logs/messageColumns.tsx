import { Link } from "react-router";
import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { WaitReason } from "@/generated/WaitReason";
import { applicationOf, detailText, MESSAGE_TYPES, recordText, transportOf } from "../labels";
import { contractPath, handoffPath, reportPath, revisionPath } from "../paths";

type FormatTime = (at: string) => string;
type PersonText = (userId: string, display?: string) => string;

/** How a person followed up a rejection: rejected a waiting message, or resolved it. */
function resolvedText(row: PartnerMessageRow, formatTime: FormatTime, person: PersonText) {
  const done = row.rejectionResolved;
  if (done === null) return null;
  const verb = row.errorCode === "rejected_by_person" ? "Rejected" : "Resolved";
  const who = person(done.resolved.by, done.resolved.byDisplay);
  return `${verb} by ${who}, ${formatTime(done.resolved.at)}: ${done.note}`;
}

/** Holds the connection's interface contract and its acceptance tests clear (#4726). */
const CONTRACT_HOLDS: readonly (WaitReason | null)[] = [
  "contract_not_approved",
  "tests_not_run",
  "tests_failed",
  "tests_out_of_date",
];

/**
 * What became of the row and how a person followed it up; a message held by its connection's
 * contract or acceptance tests links to the contract, where the tests run (#4726). Also on the
 * message panel, which pushes the Details column out of view.
 */
export function MessageDetails({
  row,
  formatTime,
  person,
}: {
  row: PartnerMessageRow;
  formatTime: FormatTime;
  person: PersonText;
}) {
  const text = [detailText(row, formatTime), resolvedText(row, formatTime, person)]
    .filter(Boolean)
    .join(" · ");
  if (row.connection === null || !CONTRACT_HOLDS.includes(row.waiting)) return text;
  return (
    <>
      {text} · <Link to={contractPath(row.connection.id)}>Open interface contract</Link>
    </>
  );
}

/** The page of the record a message was made from; none for a test message or an order. */
function recordPath({ source, sourceId }: PartnerMessageRow): string | null {
  if (sourceId === null) return null;
  switch (source) {
    case "report_revision":
      return revisionPath(sourceId);
    case "report_withdrawal":
      return reportPath(sourceId);
    case "billing_handoff":
      return handoffPath(sourceId);
    default:
      return null;
  }
}

/**
 * The record, linked to the report revision sent, the withdrawn report or the handoff; a
 * synthetic test message shows as a Test badge, so it stands apart from real messages.
 */
function Record({ row }: { row: PartnerMessageRow }) {
  if (row.source === "test") return <StatusBadge tone="info">Test</StatusBadge>;
  const to = recordPath(row);
  return to === null ? recordText(row) : <Link to={to}>{recordText(row)}</Link>;
}

/** One row of the message log: never a bare delivery claim (REQ-RPT-011). */
export function messageColumns(
  formatTime: FormatTime,
  person: PersonText,
): readonly DataGridColumn<PartnerMessageRow>[] {
  return [
    {
      id: "at",
      header: "Time",
      cell: (row) => <Timestamp value={row.at} />,
      sortValue: (row) => row.at,
      defaultSort: "desc",
      width: 190,
    },
    {
      id: "direction",
      header: "Dir.",
      cell: (row) => (row.received === null ? "Out" : "In"),
      width: 70,
    },
    {
      id: "connection",
      header: "Connection",
      cell: (row) => row.connection?.name ?? "None matched",
      sortValue: (row) => row.connection?.name ?? "",
    },
    {
      id: "message",
      header: "Message",
      cell: (row) => MESSAGE_TYPES[row.messageType],
      // Fits the longest type, HL7 Results (ORU^R01).
      width: 190,
    },
    {
      id: "record",
      header: "Record",
      cell: (row) => <Record row={row} />,
      sortValue: (row) => row.sourceNumber ?? 0,
      width: 170,
    },
    {
      id: "transport",
      header: "Transport",
      cell: (row) => {
        const transport = transportOf(row);
        return <StatusBadge tone={transport.tone}>{transport.label}</StatusBadge>;
      },
      sortValue: (row) => transportOf(row).label,
      width: 120,
    },
    {
      id: "application",
      header: "Application",
      cell: (row) => {
        const application = applicationOf(row);
        return application ? (
          <StatusBadge tone={application.tone}>{application.label}</StatusBadge>
        ) : (
          "—"
        );
      },
      // Fits the longest outcome, Rejected – unknown test.
      width: 200,
    },
    {
      id: "details",
      header: "Details",
      cell: (row) => <MessageDetails row={row} formatTime={formatTime} person={person} />,
      wrap: true,
    },
  ];
}
