import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { useToast } from "@/components/Toast";
import type { ChosenResultFolder } from "@/generated/ChosenResultFolder";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ResultFolderInput } from "@/generated/ResultFolderInput";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { INSTRUMENT_QUERIES } from "../contract";

interface FolderDialogProps {
  instrument: InstrumentConfiguration;
  onClose: () => void;
}

const folderInput = (instrument: InstrumentConfiguration): ResultFolderInput => ({
  instrumentId: instrument.id,
  expectedRevision: instrument.revision,
});

/** What the person learns about the files already in a chosen folder (§6, F13). */
function presentFilesText(count: number): string | null {
  if (count === 0) return null;
  const files = count === 1 ? "1 file was" : `${count} files were`;
  return `${files} already in this folder. They will not be loaded automatically; load any of them by hand with Load results file.`;
}

/**
 * Chooses the folder the instrument's software exports to (F13). The app then opens the
 * system folder dialog; the engine checks that the folder can be read and that no other
 * instrument watches it, and records the files already in it, which are never picked up.
 * Cancelling the folder dialog changes nothing, and this dialog stays open so the folder can
 * be chosen again. When the folder already held files, the dialog says so before it closes.
 */
export function ChooseFolderDialog({ instrument, onClose }: FolderDialogProps) {
  const toast = useToast();
  const [cancelled, setCancelled] = useState(false);
  const [present, setPresent] = useState<string | null>(null);
  const choose = useCommandMutation<ChosenResultFolder | null, { input: ResultFolderInput }>(
    "choose_result_folder",
    { invalidates: INSTRUMENT_QUERIES },
  );
  const confirm = () => {
    setCancelled(false);
    choose.mutate(
      { input: folderInput(instrument) },
      {
        onSuccess: (chosen) => {
          if (chosen === null) {
            setCancelled(true);
            return;
          }
          const title = `Watched folder of ${chosen.instrument.name} saved`;
          const text = presentFilesText(chosen.presentFiles);
          if (text === null) {
            toast.show({ tone: "success", title });
            onClose();
          } else {
            setPresent(text);
          }
        },
      },
    );
  };
  if (present !== null) {
    return (
      <Dialog
        title={`Watched folder of ${instrument.name} saved`}
        isOpen
        onOpenChange={(isOpen) => {
          if (!isOpen) onClose();
        }}
        actions={<Button onPress={onClose}>Close</Button>}
      >
        <Alert tone="info">{present}</Alert>
      </Dialog>
    );
  }
  return (
    <ConfirmDialog
      title={`Choose the watched folder of ${instrument.name}`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel="Choose folder"
      onConfirm={confirm}
      isPending={choose.isPending}
      variant="primary"
    >
      <div className={styles.stack}>
        <p>
          Only files written to the folder after you choose it are loaded. Files already there are
          not loaded automatically. Use a dedicated export folder that only this instrument's
          software writes to.
        </p>
        <p>
          The app reads a file once it has stopped changing, and never moves or deletes it. Files
          are picked up while the app is open, including at the sign-in screen.
        </p>
        {cancelled && (
          <Alert tone="info">The folder dialog was cancelled. No folder was chosen.</Alert>
        )}
        {choose.error && <ErrorState title="Folder not saved" error={choose.error} />}
      </div>
    </ConfirmDialog>
  );
}

/** Stops watching the instrument's folder; its files are then loaded by hand. */
export function ClearFolderDialog({ instrument, onClose }: FolderDialogProps) {
  return (
    <CommandConfirmDialog<ResultFolderInput>
      title={`Stop watching the folder of ${instrument.name}?`}
      confirmLabel="Clear folder"
      variant="danger"
      command={{ name: "clear_result_folder", invalidates: INSTRUMENT_QUERIES }}
      input={folderInput(instrument)}
      successTitle={`${instrument.name} no longer watches a folder`}
      onClose={onClose}
    >
      <p>
        New files in the folder are no longer picked up. Load them by hand under Result imports.
        Files already received and their imports are kept.
      </p>
    </CommandConfirmDialog>
  );
}
