import { useState } from "react";
import { Controller, useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { TextField } from "@/components/TextField";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { TestPartnerSftpInput } from "@/generated/TestPartnerSftpInput";
import { useCommandMutation } from "@/lib/commands";
import { textFieldProps } from "@/lib/form";
import row from "../actionRow.module.css";
import { GET_SFTP_KEY, TEST_SFTP } from "../contract";
import { isIncoming, sftpTestText } from "../labels";
import styles from "./ConnectionDialog.module.css";
import { type ConnectionValues, toSftp, typedPassword } from "./connectionForm";
import { CheckEveryInput } from "./IntakeSections";
import { toIntake } from "./intakeForm";

const SIGN_IN_OPTIONS = [
  { id: "key", label: "SSH key (recommended)" },
  { id: "password", label: "Password" },
];

const validPort = (value: string) =>
  /^\d+$/.test(value) && Number(value) >= 1 && Number(value) <= 65535
    ? true
    : "Use a port from 1 to 65535.";

/** The password, masked; a saved one is kept while the field is empty. */
function PasswordInput({ hasPassword }: { hasPassword: boolean }) {
  const { control } = useFormContext<ConnectionValues>();
  return (
    <Controller
      control={control}
      name="sftp.password"
      render={(controller) => (
        <TextField
          {...textFieldProps(controller)}
          label="Password"
          type="password"
          autoComplete="new-password"
          description={
            hasPassword
              ? "Leave empty to keep the saved password for this server and username."
              : "Kept sealed on this computer and never shown again."
          }
          isRequired={!hasPassword}
        />
      )}
    />
  );
}

/**
 * Section 3 for SFTP, as in mockup 6: the server, port, username and sign-in, the server's
 * fingerprint and the remote folder; Copy LIS public key (made on first use, for the partner's
 * IT to allow for the username) and Test connection, which connects, checks the fingerprint
 * and writes and removes a test file. Nothing is saved by either. For incoming orders the
 * remote folder is the one read, and the server's Processed and Failed folders and how often
 * it is read follow; the test then lists the input folder and writes in the other two.
 */
export function SftpFields({ connection }: { connection: PartnerConnection | null }) {
  const { control, getValues } = useFormContext<ConnectionValues>();
  const auth = useWatch({ control, name: "sftp.auth" });
  const incoming = isIncoming(useWatch({ control, name: "messageType" }));
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const getKey = useCommandMutation<string>(GET_SFTP_KEY, { invalidates: [] });
  const test = useCommandMutation<FolderTestResult, { input: TestPartnerSftpInput }>(TEST_SFTP, {
    invalidates: [],
  });
  const copyKey = () =>
    getKey.mutate(
      {},
      {
        onSuccess: (publicKey) =>
          void navigator.clipboard.writeText(publicKey).then(
            () =>
              setStatus({
                ok: true,
                text: "✓ LIS public key copied. Send it to the partner's IT to allow.",
              }),
            () => setStatus({ ok: true, text: `Copy this key for the partner's IT: ${publicKey}` }),
          ),
      },
    );
  const runTest = () => {
    const values = getValues("sftp");
    const password = typedPassword(values);
    const input: TestPartnerSftpInput = {
      server: toSftp(values),
      ...(password ? { password } : {}),
      ...(connection ? { connectionId: connection.id } : {}),
      ...(incoming ? { intake: toIntake(getValues("intake"), "sftp") } : {}),
    };
    test.mutate(
      { input },
      {
        onSuccess: (result) =>
          setStatus({ ok: result.problem === null, text: sftpTestText(result) }),
      },
    );
  };
  return (
    <>
      <div className={styles.row}>
        <TextInput name="sftp.host" label="Server" isRequired />
        <TextInput name="sftp.port" label="Port" isRequired validate={validPort} />
        <TextInput name="sftp.username" label="Username" isRequired />
        <ChoiceInput name="sftp.auth" label="Sign-in" options={SIGN_IN_OPTIONS} isRequired />
      </div>
      <div className={styles.pair}>
        <TextInput
          name="sftp.fingerprint"
          label="Server fingerprint"
          description="The LIS refuses to connect if the server's fingerprint changes."
          isRequired
        />
        <TextInput
          name="sftp.remoteFolder"
          label={incoming ? "Remote input folder" : "Remote folder"}
          description={
            incoming
              ? "The folder the LIS reads new files from."
              : "Files are uploaded as .tmp and renamed when complete."
          }
          isRequired
        />
      </div>
      {incoming && (
        <div className={styles.row}>
          <TextInput
            name="intake.remoteProcessedFolder"
            label="Remote processed folder"
            description="Read files move here, in year and month subfolders."
            isRequired
          />
          <TextInput
            name="intake.remoteFailedFolder"
            label="Remote failed folder"
            description="Files with errors move here; the reason is logged."
            isRequired
          />
          <CheckEveryInput />
        </div>
      )}
      {auth === "password" && (
        <div className={styles.half}>
          <PasswordInput hasPassword={connection?.hasPassword ?? false} />
        </div>
      )}
      <div className={row.row}>
        {auth === "key" && (
          <Button isPending={getKey.isPending} onPress={copyKey}>
            Copy LIS public key
          </Button>
        )}
        <Button isPending={test.isPending} onPress={runTest}>
          Test connection
        </Button>
        {status && (
          <span role="status" className={status.ok ? row.ok : row.problem}>
            {status.text}
          </span>
        )}
      </div>
      {getKey.error && <ErrorState title="Key not copied" error={getKey.error} />}
      {test.error && <ErrorState title="Connection not tested" error={test.error} />}
    </>
  );
}
