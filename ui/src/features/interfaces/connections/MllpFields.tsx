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
import { mllpTestText } from "../mllpLabels";
import styles from "./ConnectionDialog.module.css";
import { type ConnectionValues, toMllp, validPort } from "./connectionForm";

const ACKNOWLEDGEMENTS = [{ id: "application", label: "Application ACK (AA / AE / AR)" }];

const validTimeout = (value: string) =>
  /^\d+$/.test(value) && Number(value) >= 1 && Number(value) <= 300
    ? true
    : "Use 1 to 300 seconds.";

/**
 * Section 3 for MLLP – LIS connects, laid out as the listener's section of mockup 2: the
 * partner's interface on the lab network, the acknowledgement the LIS waits for and how long;
 * the note that MLLP stays on the lab network; and Test connection, which connects and sends
 * nothing. Nothing is saved by it.
 */
export function MllpFields() {
  const { getValues } = useFormContext<ConnectionValues>();
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const test = useCommandMutation<FolderTestResult, { input: TestPartnerMllpInput }>(TEST_MLLP, {
    invalidates: [],
  });
  const runTest = () => {
    const partner = toMllp(getValues("mllp"));
    test.mutate(
      { input: { partner } },
      {
        onSuccess: (result) =>
          setStatus({ ok: result.problem === null, text: mllpTestText(result, partner) }),
      },
    );
  };
  return (
    <>
      <div className={styles.row}>
        <TextInput
          name="mllp.address"
          label="Partner address"
          description="Its IP address on the lab network, such as 10.20.0.15."
          isRequired
        />
        <TextInput name="mllp.port" label="Port" isRequired validate={validPort} />
        <ChoiceInput
          name="mllp.acknowledgement"
          label="Acknowledgement"
          options={ACKNOWLEDGEMENTS}
          isRequired
        />
        <TextInput
          name="mllp.ackTimeoutSeconds"
          label="ACK timeout (s)"
          description="1 to 300. Without an ACK in time, the message is sent again later."
          isRequired
          validate={validTimeout}
        />
      </div>
      <Alert tone="warning" compact>
        Lab network only. MLLP is not encrypted: the LIS connects only to an address on the lab
        network; outside partners use SFTP, or MLLP through the lab's VPN.
      </Alert>
      <div className={row.row}>
        <Button isPending={test.isPending} onPress={runTest}>
          Test connection
        </Button>
        {status && (
          <span role="status" className={status.ok ? row.ok : row.problem}>
            {status.text}
          </span>
        )}
      </div>
      {test.error && <ErrorState title="Connection not tested" error={test.error} />}
    </>
  );
}
