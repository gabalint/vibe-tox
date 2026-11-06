import type { DataGridColumn } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import { ImportOutcomeBadge } from "./ImportOutcomeBadge";

/** How the attempt's data arrived: a result file, by hand or from the folder, or a message. */
function originText(row: ImportAttemptSummary): string {
  if (row.origin === "upload") return "File loaded by hand";
  if (row.origin === "folder") return "File from the watched folder";
  return "Analyzer message";
}

/** The processing time identifies the attempt; a click or Enter on the row opens it. */
export function attemptColumns(
  instrumentName: (id: string) => string,
): DataGridColumn<ImportAttemptSummary>[] {
  return [
    {
      id: "processed",
      header: "Processed",
      identifier: true,
      cell: (row) => <Timestamp value={row.processedAt} />,
      sortValue: (row) => row.processedAt,
      width: 200,
    },
    {
      id: "received",
      header: "Received",
      cell: (row) => <Timestamp value={row.receivedAt} />,
      sortValue: (row) => row.receivedAt,
      width: 200,
    },
    {
      id: "instrument",
      header: "Instrument",
      cell: (row) => instrumentName(row.instrumentId),
      sortValue: (row) => instrumentName(row.instrumentId),
    },
    { id: "origin", header: "Received as", cell: originText, width: 220 },
    {
      id: "attempt",
      header: "Attempt",
      cell: (row) => row.attemptNo,
      align: "end",
      width: 100,
    },
    {
      id: "outcome",
      header: "Outcome",
      cell: (row) => <ImportOutcomeBadge outcome={row.outcome} />,
      width: 260,
    },
    { id: "rows", header: "Rows", cell: (row) => row.rowsTotal, align: "end", width: 90 },
    {
      id: "quarantined",
      header: "Quarantined",
      cell: (row) => row.rowsQuarantined,
      sortValue: (row) => row.rowsQuarantined,
      align: "end",
      width: 130,
    },
  ];
}
