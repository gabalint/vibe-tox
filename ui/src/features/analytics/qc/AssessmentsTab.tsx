import { useCallback, useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { RunQcView } from "@/generated/RunQcView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { AcceptQcDialog } from "./AcceptQcDialog";
import { AssessQcDialog } from "./AssessQcDialog";
import {
  bulkAcceptBlocker,
  currentObservations,
  type QcScope,
  qcScopes,
  qcSummary,
  RUN_SCOPE_KEY,
  scopeStatus,
} from "./qcScopes";
import { ScopePanel } from "./ScopePanel";

/** A grid row: an analyte or a preparation batch, and why it cannot join a bulk acceptance. */
interface ScopeRow extends QcScope {
  blocker: string | null;
}

const COLUMNS: readonly DataGridColumn<ScopeRow>[] = [
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
  {
    id: "bulk",
    header: "Accept with others",
    cell: (row) => row.blocker ?? "Available",
    sortValue: (row) => row.blocker ?? "",
    width: 260,
  },
];

const RUN_NOTE =
  "Optional. Reject to block every analyte on this run. Accepting it doesn’t accept the analytes.";

const getRowId = (scope: QcScope) => scope.key;
const ACTIONS: readonly OfferedAction<"assess_qc">[] = [{ id: "assess_qc", label: "Assess QC" }];
/** Interim: every scope may be assessed, as before; the engine decides each assessment. */
const ALLOWED = ["assess_qc"];

/**
 * Each scope's current QC assessment (REQ-ANA-004, REQ-ANA-005): a summary line, the optional
 * whole run in its own section, then a grid of every analyte and each preparation batch.
 * Analytes that can be accepted are selected together and accepted at once, each recorded
 * separately; rejection stays one scope at a time. A scope opens in the panel (`?open=` holds
 * its key, the analyte ID for an analyte) with its history and Assess QC. The assessments
 * carry no `allowedActions` yet; the engine decides each assessment when it is recorded.
 */
export function AssessmentsTab({ view }: { view: RunQcView }) {
  const scopes = useMemo(() => qcScopes(view), [view]);
  const run = scopes.find((scope) => scope.key === RUN_SCOPE_KEY);
  const rows = useMemo(
    () =>
      scopes
        .filter((scope) => scope.key !== RUN_SCOPE_KEY)
        .map((scope) => ({ ...scope, blocker: bulkAcceptBlocker(scope, view) })),
    [scopes, view],
  );
  const isSelectable = useCallback(
    (id: string) => rows.some((row) => row.key === id && row.blocker === null),
    [rows],
  );
  const record = useOpenRecord();
  const [assessing, setAssessing] = useState<QcScope | null>(null);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const [accepting, setAccepting] = useState<readonly QcScope[] | null>(null);
  const selected = rows.filter((row) => row.blocker === null && selectedIds.has(row.key));
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
        <Toolbar variant="collection" count={qcSummary(scopes)} />
        {run && (
          <Section
            title="Whole run"
            variant="plain"
            actions={
              <>
                <Button onPress={() => record.open(run.key)}>History</Button>
                <Button onPress={() => setAssessing(run)}>Assess QC</Button>
              </>
            }
          >
            <p>
              <StatusBadge tone={scopeStatus(run).tone}>{scopeStatus(run).label}</StatusBadge>{" "}
              {RUN_NOTE}
            </p>
          </Section>
        )}
        <DataGrid
          label="QC assessments"
          columns={COLUMNS}
          rows={rows}
          getRowId={getRowId}
          onRowAction={(scope, order) => record.open(scope.key, order)}
          openRowId={record.openId}
          onOrderChange={record.followOrder}
          rowActions={rowActions}
          bulk={{
            selectedIds,
            onChange: setSelectedIds,
            isSelectable,
            actions: (
              <Button variant="primary" onPress={() => setAccepting(selected)}>
                Accept QC
              </Button>
            ),
          }}
          fill
        />
      </DetailLayout>
      {accepting && (
        <AcceptQcDialog
          scopes={accepting}
          onDone={() => setSelectedIds(new Set())}
          onClose={() => setAccepting(null)}
        />
      )}
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
