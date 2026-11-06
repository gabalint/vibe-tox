import { useState } from "react";
import { useFormContext } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { TestPartnerMllpInput } from "@/generated/TestPartnerMllpInput";
import { useCommandMutation } from "@/lib/commands";
import row from "../actionRow.module.css";
import { TEST_MLLP } from "../contract";
import { listenerTestText } from "../mllpLabels";
import styles from "./ConnectionDialog.module.css";
import { type ConnectionValues, validPort } from "./connectionForm";
import { toListener } from "./listenerForm";
import { type TestOutcome, TestStatus } from "./TestStatus";

const ACKNOWLEDGEMENTS = [{ id: "application", label: "Application ACK (AA / AE / AR)" }];
const CHARACTER_SETS = [{ id: "utf-8", label: "UTF-8" }];

const validIdle = (value: string) =>
  /^\d+$/.test(value) && Number(value) >= 10 && Number(value) <= 3600
    ? true
    : "Use 10 to 3600 seconds.";

/**
 * Section 3 for MLLP – LIS listens, as mockup 2: the address of this computer on the lab
 * network and the port the LIS listens on, the partner addresses allowed to connect, the
 * acknowledgement each message gets once saved, the character set and the idle timeout; the
 * note that the LIS listens on the lab network only; and Test listener, which checks the port
 * is free there and saves nothing.
 */
export function ListenerFields({ connectionId }: { connectionId: string | null }) {
  const { getValues } = useFormContext<ConnectionValues>();
  const [status, setStatus] = useState<TestOutcome | null>(null);
  const test = useCommandMutation<FolderTestResult, { input: TestPartnerMllpInput }>(TEST_MLLP, {
    invalidates: [],
  });
  const runTest = () => {
    const listener = toListener(getValues("mllpListener"));
    const own = connectionId === null ? {} : { connectionId };
    test.mutate(
      { input: { listener, ...own } },
      {
        onSuccess: (result) =>
          setStatus({ ok: result.problem === null, text: listenerTestText(result, listener) }),
      },
    );
  };
  return (
    <>
      <div className={styles.listen}>
        <TextInput
          name="mllpListener.address"
          label="Listen on address"
          description="This computer's IP address on the lab network."
          isRequired
        />
        <TextInput name="mllpListener.port" label="Port" isRequired validate={validPort} />
        <TextInput
          name="mllpListener.allowedPartners"
          label="Partner addresses allowed to connect"
          description="Comma separated, each with an optional note in brackets. Others are refused and logged."
          isRequired
        />
      </div>
      <div className={styles.row}>
        <ChoiceInput
          name="mllpListener.acknowledgement"
          label="Acknowledgement"
          options={ACKNOWLEDGEMENTS}
          isRequired
        />
        <ChoiceInput
          name="mllpListener.characterSet"
          label="Character set"
          options={CHARACTER_SETS}
          isRequired
        />
        <TextInput
          name="mllpListener.idleTimeoutSeconds"
          label="Idle timeout (s)"
          description="10 to 3600. A partner's connection that sends nothing this long is closed."
          isRequired
          validate={validIdle}
        />
      </div>
      <Alert tone="warning" compact>
        Lab network only. The LIS never listens on an internet address; outside partners use SFTP,
        or MLLP through the lab's VPN.
      </Alert>
      <div className={row.row}>
        <Button isPending={test.isPending} onPress={runTest}>
          Test listener
        </Button>
        <TestStatus status={status} />
      </div>
      {test.error && <ErrorState title="Listener not tested" error={test.error} />}
    </>
  );
}
