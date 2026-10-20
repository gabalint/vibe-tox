import { useWatch } from "react-hook-form";
import { ChoiceInput } from "@/components/FormFields";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { RegisterInstrumentInput } from "@/generated/RegisterInstrumentInput";
import type { UpdateInstrumentConnectionInput } from "@/generated/UpdateInstrumentConnectionInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { type Choice, checked, choiceOf } from "@/lib/choices";
import { CodedNameFields } from "../CodedNameFields";
import { INSTRUMENT_QUERIES } from "../contract";
import { ConnectionFields } from "./ConnectionFields";
import {
  type ConnectionValues,
  connectionValues,
  EMPTY_CONNECTION,
  toConnection,
} from "./connectionValues";

type Enabled = "enabled" | "disabled";

const LISTENING: readonly Choice<Enabled>[] = [
  { id: "enabled", label: "Listen for this analyzer" },
  { id: "disabled", label: "Keep disabled for now" },
];

const FOLDER_PICKUP: readonly Choice<Enabled>[] = [
  { id: "enabled", label: "Pick up files once a folder is chosen" },
  { id: "disabled", label: "Keep disabled for now" },
];

interface RegisterValues {
  code: string;
  name: string;
  enabled: string | null;
  connection: ConnectionValues;
}

interface ChangeDialogProps {
  onSaved: (result: InstrumentChangeResult) => void;
  onClose: () => void;
}

export function RegisterDialog({ onSaved, onClose }: ChangeDialogProps) {
  return (
    <CommandDialog<RegisterValues, RegisterInstrumentInput, InstrumentChangeResult>
      title="Register analyzer"
      submitLabel="Register analyzer"
      command={{ name: "register_instrument", invalidates: INSTRUMENT_QUERIES }}
      defaultValues={{ code: "", name: "", enabled: null, connection: EMPTY_CONNECTION }}
      toInput={(values) => ({
        code: values.code.trim(),
        name: values.name.trim(),
        connection: toConnection(values.connection),
        enabled: checked(choiceOf(LISTENING, values.enabled)) === "enabled",
      })}
      successTitle={(values) => `${values.name.trim()} registered`}
      onDone={onSaved}
      errorTitle="Not saved"
      onClose={onClose}
    >
      <CodedNameFields />
      <ConnectionFields />
      <EnabledField />
    </CommandDialog>
  );
}

export function ConnectionDialog({
  instrument,
  onSaved,
  onClose,
}: ChangeDialogProps & { instrument: InstrumentConfiguration }) {
  return (
    <CommandDialog<
      { connection: ConnectionValues },
      UpdateInstrumentConnectionInput,
      InstrumentChangeResult
    >
      title={`Edit connection of ${instrument.name}`}
      submitLabel="Save connection"
      command={{ name: "update_instrument_connection", invalidates: INSTRUMENT_QUERIES }}
      defaultValues={{ connection: connectionValues(instrument.connection) }}
      toInput={(values) => ({
        instrumentId: instrument.id,
        expectedRevision: instrument.revision,
        connection: toConnection(values.connection),
      })}
      successTitle={`Connection of ${instrument.name} saved`}
      onDone={onSaved}
      errorTitle="Not saved"
      onClose={onClose}
    >
      <ConnectionFields />
    </CommandDialog>
  );
}

/** Listening for an analyzer, or folder pickup for a result-file instrument. */
function EnabledField() {
  const kind = useWatch<RegisterValues, "connection.kind">({ name: "connection.kind" });
  const isFile = kind === "result_file";
  return (
    <ChoiceInput
      name="enabled"
      label={isFile ? "Folder pickup" : "Listening"}
      options={isFile ? FOLDER_PICKUP : LISTENING}
      isRequired
    />
  );
}
