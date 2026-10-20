import { useState } from "react";
import type { DataGridColumn } from "@/components/DataGrid";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { codedColumns } from "../catalogColumns";
import { LIST_INSTRUMENTS } from "../contract";
import { byId, Directory } from "../Directory";
import { connectionText } from "./connectionValues";
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
 */
export function InstrumentsPage() {
  const query = useCommandQuery<InstrumentConfiguration[]>(LIST_INSTRUMENTS);
  const record = useOpenRecord();
  const [lastChange, setLastChange] = useState<InstrumentChangeResult | null>(null);
  const { actionsOf, rowActions, dialog } = useInstrumentActions(query.data ?? NONE, setLastChange);
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
        panel={openRecordPanel(record, query.data, byId, (open) => (
          <InstrumentPanel
            instrument={open}
            actions={actionsOf(open)}
            lastChange={lastChange?.instrument.id === open.id ? lastChange : null}
            record={record}
          />
        ))}
        add={{
          label: "Register analyzer",
          emptyText: "Register an analyzer to receive its results.",
          dialog: (onClose) => (
            <RegisterDialog
              onSaved={(result) => {
                setLastChange(result);
                record.open(result.instrument.id);
              }}
              onClose={onClose}
            />
          ),
        }}
      />
      {dialog}
    </>
  );
}
