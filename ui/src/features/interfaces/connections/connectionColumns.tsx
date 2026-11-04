import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import styles from "@/styles/layout.module.css";
import { destinationText, MESSAGE_TYPES, PROTOCOLS, STATUS } from "../labels";

/** Verified or not: whether a destination check covers the connection's current destination. */
export function CheckBadge({ connection }: { connection: PartnerConnection }) {
  return connection.verification ? (
    <StatusBadge tone="success">Verified</StatusBadge>
  ) : (
    <StatusBadge tone="warning">Not verified</StatusBadge>
  );
}

export function StatusOf({ connection }: { connection: PartnerConnection }) {
  const status = STATUS[connection.status];
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}

export const CONNECTION_COLUMNS: readonly DataGridColumn<PartnerConnection>[] = [
  {
    id: "name",
    header: "Name",
    cell: (row) => row.settings.name,
    sortValue: (row) => row.settings.name,
    identifier: true,
  },
  {
    id: "partner",
    header: "Partner",
    cell: (row) => row.settings.partner,
    sortValue: (row) => row.settings.partner,
  },
  {
    id: "message",
    header: "Message",
    cell: (row) => MESSAGE_TYPES[row.settings.messageType],
    sortValue: (row) => MESSAGE_TYPES[row.settings.messageType],
    width: 130,
  },
  // Every message type of this release goes out of the LIS.
  { id: "direction", header: "Dir.", cell: () => "Out", width: 70 },
  {
    id: "protocol",
    header: "Protocol",
    cell: (row) => PROTOCOLS[row.settings.protocol],
    width: 100,
  },
  {
    id: "destination",
    header: "Destination",
    cell: (row) => <span className={styles.code}>{destinationText(row)}</span>,
    sortValue: (row) => destinationText(row),
    wrap: true,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => <StatusOf connection={row} />,
    sortValue: (row) => STATUS[row.status].label,
    width: 110,
  },
  {
    id: "check",
    header: "Destination check",
    cell: (row) => <CheckBadge connection={row} />,
    sortValue: (row) => (row.verification ? 1 : 0),
    width: 150,
  },
  {
    id: "last",
    header: "Last attempt",
    cell: (row) => (row.lastAttempt ? <Timestamp value={row.lastAttempt.at} /> : "None yet"),
    sortValue: (row) => row.lastAttempt?.at,
    width: 180,
  },
  {
    id: "waiting",
    header: "Waiting",
    cell: (row) => row.waiting,
    sortValue: (row) => row.waiting,
    align: "end",
    width: 90,
  },
  {
    id: "failing",
    header: "Errors",
    cell: (row) => row.failing,
    sortValue: (row) => row.failing,
    align: "end",
    width: 80,
  },
];
