// Turning analyzers on or off, one or many at a time, and what came of it (REQ-INT-005): used by
// Laboratory setup › Instruments (ticket #4559) and the Analyzer listeners window (ticket #4746).
import { useState } from "react";
import { Alert } from "@/components/Alert";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import type { SetInstrumentEnabledInput } from "@/generated/SetInstrumentEnabledInput";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { toCommandError, toUserMessage } from "@/lib/errors";
import { connectionText, INSTRUMENT_QUERIES } from "@/lib/instrumentPages";
import { listenerDetailText } from "@/lib/listenerDetail";

/** What Enable all or Disable all did: the analyzers changed and those refused, with why. */
export interface BulkOutcome {
  enabling: boolean;
  changed: readonly InstrumentConfiguration[];
  refused: readonly { instrument: InstrumentConfiguration; reason: string }[];
  /** The engine's words when the last restart left a listener stopped. */
  restartMessage: string | null;
}

/** The analyzers Enable all (or Disable all) would change: the rest are already in that state. */
export const bulkTargets = (instruments: readonly InstrumentConfiguration[], enabling: boolean) =>
  instruments.filter((instrument) => instrument.enabled !== enabling);

export const countedInstruments = (count: number) =>
  count === 1 ? "1 instrument" : `${count} instruments`;
export const namedInstrument = (instrument: InstrumentConfiguration) =>
  `${instrument.name} (${connectionText(instrument.connection)})`;

/**
 * Turns listening, or folder pickup, on or off for each analyzer given: one
 * `set_instrument_enabled` per analyzer, so each change is recorded and audited on its own.
 * They run one after another and a refusal does not stop the rest; the outcome names each one
 * refused and why. Every change restarts the listeners.
 */
export function useBulkEnable() {
  const change = useCommandMutation<InstrumentChangeResult, { input: SetInstrumentEnabledInput }>(
    "set_instrument_enabled",
    { invalidates: INSTRUMENT_QUERIES },
  );
  const [isPending, setPending] = useState(false);
  const apply = async (
    targets: readonly InstrumentConfiguration[],
    enabling: boolean,
  ): Promise<BulkOutcome> => {
    setPending(true);
    const changed: InstrumentConfiguration[] = [];
    const refused: BulkOutcome["refused"][number][] = [];
    let restartMessage: string | null = null;
    for (const instrument of targets) {
      try {
        const result = await change.mutateAsync({
          input: {
            instrumentId: instrument.id,
            expectedRevision: instrument.revision,
            enabled: enabling,
          },
        });
        changed.push(result.instrument);
        restartMessage = result.listenersRestarted ? null : result.message;
      } catch (error) {
        refused.push({ instrument, reason: toUserMessage(toCommandError(error)) });
      }
    }
    setPending(false);
    return { enabling, changed, refused, restartMessage };
  };
  return { apply, isPending };
}

/** The listener states of an analyzer that started listening, or folder pickup. */
const STARTED: ReadonlySet<ListenerStatusView["state"]> = new Set([
  "listening",
  "connected",
  "receiving",
]);

/**
 * The outcome: how many changed, each refused instrument with the engine's reason and, after
 * enabling, which enabled ones are listening now and each one whose listener reports an error.
 */
export function BulkOutcomeAlert({
  outcome,
  onDismiss,
}: {
  outcome: BulkOutcome;
  onDismiss: () => void;
}) {
  const statuses = useCommandQuery<ListenerStatusView[]>("list_listener_status");
  const statusOf = (instrument: InstrumentConfiguration) =>
    statuses.data?.find((each) => each.instrumentId === instrument.id);
  const enabled = outcome.enabling ? outcome.changed : [];
  const started = enabled.filter((instrument) => {
    const state = statusOf(instrument)?.state;
    return state !== undefined && STARTED.has(state);
  });
  const failing = enabled.flatMap((instrument) => {
    const status = statusOf(instrument);
    if (status?.state !== "error") return [];
    const reason = listenerDetailText({ detail: status.lastError ?? status.detail });
    return [{ instrument, reason: reason ?? "Error" }];
  });
  const failures = [...outcome.refused, ...failing];
  const verb = outcome.enabling ? "enabled" : "disabled";
  return (
    <Alert
      tone={failures.length === 0 && outcome.restartMessage === null ? "success" : "warning"}
      title={`${countedInstruments(outcome.changed.length)} ${verb}`}
      onDismiss={onDismiss}
    >
      {started.length > 0 && (
        <p>Listening now: {started.map((instrument) => instrument.name).join(", ")}.</p>
      )}
      {outcome.refused.length > 0 && (
        <p>
          Not {verb}: {countedInstruments(outcome.refused.length)}.
        </p>
      )}
      {failures.length > 0 && (
        <ul>
          {failures.map(({ instrument, reason }) => (
            <li key={instrument.id}>
              {namedInstrument(instrument)}: {reason}
            </li>
          ))}
        </ul>
      )}
      {outcome.restartMessage && <p>{outcome.restartMessage}</p>}
    </Alert>
  );
}
