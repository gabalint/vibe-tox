import type { DataGridColumn } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { RestoreTestView } from "@/generated/RestoreTestView";
import { LIST_RESTORE_TESTS } from "../contract";
import { JobHistory } from "./JobHistory";
import { elapsedText, RestoreTestPanel, TestState } from "./RestoreTestPanel";
import { useRestoreTests } from "./useBackupLists";

const COLUMNS: readonly DataGridColumn<RestoreTestView>[] = [
  {
    id: "requested",
    header: "Requested",
    identifier: true,
    cell: (row) => <Timestamp value={row.requestedAt} />,
    sortValue: (row) => row.requestedAt,
    width: 190,
  },
  { id: "target", header: "Target", cell: (row) => row.targetLabel },
  {
    id: "set",
    header: "Backup made",
    cell: (row) => (row.setCreatedAt ? <Timestamp value={row.setCreatedAt} /> : "Unknown"),
    width: 190,
  },
  {
    id: "state",
    header: "Result",
    cell: (row) => <TestState test={row} />,
    width: 140,
  },
  {
    id: "elapsed",
    header: "Restore time",
    cell: (row) => elapsedText(row.elapsedMs),
    sortValue: (row) => row.elapsedMs ?? -1,
    align: "end",
    width: 130,
  },
  {
    id: "findings",
    header: "Findings",
    cell: (row) => row.findings.length,
    sortValue: (row) => row.findings.length,
    align: "end",
    width: 100,
  },
];

const getRowId = (row: RestoreTestView) => row.testId;

/** Retained restore-test results and exact older test links. */
export function RestoreTestsSection() {
  const history = useRestoreTests();
  return (
    <JobHistory
      {...history}
      command={LIST_RESTORE_TESTS}
      idField="testId"
      getRowId={getRowId}
      columns={COLUMNS}
      noun="restore tests"
      singular="restore test"
      label="Restore tests"
      emptyDescription="A backup is proven only by reading it back. Open a target on the Targets tab and request a restore test."
      renderPanel={(test, nav) => (
        <RestoreTestPanel
          test={test}
          onClose={nav.close}
          onPrevious={nav.previous}
          onNext={nav.next}
        />
      )}
    />
  );
}
