import { useState } from "react";
import { Button } from "@/components/Button";
import type { DataGridColumn } from "@/components/DataGrid";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import { useCommandQuery } from "@/lib/commands";
import { connectionText } from "@/lib/instrumentPages";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { codedColumns } from "../catalogColumns";
import { LIST_INSTRUMENTS } from "../contract";
import { byId, Directory } from "../Directory";
import {
  BulkEnableDialog,
  type BulkOutcome,
  BulkOutcomeAlert,
  bulkTargets,
} from "./BulkEnableDialog";
import { useInstrumentActions } from "./InstrumentActions";
import { RegisterDialog } from "./InstrumentDialogs";
import { InstrumentPanel, Listening } from "./InstrumentPanel";

const COLUMNS: readonly DataGridColumn<InstrumentConfiguration>[] = [
  ...codedColumns<InstrumentConfiguration>(),
  { id: "connection", header: "Connection", cell: (row) => connectionText(row.connection) },
  {
    id: "enabled",
    header: "Listening",
    cell: (row) => <Listening instrument={row} />,
    sortValue: (row) => (row.enabled ? 1 : 0),
    width: 140,
  },
];

const FIND_FIELDS = (row: InstrumentConfiguration) => [row.code, row.name];
const NONE: readonly InstrumentConfiguration[] = [];

/**
 * Analyzer connections (REQ-INT-005), for people with `manage_configuration`. A network
 * analyzer needs the list of addresses allowed to connect. Each change restarts the
 * listeners, and the engine's answer says whether they came back; the panel shows it.
 * Enable all and Disable all change every analyzer not already in that state, one by one.
 */
export function InstrumentsPage() {
  const query = useCommandQuery<InstrumentConfiguration[]>(LIST_INSTRUMENTS);
  const record = useOpenRecord();
  const [lastChange, setLastChange] = useState<InstrumentChangeResult | null>(null);
  // The last change was the analyzer's registration, so its confirmation offers the next steps.
  const [registered, setRegistered] = useState(false);
  const changed = (result: InstrumentChangeResult, isRegistration = false) => {
    setLastChange(result);
    setRegistered(isRegistration);
  };
  const { actionsOf, rowActions, dialog } = useInstrumentActions(query.data ?? NONE, changed);
  // The analyzers as listed when Enable all or Disable all was pressed, kept while it runs.
  const [bulk, setBulk] = useState<{
    enabling: boolean;
    targets: readonly InstrumentConfiguration[];
  } | null>(null);
  const [outcome, setOutcome] = useState<BulkOutcome | null>(null);
  const toEnable = bulkTargets(query.data ?? NONE, true);
  const toDisable = bulkTargets(query.data ?? NONE, false);
  return (
    <>
      <Directory
        title="Instruments"
        description="Analyzer connections. Results arrive only from enabled, registered analyzers."
        noun={{ many: "analyzers", one: "analyzer" }}
        query={query}
        columns={COLUMNS}
        getRowId={byId}
        findFields={FIND_FIELDS}
        findPlaceholder="Code or name"
        onRowAction={(row, order) => record.open(row.id, order)}
        rowActions={rowActions}
        openRowId={record.openId}
        onOrderChange={record.followOrder}
        // Looked up in the latest list, so the panel always shows the current revision.
        actions={
          <>
            {toEnable.length > 0 && (
              <Button onPress={() => setBulk({ enabling: true, targets: toEnable })}>
                Enable all
              </Button>
            )}
            {toDisable.length > 0 && (
              <Button onPress={() => setBulk({ enabling: false, targets: toDisable })}>
                Disable all
              </Button>
            )}
          </>
        }
        notice={
          outcome && <BulkOutcomeAlert outcome={outcome} onDismiss={() => setOutcome(null)} />
        }
        panel={openRecordPanel(record, query.data, byId, (open) => (
          <InstrumentPanel
            instrument={open}
            actions={actionsOf(open)}
            lastChange={lastChange?.instrument.id === open.id ? lastChange : null}
            registered={registered && lastChange?.instrument.id === open.id}
            record={record}
          />
        ))}
        add={{
          label: "Register analyzer",
          emptyText: "Register an analyzer to receive its results.",
          dialog: (onClose) => (
            <RegisterDialog
              onSaved={(result) => {
                changed(result, true);
                record.open(result.instrument.id);
              }}
              onClose={onClose}
            />
          ),
        }}
      />
      {dialog}
      {bulk && (
        <BulkEnableDialog
          targets={bulk.targets}
          enabling={bulk.enabling}
          onDone={setOutcome}
          onClose={() => setBulk(null)}
        />
      )}
    </>
  );
}
