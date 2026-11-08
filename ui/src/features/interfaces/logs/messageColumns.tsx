import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { applicationOf, detailText, MESSAGE_TYPES, recordText, transportOf } from "../labels";

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
      width: 130,
    },
    {
      id: "record",
      header: "Record",
      cell: recordText,
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
      cell: (row) =>
        [detailText(row, formatTime), resolvedText(row, formatTime, person)]
          .filter(Boolean)
          .join(" · "),
      wrap: true,
    },
  ];
}
