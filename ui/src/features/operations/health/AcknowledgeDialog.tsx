import type { AppRunView } from "@/generated/AppRunView";
import { ACKNOWLEDGE_LISTENING_GAP, APP_RUN_QUERIES } from "../contract";
import { gapText } from "./AppRunPanel";
import { NoteDialog } from "./NoteDialog";

/** Records, with a note, that someone checked for results lost in a run's listening gap. */
export function AcknowledgeDialog({ run, onClose }: { run: AppRunView; onClose: () => void }) {
  return (
    <NoteDialog
      title="Acknowledge listening gap"
      submitLabel="Acknowledge"
      command={{ name: ACKNOWLEDGE_LISTENING_GAP, invalidates: APP_RUN_QUERIES }}
      toInput={(note) => ({ runId: run.runId, note })}
      successTitle="Listening gap acknowledged"
      onClose={onClose}
    >
      <p>{gapText(run)}</p>
    </NoteDialog>
  );
}
