import { useState } from "react";
import { Alert } from "@/components/Alert";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import type { SetInstrumentEnabledInput } from "@/generated/SetInstrumentEnabledInput";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { toCommandError, toUserMessage } from "@/lib/errors";
import { connectionText } from "@/lib/instrumentPages";
import { listenerDetailText } from "@/lib/listenerDetail";
import { INSTRUMENT_QUERIES } from "../contract";
import { pickupName } from "./connectionValues";

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

const counted = (count: number) => (count === 1 ? "1 instrument" : `${count} instruments`);
const named = (instrument: InstrumentConfiguration) =>
  `${instrument.name} (${connectionText(instrument.connection)})`;

interface BulkEnableDialogProps {
  /** Only the analyzers whose state changes. */
  targets: readonly InstrumentConfiguration[];
  enabling: boolean;
  onDone: (outcome: BulkOutcome) => void;
  onClose: () => void;
}

/**
 * Turns listening, or folder pickup, on or off for every analyzer not already in that state:
 * one `set_instrument_enabled` per analyzer, so each change is recorded and audited on its own.
 * They run one after another and a refusal does not stop the rest; the outcome names each one
 * refused and why. Every change restarts the listeners.
 */
export function BulkEnableDialog({ targets, enabling, onDone, onClose }: BulkEnableDialogProps) {
  const change = useCommandMutation<InstrumentChangeResult, { input: SetInstrumentEnabledInput }>(
    "set_instrument_enabled",
    { invalidates: INSTRUMENT_QUERIES },
  );
  const [isPending, setPending] = useState(false);
  const action = enabling ? "Enable all" : "Disable all";

  const confirm = async () => {
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
    onDone({ enabling, changed, refused, restartMessage });
    onClose();
  };

  return (
    <ConfirmDialog
      title={`${action}: ${counted(targets.length)}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={action}
      onConfirm={confirm}
      isPending={isPending}
      variant={enabling ? "primary" : "danger"}
    >
      <p>
        {enabling
          ? "The app starts receiving results from these instruments:"
          : "The app stops receiving results from these instruments until they are enabled again:"}
      </p>
      <ul>
        {targets.map((instrument) => (
          <li key={instrument.id}>
            {named(instrument)} — {pickupName(instrument.connection.kind)}
          </li>
        ))}
      </ul>
      <p>
        Each instrument is changed and recorded on its own. All analyzer listeners restart after
        each change; an analyzer that is sending at that moment may need to send again.
      </p>
    </ConfirmDialog>
  );
}

/**
 * The outcome under the page header: how many changed, each refused instrument with the
 * engine's reason and, after Enable all, each enabled one whose listener reports an error.
 */
export function BulkOutcomeAlert({
  outcome,
  onDismiss,
}: {
  outcome: BulkOutcome;
  onDismiss: () => void;
}) {
  const statuses = useCommandQuery<ListenerStatusView[]>("list_listener_status");
  const failing = outcome.enabling
    ? outcome.changed.flatMap((instrument) => {
        const status = statuses.data?.find((each) => each.instrumentId === instrument.id);
        if (status?.state !== "error") return [];
        const reason = listenerDetailText({ detail: status.lastError ?? status.detail });
        return [{ instrument, reason: reason ?? "Error" }];
      })
    : [];
  const failures = [...outcome.refused, ...failing];
  const verb = outcome.enabling ? "enabled" : "disabled";
  return (
    <Alert
      tone={failures.length === 0 && outcome.restartMessage === null ? "success" : "warning"}
      title={`${counted(outcome.changed.length)} ${verb}`}
      onDismiss={onDismiss}
    >
      {outcome.refused.length > 0 && (
        <p>
          Not {verb}: {counted(outcome.refused.length)}.
        </p>
      )}
      {failures.length > 0 && (
        <ul>
          {failures.map(({ instrument, reason }) => (
            <li key={instrument.id}>
              {named(instrument)}: {reason}
            </li>
          ))}
        </ul>
      )}
      {outcome.restartMessage && <p>{outcome.restartMessage}</p>}
    </Alert>
  );
}
