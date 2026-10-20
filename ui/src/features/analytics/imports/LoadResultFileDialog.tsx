import { useState } from "react";
import { useForm } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { ErrorState } from "@/components/ErrorState";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput } from "@/components/FormFields";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LoadedResultFile } from "@/generated/LoadedResultFile";
import type { LoadResultFileInput } from "@/generated/LoadResultFileInput";
import { checked } from "@/lib/choices";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { IMPORT_QUERIES, LIST_INSTRUMENT_READINESS } from "../contract";
import { LoadedFile } from "./LoadedFile";

/**
 * Loads one results file by hand (result files brief §6, F16; REQ-INT-009), for people with
 * `perform_analysis`. Only instruments whose approved source is a file template are offered.
 * The instrument is chosen first, then the file in the system dialog; the engine stores its
 * exact bytes as a receipt before anything reads them, and the import follows. Cancelling
 * the file dialog loads nothing, and this dialog stays open.
 */
export function LoadResultFileDialog({ onClose }: { onClose: () => void }) {
  const [cancelled, setCancelled] = useState(false);
  const [loaded, setLoaded] = useState<LoadedResultFile | null>(null);
  const form = useForm<{ instrumentId: string | null }>({ defaultValues: { instrumentId: null } });
  const instrumentId = form.watch("instrumentId");
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const load = useCommandMutation<LoadedResultFile | null, { input: LoadResultFileInput }>(
    "load_result_file",
    { invalidates: IMPORT_QUERIES },
  );
  const submit = form.handleSubmit((values) => {
    setCancelled(false);
    load.mutate(
      { input: { instrumentId: checked(values.instrumentId) } },
      { onSuccess: (file) => (file === null ? setCancelled(true) : setLoaded(file)) },
    );
  });
  if (loaded !== null) return <LoadedFile file={loaded} onClose={onClose} />;
  const withTemplate = (instruments.data ?? []).filter(
    (instrument) => instrument.activeRegistration?.fileTemplate === true,
  );
  const options = withTemplate.map(({ instrumentId: id, name }) => ({ id, label: name }));
  const template = withTemplate.find(
    (row) => row.instrumentId === instrumentId,
  )?.activeRegistration;
  return (
    <FormDialog
      form={form}
      title="Load results file"
      submitLabel="Choose file"
      errorTitle="The file was not loaded"
      command={load}
      onSubmit={submit}
      onClose={onClose}
    >
      <p>
        Choose the instrument whose software exported the file. The file is read with that
        instrument's approved file template, and is never changed or moved.
      </p>
      <ChoiceInput name="instrumentId" label="Instrument" options={options} isRequired />
      {instruments.error && (
        <ErrorState title="The instruments could not be loaded" error={instruments.error} />
      )}
      {instruments.data && options.length === 0 && (
        <Alert tone="warning">No instrument has an approved file template yet.</Alert>
      )}
      {template && (
        <p>
          Read with the approved template of {template.sender} {template.version}.
        </p>
      )}
      {cancelled && <Alert tone="info">The file dialog was cancelled. No file was loaded.</Alert>}
    </FormDialog>
  );
}
