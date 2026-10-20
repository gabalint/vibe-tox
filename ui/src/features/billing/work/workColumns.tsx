import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BillingWorkItem } from "@/generated/BillingWorkItem";
import { workState } from "../labels";
import { accessionsText } from "./useWorkRowActions";

/**
 * The billing work columns. The accessions identify a row; the patient is never named in the
 * list. `sharedState` shows the state as plain text, when every row is in the same one.
 */
export function workColumns(sharedState: boolean): DataGridColumn<BillingWorkItem>[] {
  return [
    {
      id: "accessions",
      header: "Accessions",
      cell: accessionsText,
      sortValue: accessionsText,
      identifier: true,
    },
    {
      id: "dos",
      header: "Date of service",
      cell: (row) => row.dateOfService ?? "Unknown",
      sortValue: (row) => row.dateOfService ?? "",
      width: 150,
    },
    {
      id: "handoff",
      header: "Handoff",
      cell: (row) => row.handoffNo ?? "None",
      sortValue: (row) => row.handoffNo ?? 0,
      align: "end",
      width: 110,
    },
    {
      id: "state",
      header: "State",
      cell: (row) => {
        const state = workState(row.state);
        if (sharedState) return state.label;
        return <StatusBadge tone={state.tone}>{state.label}</StatusBadge>;
      },
      sortValue: (row) => workState(row.state).label,
      width: 200,
    },
    {
      id: "since",
      header: "Since",
      cell: (row) => <Timestamp value={row.since} />,
      sortValue: (row) => row.since,
      width: 190,
    },
  ];
}
