import { useState } from "react";
import { useNavigate } from "react-router";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { LIST_INSTRUMENT_READINESS, MANAGE_CONFIGURATION, MANAGE_QUALITY } from "../contract";
import { instrumentStatus, sourceText } from "../labels";
import { fileTemplatePath } from "../paths";
import { InstrumentPanel } from "./InstrumentPanel";
import { InstrumentStatusDialog } from "./InstrumentStatusDialog";
import { RegisterSourceDialog } from "./RegisterSourceDialog";

const COLUMNS: readonly DataGridColumn<InstrumentReadinessView>[] = [
  {
    id: "name",
    header: "Instrument",
    identifier: true,
    cell: (row) => row.name,
    sortValue: (row) => row.name,
  },
  {
    id: "status",
    header: "Status",
    cell: (row) => {
      const status = instrumentStatus(row.status);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    width: 200,
  },
  {
    id: "since",
    header: "Since",
    cell: (row) => (row.since === null ? "Not recorded" : <Timestamp value={row.since} />),
    width: 200,
  },
  { id: "source", header: "Approved result source", cell: sourceText },
];

const getRowId = (row: InstrumentReadinessView) => row.instrumentId;

type InstrumentAction = "set_instrument_status" | "register_source" | "new_file_template";

const ACTIONS: readonly OfferedAction<InstrumentAction>[] = [
  { id: "set_instrument_status", label: "Set status" },
  { id: "register_source", label: "Register source" },
  { id: "new_file_template", label: "New file template" },
];

type InstrumentDialog = {
  kind: Exclude<InstrumentAction, "new_file_template">;
  instrument: InstrumentReadinessView;
} | null;

/**
 * Each instrument's service status (REQ-ANA-007), opening its panel with the approved result
 * sources. Readiness carries no `allowedActions` yet, so its actions follow the session's
 * capabilities: quality staff set the status with a reason, configuration staff draft
 * sources. The instrument's transport decides between an analyzer's source and a result-file
 * instrument's file template (result files brief §7).
 */
export function InstrumentsTab() {
  const navigate = useNavigate();
  const readiness = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const record = useOpenRecord();
  const canSetStatus = useHasCapability(MANAGE_QUALITY);
  const canRegister = useHasCapability(MANAGE_CONFIGURATION);
  const [dialog, setDialog] = useState<InstrumentDialog>(null);
  const close = () => setDialog(null);
  // Interim: the session's capabilities stand in for the engine's `allowedActions`.
  const allowedFor = (instrument: InstrumentReadinessView): InstrumentAction[] => {
    const allowed: InstrumentAction[] = [];
    if (canSetStatus) allowed.push("set_instrument_status");
    if (canRegister) {
      allowed.push(
        instrument.transport === "result_file" ? "new_file_template" : "register_source",
      );
    }
    return allowed;
  };
  const act = (kind: InstrumentAction, instrument: InstrumentReadinessView) => {
    if (kind === "new_file_template") void navigate(fileTemplatePath(instrument.instrumentId));
    else setDialog({ kind, instrument });
  };
  const panel = openRecordPanel(record, readiness.data, getRowId, (open) => (
    <InstrumentPanel
      instrument={open}
      actions={allowedActionItems(ACTIONS, allowedFor(open), (kind) => act(kind, open))}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: InstrumentReadinessView) => row.name,
    allowed: allowedFor,
    onAction: act,
  });
  return (
    <>
      <DetailLayout panel={panel}>
        {readiness.data && (
          <Toolbar
            variant="collection"
            count={
              readiness.data && (
                <ListCount shown={readiness.data.length} noun="instruments" one="instrument" />
              )
            }
          />
        )}
        <QueryContent
          query={readiness}
          what="instruments"
          loadingRows={4}
          empty={{
            when: (rows) => rows.length === 0,
            title: "No instruments yet",
            description: "Instruments are registered under Laboratory setup.",
          }}
        >
          {(rows) => (
            <DataGrid
              label="Instruments"
              columns={COLUMNS}
              rows={rows}
              getRowId={getRowId}
              onRowAction={(row, order) => record.open(row.instrumentId, order)}
              openRowId={record.openId}
              onOrderChange={record.followOrder}
              rowActions={rowActions}
              fill
            />
          )}
        </QueryContent>
      </DetailLayout>
      {dialog?.kind === "set_instrument_status" && (
        <InstrumentStatusDialog instrument={dialog.instrument} onClose={close} />
      )}
      {dialog?.kind === "register_source" && (
        <RegisterSourceDialog instrument={dialog.instrument} onClose={close} />
      )}
    </>
  );
}
