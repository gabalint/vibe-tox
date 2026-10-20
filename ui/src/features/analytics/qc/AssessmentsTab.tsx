import { useMemo, useState } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { ListCount } from "@/components/ListLimit";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { RunQcView } from "@/generated/RunQcView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { AssessQcDialog } from "./AssessQcDialog";
import { currentObservations, type QcScope, qcScopes, scopeStatus } from "./qcScopes";
import { ScopePanel } from "./ScopePanel";

const COLUMNS: readonly DataGridColumn<QcScope>[] = [
  {
    id: "scope",
    header: "Assessed for",
    identifier: true,
    cell: (scope) => scope.label,
    sortValue: (scope) => scope.label,
  },
  {
    id: "observations",
    header: "Current observations",
    cell: (scope) => currentObservations(scope).length,
    sortValue: (scope) => currentObservations(scope).length,
    align: "end",
    width: 190,
  },
  {
    id: "assessment",
    header: "Assessment",
    cell: (scope) => {
      const status = scopeStatus(scope);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    width: 150,
  },
  {
    id: "assessed",
    header: "Assessed",
    cell: ({ assessments: { current } }) => current && <Timestamp value={current.assessedAt} />,
    width: 180,
  },
  {
    id: "earlier",
    header: "Replaced",
    cell: ({ assessments }) => assessments.history.length - (assessments.current ? 1 : 0),
    align: "end",
    width: 100,
  },
];

const getRowId = (scope: QcScope) => scope.key;
const ACTIONS: readonly OfferedAction<"assess_qc">[] = [{ id: "assess_qc", label: "Assess QC" }];
/** Interim: every scope may be assessed, as before; the engine decides each assessment. */
const ALLOWED = ["assess_qc"];

/**
 * Each scope's current QC assessment in one grid (REQ-ANA-004, REQ-ANA-005): every analyte,
 * the whole run and each preparation batch. A scope opens in the panel (`?open=` holds its
 * key, the analyte ID for an analyte) with its history and Assess QC. The assessments carry
 * no `allowedActions` yet; the engine decides each assessment when it is recorded.
 */
export function AssessmentsTab({ view }: { view: RunQcView }) {
  const scopes = useMemo(() => qcScopes(view), [view]);
  const record = useOpenRecord();
  const [assessing, setAssessing] = useState<QcScope | null>(null);
  const panel = openRecordPanel(record, scopes, getRowId, (open) => (
    <ScopePanel
      scope={open}
      actions={allowedActionItems(ACTIONS, ALLOWED, () => setAssessing(open))}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (scope: QcScope) => scope.label,
    allowed: () => ALLOWED,
    onAction: (_, scope) => setAssessing(scope),
  });
  return (
    <>
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            <ListCount shown={scopes.length} noun="assessment scopes" one="assessment scope" />
          }
        />
        <DataGrid
          label="QC assessments"
          columns={COLUMNS}
          rows={scopes}
          getRowId={getRowId}
          onRowAction={(scope, order) => record.open(scope.key, order)}
          openRowId={record.openId}
          onOrderChange={record.followOrder}
          rowActions={rowActions}
          fill
        />
      </DetailLayout>
      {assessing && (
        <AssessQcDialog
          title={assessing.title}
          target={assessing.target}
          evidence={assessing.target.scope === "run_analyte" ? currentObservations(assessing) : []}
          current={assessing.assessments.current}
          onClose={() => setAssessing(null)}
        />
      )}
    </>
  );
}
