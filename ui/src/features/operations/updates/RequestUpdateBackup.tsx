import type { ReactNode } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Toolbar } from "@/components/Toolbar";
import { useCommandMutation, useHasCapability } from "@/lib/commands";
import { BACKUP_QUERIES, BACKUP_RESTORE, type BackupRunView, REQUEST_BACKUP } from "../contract";

/** Starts backup work where it is needed; verification still happens in the engine. */
export function RequestUpdateBackup({ children }: { children?: ReactNode }) {
  const canBackUp = useHasCapability(BACKUP_RESTORE);
  const request = useCommandMutation<BackupRunView, { input: { kind: "pre_update" } }>(
    REQUEST_BACKUP,
    { invalidates: BACKUP_QUERIES },
  );
  return (
    <>
      <Toolbar>
        {canBackUp && (
          <Button
            isPending={request.isPending}
            onPress={() => request.mutate({ input: { kind: "pre_update" } })}
          >
            Request pre-update backup
          </Button>
        )}
        {children}
      </Toolbar>
      {request.isSuccess && (
        <Alert tone="info">
          Pre-update backup requested. Wait for a verified copy in{" "}
          <Link to="/operations/backups/runs">Backups → Runs</Link>.
        </Alert>
      )}
      {request.error && <ErrorState title="Backup not requested" error={request.error} />}
    </>
  );
}
