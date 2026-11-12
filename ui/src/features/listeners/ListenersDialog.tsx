import { useState } from "react";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Dialog } from "@/components/Dialog";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import type { RunSummary } from "@/generated/RunSummary";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import {
  type BulkOutcome,
  BulkOutcomeAlert,
  bulkTargets,
  countedInstruments,
  useBulkEnable,
} from "@/lib/instrumentEnabling";
import { instrumentConnectionPath } from "@/lib/instrumentPages";
import {
  ListenerList,
  type ListenersQuery,
  RestartSummary,
  type RowControls,
} from "./ListenerList";

const NONE: readonly InstrumentConfiguration[] = [];

/** A change waiting for confirmation, with what it would interrupt. */
interface PendingChange {
  targets: readonly InstrumentConfiguration[];
  enabling: boolean;
  label: string;
  warnings: readonly string[];
}

/**
 * Every listener's state and last activity (REQ-OPS-013). People with `manage_configuration`
 * also enable or disable each analyzer, or all at once, and open its settings from its name
 * (ticket #4746); everyone else sees the states only.
 */
export function ListenersDialog({
  listeners,
  onClose,
}: {
  listeners: ListenersQuery;
  onClose: () => void;
}) {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const canManage = session.data?.capabilities.includes("manage_configuration") ?? false;
  const controls = useListenerControls(canManage, listeners.data, onClose);
  return (
    <Dialog
      title="Analyzer listeners"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      actions={
        <>
          {controls.bulkActions}
          <Button onPress={onClose}>Close</Button>
        </>
      }
    >
      <RestartSummary />
      {controls.notice}
      <ListenerList listeners={listeners} stacked controls={controls.row} />
      {controls.confirmation}
    </Dialog>
  );
}

/**
 * The window's actions. Each change is one `set_instrument_enabled` per analyzer, as on
 * Laboratory setup › Instruments, and restarts every listener. It is confirmed first when it
 * would interrupt something: an analyzer sending results now, or an open run on an analyzer
 * being disabled.
 */
function useListenerControls(
  canManage: boolean,
  statuses: readonly ListenerStatusView[] | undefined,
  onNavigate: () => void,
) {
  const instruments = useCommandQuery<InstrumentConfiguration[]>("list_instruments", undefined, {
    enabled: canManage,
  });
  const openRuns = useCommandQuery<RunSummary[]>(
    "list_runs",
    { input: { state: "open" } },
    { enabled: canManage },
  );
  const bulk = useBulkEnable();
  const [outcome, setOutcome] = useState<BulkOutcome | null>(null);
  const [pending, setPending] = useState<PendingChange | null>(null);
  if (!canManage) return { bulkActions: null, notice: null, row: undefined, confirmation: null };

  const all = instruments.data ?? NONE;
  const apply = async (targets: readonly InstrumentConfiguration[], enabling: boolean) => {
    const result = await bulk.apply(targets, enabling);
    setPending(null);
    setOutcome(result);
  };
  const request: Request = (targets, enabling, label) => {
    const warnings = interruptions(targets, enabling, statuses, openRuns.data);
    if (warnings.length === 0) void apply(targets, enabling);
    else setPending({ targets, enabling, label, warnings });
  };
  const allButton = (enabling: boolean) => {
    const targets = bulkTargets(all, enabling);
    if (targets.length === 0) return null;
    const label = enabling ? "Enable all" : "Disable all";
    return (
      <Button isPending={bulk.isPending} onPress={() => request(targets, enabling, label)}>
        {label}
      </Button>
    );
  };
  const row = rowControls(all, bulk.isPending, request, onNavigate);
  return {
    bulkActions: (
      <>
        {allButton(true)}
        {allButton(false)}
      </>
    ),
    notice: outcome && <BulkOutcomeAlert outcome={outcome} onDismiss={() => setOutcome(null)} />,
    row,
    confirmation: pending && (
      <InterruptConfirm
        change={pending}
        isPending={bulk.isPending}
        onConfirm={() => void apply(pending.targets, pending.enabling)}
        onClose={() => setPending(null)}
      />
    ),
  };
}

/** Asks for a change: which analyzers, on or off, and the label of the button that confirms it. */
type Request = (
  targets: readonly InstrumentConfiguration[],
  enabling: boolean,
  label: string,
) => void;

/** Each row's name as a link to the analyzer's settings, and its Enable or Disable button. */
function rowControls(
  all: readonly InstrumentConfiguration[],
  isPending: boolean,
  request: Request,
  onNavigate: () => void,
): RowControls {
  return {
    nameOf: (listener) => (
      <Link to={instrumentConnectionPath(listener.instrumentId)} onClick={onNavigate}>
        {listener.instrumentName}
      </Link>
    ),
    actionOf: (listener) => {
      const instrument = all.find((each) => each.id === listener.instrumentId);
      if (!instrument) return null;
      const label = instrument.enabled ? "Disable" : "Enable";
      return (
        <Button
          isDisabled={isPending}
          aria-label={`${label} ${instrument.name}`}
          onPress={() => request([instrument], !instrument.enabled, label)}
        >
          {label}
        </Button>
      );
    },
  };
}

/**
 * What the change would interrupt, in the words of the close warning: every listener restarts,
 * so any analyzer sending results now; and an open run on each analyzer being disabled. What
 * cannot be read is said too, so the change is confirmed rather than assumed safe.
 */
function interruptions(
  targets: readonly InstrumentConfiguration[],
  enabling: boolean,
  statuses: readonly ListenerStatusView[] | undefined,
  openRuns: readonly RunSummary[] | undefined,
): string[] {
  const warnings =
    statuses === undefined
      ? ["Whether results are coming in could not be checked."]
      : statuses
          .filter((status) => status.state === "receiving")
          .map((status) => `${status.instrumentName} is sending results now.`);
  if (enabling) return warnings;
  if (openRuns === undefined) return [...warnings, "Open runs could not be checked."];
  for (const instrument of targets) {
    const count = openRuns.filter((run) => run.instrumentId === instrument.id).length;
    if (count > 0) {
      warnings.push(
        `${instrument.name} has ${count === 1 ? "an open run" : `${count} open runs`}.`,
      );
    }
  }
  return warnings;
}

function InterruptConfirm({
  change,
  isPending,
  onConfirm,
  onClose,
}: {
  change: PendingChange;
  isPending: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const what =
    change.targets.length === 1
      ? (change.targets[0]?.name ?? "")
      : countedInstruments(change.targets.length);
  return (
    <ConfirmDialog
      title={`${change.label}: ${what}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={change.label}
      onConfirm={onConfirm}
      isPending={isPending}
      variant={change.enabling ? "primary" : "danger"}
    >
      <ul>
        {change.warnings.map((warning) => (
          <li key={warning}>{warning}</li>
        ))}
      </ul>
      <p>
        {change.enabling
          ? ""
          : "A disabled analyzer's results are not received until it is enabled again. "}
        All analyzer listeners restart after each change; an analyzer that is sending at that moment
        may need to send again.
      </p>
    </ConfirmDialog>
  );
}
