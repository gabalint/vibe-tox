import { Alert } from "@/components/Alert";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { OpenRecord } from "@/lib/openRecord";
import { connectionText } from "./connectionValues";
import { folderFacts } from "./FolderStatus";

/** Whether the app listens for the analyzer: a routine state either way. */
export function Listening({ instrument }: { instrument: InstrumentConfiguration }) {
  return <StatusBadge tone="neutral">{instrument.enabled ? "Enabled" : "Disabled"}</StatusBadge>;
}

interface InstrumentPanelProps {
  instrument: InstrumentConfiguration;
  /** Edit connection, the watched folder of a result-file instrument, then Enable or Disable. */
  actions: readonly ActionItem[];
  /** The outcome of the last change made to this analyzer here, if any. */
  lastChange: InstrumentChangeResult | null;
  record: OpenRecord;
}

/**
 * One analyzer in view mode: its connection, whether it listens (or, for result files, its
 * watched folder and whether files are picked up), and the last change's outcome.
 */
export function InstrumentPanel({ instrument, actions, lastChange, record }: InstrumentPanelProps) {
  return (
    <DetailPanel
      identifier={instrument.code}
      status={<Listening instrument={instrument} />}
      actions={<RecordActions items={actions} />}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    >
      {lastChange && (
        <Alert
          tone={lastChange.listenersRestarted ? "success" : "warning"}
          title={`${lastChange.instrument.name} saved`}
        >
          {lastChange.message}
        </Alert>
      )}
      <Facts
        items={[
          { label: "Name", value: instrument.name },
          { label: "Connection", value: connectionText(instrument.connection) },
          ...(instrument.connection.kind === "result_file" ? folderFacts(instrument) : []),
        ]}
      />
    </DetailPanel>
  );
}
