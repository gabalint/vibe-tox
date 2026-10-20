import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { SetInstrumentEnabledInput } from "@/generated/SetInstrumentEnabledInput";
import { useCommandMutation } from "@/lib/commands";
import { INSTRUMENT_QUERIES } from "../contract";
import { pickupName } from "./connectionValues";

interface EnableInstrumentDialogProps {
  instrument: InstrumentConfiguration;
  onSaved: (result: InstrumentChangeResult) => void;
  onClose: () => void;
}

/**
 * Turns listening for one analyzer, or folder pickup for a result-file instrument, on or off;
 * every listener restarts to apply it.
 */
export function EnableInstrumentDialog({
  instrument,
  onSaved,
  onClose,
}: EnableInstrumentDialogProps) {
  const enabling = !instrument.enabled;
  const isFile = instrument.connection.kind === "result_file";
  const action = `${enabling ? "Enable" : "Disable"} ${pickupName(instrument.connection.kind)}`;
  const change = useCommandMutation<InstrumentChangeResult, { input: SetInstrumentEnabledInput }>(
    "set_instrument_enabled",
    { invalidates: INSTRUMENT_QUERIES },
  );
  const confirm = () =>
    change.mutate(
      {
        input: {
          instrumentId: instrument.id,
          expectedRevision: instrument.revision,
          enabled: enabling,
        },
      },
      {
        onSuccess: (result) => {
          onSaved(result);
          onClose();
        },
      },
    );
  return (
    <ConfirmDialog
      title={`${action} for ${instrument.name}?`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={action}
      onConfirm={confirm}
      isPending={change.isPending}
      variant={enabling ? "primary" : "danger"}
    >
      <p>
        {effect(enabling, isFile)} All analyzer listeners restart to apply the change; an analyzer
        that is sending at that moment may need to send again.
      </p>
      {change.error && <ErrorState title="Not changed" error={change.error} />}
    </ConfirmDialog>
  );
}

function effect(enabling: boolean, isFile: boolean): string {
  if (isFile) {
    return enabling
      ? "The app starts picking up files from the watched folder."
      : "The app stops picking up files from the watched folder until folder pickup is enabled again. Files can still be loaded by hand.";
  }
  return enabling
    ? "The app starts listening for this analyzer's results."
    : "The app stops receiving results from this analyzer until listening is enabled again.";
}
