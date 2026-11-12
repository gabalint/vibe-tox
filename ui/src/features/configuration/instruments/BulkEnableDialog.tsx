import { ConfirmDialog } from "@/components/ConfirmDialog";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import {
  type BulkOutcome,
  countedInstruments,
  namedInstrument,
  useBulkEnable,
} from "@/lib/instrumentEnabling";
import { pickupName } from "./connectionValues";

interface BulkEnableDialogProps {
  /** Only the analyzers whose state changes. */
  targets: readonly InstrumentConfiguration[];
  enabling: boolean;
  onDone: (outcome: BulkOutcome) => void;
  onClose: () => void;
}

/** Confirms Enable all or Disable all for every analyzer not already in that state. */
export function BulkEnableDialog({ targets, enabling, onDone, onClose }: BulkEnableDialogProps) {
  const bulk = useBulkEnable();
  const action = enabling ? "Enable all" : "Disable all";

  const confirm = async () => {
    onDone(await bulk.apply(targets, enabling));
    onClose();
  };

  return (
    <ConfirmDialog
      title={`${action}: ${countedInstruments(targets.length)}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={action}
      onConfirm={confirm}
      isPending={bulk.isPending}
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
            {namedInstrument(instrument)} — {pickupName(instrument.connection.kind)}
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
