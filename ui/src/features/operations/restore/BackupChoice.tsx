import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { BackupSetView } from "@/generated/BackupSetView";
import type { CommandError } from "@/generated/CommandError";
import { SET_COLUMNS } from "../setColumns";

/** The first column, the time the backup was made, is what people pick by. */
const COLUMNS = SET_COLUMNS.map((column, index) =>
  index === 0 ? { ...column, identifier: true } : column,
);

interface BackupChoiceProps {
  sets: readonly BackupSetView[];
  onCheck: (setId: string) => void;
  isChecking: boolean;
  /** The engine's refusal of the last check. */
  error: CommandError | null;
}

/**
 * The backups found in the chosen folder. Choosing one is a pick in a task flow, not a
 * selection: a click or Enter on its row chooses it, the row is highlighted and named, and one
 * compatible backup is checked in full.
 */
export function BackupChoice({ sets, onCheck, isChecking, error }: BackupChoiceProps) {
  const [chosenId, setChosenId] = useState<string | null>(null);
  const chosen = sets.find((set) => set.setId === chosenId);
  if (sets.length === 0) {
    return (
      <EmptyState
        title="No backups in this folder"
        description="Choose the folder that holds this laboratory's backups."
      />
    );
  }
  return (
    <>
      <DataGrid
        label="Backups in the folder"
        columns={COLUMNS}
        rows={sets}
        getRowId={(set) => set.setId}
        openRowId={chosenId}
        onRowAction={(set) => setChosenId(set.setId)}
      />
      <Toolbar>
        <Button
          variant="primary"
          isDisabled={!chosen?.compatible}
          isPending={isChecking}
          onPress={() => chosen && onCheck(chosen.setId)}
        >
          Check this backup
        </Button>
        {chosen && (
          <span>
            Chosen: backup made <Timestamp value={chosen.createdAt} />, app {chosen.appVersion}
            {chosen.compatible ? "" : ". It is not compatible with this app."}
          </span>
        )}
      </Toolbar>
      {isChecking && (
        <Alert tone="info" title="Checking the backup">
          The app decrypts, opens and verifies every record and stored file in this backup. This can
          take several minutes.
        </Alert>
      )}
      {error && <ErrorState title="The backup could not be checked" error={error} />}
    </>
  );
}
