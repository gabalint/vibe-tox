import { useState } from "react";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { SetPartnerConnectionActive } from "@/generated/SetPartnerConnectionActive";
import { useCommandMutation } from "@/lib/commands";
import row from "../actionRow.module.css";
import {
  CONNECTION_QUERIES,
  RECORD_CHECK,
  SEND_TEST_MESSAGE,
  SET_ACTIVE,
  TEST_FOLDER,
  UPDATE_CONNECTION,
} from "../contract";
import { problemText } from "../labels";
import { testAnswerText } from "../mllpLabels";
import { CheckDialog } from "./CheckDialog";
import { ConnectionDialog } from "./ConnectionDialog";

type Open = "edit" | "active" | "check" | null;

/**
 * The connection's actions in one row under its description, as in the mockup, and as the
 * engine allows them: Edit, Stop or Start, the tests (which write no patient data), and
 * Record destination check while no check covers the current folder.
 */
export function ConnectionActions({ connection }: { connection: PartnerConnection }) {
  const [open, setOpen] = useState<Open>(null);
  // The latest test's result, on the action row as in the mockup; the next test replaces it.
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const allowed = (action: string) => connection.allowedActions.includes(action);
  const mllp = connection.settings.protocol === "mllp_connect";
  // An MLLP partner that answers AE or AR has the message, but did not accept it.
  const tested = (done: (outcome: FolderTestResult) => string) => (outcome: FolderTestResult) => {
    if (outcome.problem !== null) {
      setResult({ ok: false, text: `✗ Not done: ${problemText(outcome.problem)}.` });
      return;
    }
    const ok = outcome.ackCode === undefined || outcome.ackCode === "AA";
    setResult({ ok, text: `${ok ? "✓" : "✗"} ${done(outcome)}` });
  };
  const test = useCommandMutation<FolderTestResult, { input: { connectionId: string } }>(
    TEST_FOLDER,
    { invalidates: [] },
  );
  const sendTest = useCommandMutation<FolderTestResult, { input: { connectionId: string } }>(
    SEND_TEST_MESSAGE,
    { invalidates: CONNECTION_QUERIES },
  );
  const input = { connectionId: connection.id };
  const testError = test.error ?? sendTest.error;
  return (
    <>
      <div className={row.row}>
        {allowed(UPDATE_CONNECTION) && <Button onPress={() => setOpen("edit")}>Edit</Button>}
        {allowed(SET_ACTIVE) && (
          <Button onPress={() => setOpen("active")}>{connection.active ? "Stop" : "Start"}</Button>
        )}
        {allowed(TEST_FOLDER) && (
          <Button
            isPending={test.isPending}
            onPress={() =>
              test.mutate(
                { input },
                {
                  onSuccess: tested(() =>
                    mllp
                      ? "The partner's interface takes a connection; nothing was sent."
                      : "The LIS can create, write and remove files here.",
                  ),
                },
              )
            }
          >
            Test connection
          </Button>
        )}
        {allowed(SEND_TEST_MESSAGE) && (
          <Button
            isPending={sendTest.isPending}
            onPress={() =>
              sendTest.mutate(
                { input },
                {
                  onSuccess: tested((outcome) =>
                    mllp
                      ? testAnswerText(outcome)
                      : `Synthetic test message written as ${outcome.fileName}.`,
                  ),
                },
              )
            }
          >
            Send test message
          </Button>
        )}
        {allowed(RECORD_CHECK) && (
          <Button onPress={() => setOpen("check")}>Record destination check</Button>
        )}
        {result && (
          <span role="status" className={result.ok ? row.ok : row.problem}>
            {result.text}
          </span>
        )}
      </div>
      {testError && <ErrorState title="Test not run" error={testError} />}
      {open === "edit" && (
        <ConnectionDialog connection={connection} onClose={() => setOpen(null)} />
      )}
      {open === "check" && <CheckDialog connection={connection} onClose={() => setOpen(null)} />}
      {open === "active" && <ActiveDialog connection={connection} onClose={() => setOpen(null)} />}
    </>
  );
}

/** Stop or Start, with a reason. */
function ActiveDialog({
  connection,
  onClose,
}: {
  connection: PartnerConnection;
  onClose: () => void;
}) {
  const setActive = useCommandMutation<unknown, { input: SetPartnerConnectionActive }>(SET_ACTIVE, {
    invalidates: CONNECTION_QUERIES,
  });
  const stopping = connection.active;
  return (
    <ReasonConfirmDialog
      title={stopping ? "Stop connection" : "Start connection"}
      confirmLabel={stopping ? "Stop" : "Start"}
      variant="primary"
      command={setActive}
      input={(reason) => ({
        connectionId: connection.id,
        expectedRevision: connection.revision,
        active: !stopping,
        reason,
      })}
      successTitle={stopping ? "Connection stopped" : "Connection started"}
      onClose={onClose}
    >
      {stopping
        ? "A stopped connection writes nothing. Its messages wait and are written after it starts."
        : "Waiting messages are written once the destination check is recorded."}
    </ReasonConfirmDialog>
  );
}
