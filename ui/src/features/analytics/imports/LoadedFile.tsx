import { useNavigate } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Facts } from "@/components/Facts";
import { LoadingState } from "@/components/LoadingState";
import { Timestamp } from "@/components/Timestamp";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { LoadedResultFile } from "@/generated/LoadedResultFile";
import { useCommandQuery } from "@/lib/commands";
import { importAttemptPath } from "../paths";
import { ImportOutcomeBadge } from "./ImportOutcomeBadge";

/** How often the import list is read again while a received file's import is awaited. */
const POLL_MS = 1_000;

const TITLES: Record<LoadedResultFile["outcome"], string> = {
  received: "File received",
  duplicate: "File already received",
  too_large: "File not read",
};

/**
 * What became of a loaded file (F12, F14): received, with its import attempt once the outbox
 * has processed it; already received, pointing to the earlier attempt; or too large to read.
 */
export function LoadedFile({ file, onClose }: { file: LoadedResultFile; onClose: () => void }) {
  const navigate = useNavigate();
  // The receipt whose import attempt to offer: this one, or the earlier one of a duplicate.
  const receiptId = file.outcome === "received" ? file.receiptId : file.duplicateOf;
  const attempts = useCommandQuery<ImportAttemptSummary | null>(
    "get_receipt_import_attempt",
    { input: { receiptId } },
    { enabled: receiptId !== null },
  );
  const attempt = attempts.data ?? undefined;
  const waiting = receiptId !== null && attempt === undefined && !attempts.isError;
  return (
    <Dialog
      title={TITLES[file.outcome]}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      actions={
        <>
          {attempt && (
            <Button
              variant="primary"
              onPress={() => navigate(importAttemptPath(attempt.importAttemptId))}
            >
              {file.outcome === "duplicate"
                ? "Open the earlier import attempt"
                : "Open import attempt"}
            </Button>
          )}
          <Button onPress={onClose}>Close</Button>
        </>
      }
    >
      {waiting && receiptId && <ImportPoller receiptId={receiptId} />}
      <Facts
        items={[
          { label: "File", value: file.fileName },
          { label: "Size", value: `${file.byteLen.toLocaleString("en-US")} bytes` },
          { label: "Received", value: <Timestamp value={file.receivedAt} /> },
        ]}
      />
      <Outcome file={file} attempt={attempt} waiting={waiting} unlisted={attempts.isError} />
    </Dialog>
  );
}

/**
 * A second reader of the exact receipt outcome that reads it again every second; mounted only
 * while the attempt has not appeared, so reading stops once it has (or the dialog closes).
 */
function ImportPoller({ receiptId }: { receiptId: string }) {
  useCommandQuery(
    "get_receipt_import_attempt",
    { input: { receiptId } },
    { refetchInterval: POLL_MS },
  );
  return null;
}

interface OutcomeProps {
  file: LoadedResultFile;
  attempt: ImportAttemptSummary | undefined;
  waiting: boolean;
  /** The import list cannot be read in this session. */
  unlisted: boolean;
}

function Outcome({ file, attempt, waiting, unlisted }: OutcomeProps) {
  if (file.outcome === "too_large") {
    return (
      <Alert tone="danger" title="The file is larger than the 20 MiB the LIS reads">
        It was not read and nothing was imported. A receipt records that it arrived.
      </Alert>
    );
  }
  if (file.outcome === "duplicate") {
    return (
      <Alert tone="info" title="The same file was received before from this instrument">
        Nothing was imported again. To read it again after a template change, use Retry import on
        the earlier import attempt.
      </Alert>
    );
  }
  if (attempt) {
    return (
      <Facts
        items={[{ label: "Import", value: <ImportOutcomeBadge outcome={attempt.outcome} /> }]}
      />
    );
  }
  if (unlisted)
    return (
      <p>
        The receipt is retained, but its processing outcome could not be loaded. Close this dialog
        and inspect Result imports; receiving a file does not establish successful processing.
      </p>
    );
  return waiting ? <LoadingState label="Importing the file…" /> : null;
}
