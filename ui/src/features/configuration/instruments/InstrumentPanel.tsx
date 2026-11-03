import type { ReactNode } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { ChangeStamp } from "@/generated/ChangeStamp";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { benchInstrumentPath, benchStepState } from "@/lib/instrumentPages";
import { instrumentStatus } from "@/lib/instrumentStatus";
import type { OpenRecord } from "@/lib/openRecord";
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import { CONNECTION_KINDS, pickupName } from "./connectionValues";
import { Activity, LiveStatus, ResultSource } from "./LiveGroups";
import { PanelGroup } from "./PanelGroup";
import { auFacts, connectionFacts } from "./settingsFacts";

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
  /** The analyzer was just registered: its confirmation offers the next steps in Bench setup. */
  registered: boolean;
  record: OpenRecord;
}

/**
 * One analyzer in view mode, read-only: its settings in the Edit connection form's groups and
 * order, then its live listener state, result source, recent imports and who registered and
 * last changed it, so nobody opens the form just to read a value. The last change's outcome
 * comes first.
 */
export function InstrumentPanel({
  instrument,
  actions,
  lastChange,
  registered,
  record,
}: InstrumentPanelProps) {
  const { connection } = instrument;
  const pickup = pickupName(connection.kind);
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
          {registered && <NextSteps instrument={instrument} />}
        </Alert>
      )}
      <PanelGroup title="General">
        <Facts
          items={[
            { label: "Code", value: instrument.code },
            { label: "Name", value: instrument.name },
            { label: "Connection type", value: labelOf(CONNECTION_KINDS, connection.kind) },
            {
              label: pickup.charAt(0).toUpperCase() + pickup.slice(1),
              value: instrument.enabled ? "Enabled" : "Disabled",
            },
            {
              label: "Service status",
              value: (
                <Readiness instrumentId={instrument.id}>
                  {(view) => {
                    const { tone, label } = instrumentStatus(view?.status ?? null);
                    return <StatusBadge tone={tone}>{label}</StatusBadge>;
                  }}
                </Readiness>
              ),
            },
            {
              label: "Result source",
              value: (
                <Readiness instrumentId={instrument.id}>
                  {(view) =>
                    view?.activeRegistration
                      ? `Source approved: ${view.activeRegistration.sender} ${view.activeRegistration.version}`
                      : "No approved source"
                  }
                </Readiness>
              ),
            },
            {
              label: "Bench setup",
              value: <Link to={benchInstrumentPath(instrument.id)}>Readiness and sources</Link>,
            },
          ]}
        />
      </PanelGroup>
      <PanelGroup title="Connection">
        <Facts
          items={
            connection.kind === "result_file"
              ? [{ label: "Watched folder", value: <WatchedFolder instrument={instrument} /> }]
              : connectionFacts(connection)
          }
        />
      </PanelGroup>
      {(connection.kind === "au_tcp" || connection.kind === "au_serial") && (
        <PanelGroup title="AU settings">
          <Facts items={auFacts(connection.au)} />
        </PanelGroup>
      )}
      <LiveStatus instrument={instrument} />
      <ResultSource instrument={instrument} />
      <Activity instrument={instrument} />
      <Changes instrument={instrument} />
    </DetailPanel>
  );
}

/**
 * Who registered the analyzer and who last changed its configuration, and when. Analyzers
 * registered before the app recorded this have neither; their audit events still do.
 */
function Changes({ instrument }: { instrument: InstrumentConfiguration }) {
  const person = usePersonText();
  const stamp = (change: ChangeStamp) => (
    <>
      <Timestamp value={change.at} /> by {person(change.by, change.byDisplay)}
    </>
  );
  const unchanged = instrument.created ? "Not changed since it was registered" : "Not recorded";
  return (
    <PanelGroup title="Changes">
      <Facts
        items={[
          {
            label: "Registered",
            value: instrument.created ? stamp(instrument.created) : "Not recorded",
          },
          {
            label: "Last changed",
            value: instrument.changed ? stamp(instrument.changed) : unchanged,
          },
        ]}
      />
    </PanelGroup>
  );
}

/** A result-file instrument's watched folder (F13), or how its files arrive without one. */
function WatchedFolder({ instrument }: { instrument: InstrumentConfiguration }) {
  if (instrument.resultFolder === undefined) {
    return "None. Files are loaded by hand under Result imports.";
  }
  return <span className={styles.code}>{instrument.resultFolder}</span>;
}

/**
 * Something read from the instrument's readiness under Bench setup: whether it may be used
 * (REQ-ANA-007) and its approved result source (REQ-ANA-003).
 */
function Readiness({
  instrumentId,
  children,
}: {
  instrumentId: string;
  children: (view: InstrumentReadinessView | undefined) => ReactNode;
}) {
  const readiness = useCommandQuery<InstrumentReadinessView[]>("list_instrument_readiness");
  if (readiness.isPending) return "Checking";
  if (readiness.isError) return "Status unavailable";
  return children(readiness.data.find((candidate) => candidate.instrumentId === instrumentId));
}

/**
 * What a new analyzer needs before a run, each opening its action in Bench setup: quality staff
 * set it in service, and its results need an approved source (a file template for result files).
 */
function NextSteps({ instrument }: { instrument: InstrumentConfiguration }) {
  const canSetStatus = useHasCapability("manage_quality");
  const path = benchInstrumentPath(instrument.id);
  return (
    <p>
      Next:{" "}
      {canSetStatus && (
        <>
          <Link to={path} state={benchStepState("set_instrument_status")}>
            Set in service
          </Link>
          {" · "}
        </>
      )}
      <Link to={path} state={benchStepState("register_source")}>
        {instrument.connection.kind === "result_file" ? "New file template" : "Register source"}
      </Link>
    </p>
  );
}
