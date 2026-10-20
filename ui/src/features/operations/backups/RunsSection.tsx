import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BackupRunView } from "@/generated/BackupRunView";
import { LIST_BACKUP_RUNS } from "../contract";
import { backupKind, jobState, sizeText } from "../labels";
import { JobHistory } from "./JobHistory";
import { copyText, RunPanel } from "./RunPanel";
import { useBackupRuns } from "./useBackupLists";

const COLUMNS: readonly DataGridColumn<BackupRunView>[] = [
  {
    id: "requested",
    header: "Requested",
    identifier: true,
    cell: (row) => <Timestamp value={row.requestedAt} />,
    sortValue: (row) => row.requestedAt,
    width: 190,
  },
  { id: "kind", header: "Kind", cell: (row) => backupKind(row.kind), width: 150 },
  {
    id: "state",
    header: "State",
    cell: (row) => {
      const state = jobState(row.state);
      return <StatusBadge tone={state.tone}>{state.label}</StatusBadge>;
    },
    width: 140,
  },
  {
    id: "copies",
    header: "Copies",
    cell: (row) => (row.copies.length === 0 ? "None" : row.copies.map(copyText).join("; ")),
  },
  {
    id: "size",
    header: "Database",
    cell: (row) => sizeText(row.databaseBytes),
    align: "end",
    width: 110,
  },
  {
    id: "problems",
    header: "Stored-file problems",
    cell: (row) => row.artifactProblems,
    align: "end",
    width: 170,
  },
  { id: "failure", header: "Failure", cell: (row) => row.failureCode ?? "None", width: 170 },
];

const getRowId = (row: BackupRunView) => row.runId;

/** Retained backup request-time history, including equal timestamps and exact old links. */
export function RunsSection() {
  const history = useBackupRuns();
  return (
    <JobHistory
      {...history}
      command={LIST_BACKUP_RUNS}
      idField="runId"
      getRowId={getRowId}
      columns={COLUMNS}
      noun="backup runs"
      singular="backup run"
      label="Backup runs"
      emptyDescription="Runs appear here once a target is added and the first backup is due."
      renderPanel={(run, nav) => (
        <RunPanel run={run} onClose={nav.close} onPrevious={nav.previous} onNext={nav.next} />
      )}
    />
  );
}
