import { Link } from "react-router";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { connectionText, instrumentConnectionPath } from "@/lib/instrumentPages";
import { instrumentStatus } from "@/lib/instrumentStatus";
import { LIST_INSTRUMENTS, MANAGE_CONFIGURATION } from "../contract";
import { sourceText } from "../labels";
import { InstrumentSources } from "./InstrumentSources";

interface InstrumentPanelProps {
  instrument: InstrumentReadinessView;
  /** The same actions as the instrument's row menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * One instrument's service status and its registered result sources (REQ-ANA-003, -007). For
 * configuration staff, its connection in one line, linked to its settings under Laboratory setup.
 */
export function InstrumentPanel({ instrument, actions, ...navigation }: InstrumentPanelProps) {
  const status = instrumentStatus(instrument.status);
  const canConfigure = useHasCapability(MANAGE_CONFIGURATION);
  return (
    <DetailPanel
      identifier={instrument.name}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          {
            label: "Status since",
            value:
              instrument.since === null ? "Not recorded" : <Timestamp value={instrument.since} />,
          },
          { label: "Approved result source", value: sourceText(instrument) },
          ...(canConfigure
            ? [
                {
                  label: "Connection",
                  value: <ConnectionSummary instrumentId={instrument.instrumentId} />,
                },
                {
                  label: "Laboratory setup",
                  value: (
                    <Link to={instrumentConnectionPath(instrument.instrumentId)}>
                      Connection settings
                    </Link>
                  ),
                },
              ]
            : []),
        ]}
      />
      <InstrumentSources instrument={instrument} />
    </DetailPanel>
  );
}

/** The connection as Laboratory setup › Instruments lists it (`list_instruments`). */
function ConnectionSummary({ instrumentId }: { instrumentId: string }) {
  const instruments = useCommandQuery<InstrumentConfiguration[]>(LIST_INSTRUMENTS);
  if (instruments.isPending) return "Checking";
  if (instruments.isError) return "Connection unavailable";
  const configured = instruments.data.find((candidate) => candidate.id === instrumentId);
  return configured ? connectionText(configured.connection) : "Not configured";
}
