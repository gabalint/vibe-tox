import { useContext, useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Form } from "@/components/Form";
import { LoadingState } from "@/components/LoadingState";
import type { CreateFirstAdministratorInput } from "@/generated/CreateFirstAdministratorInput";
import type { KeyRecoveryResult } from "@/generated/KeyRecoveryResult";
import type { SecurityKeySignupInput } from "@/generated/SecurityKeySignupInput";
import type { StartupStatus } from "@/generated/StartupStatus";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { SETUP_STATUS } from "./contract";
import type { NewPasswordValues } from "./NewPasswordFields";
import { RestoreEntry } from "./RestoreEntry";
import { RestoreFlowContext } from "./restoreSlot";
import { SetupAccountFields } from "./SetupAccountFields";
import { SetupDataOptions } from "./SetupDataOptions";
import { SetupIntroduction } from "./SetupIntroduction";
import { SetupKeyRecovery } from "./SetupKeyRecovery";
import styles from "./setup.module.css";

interface SetupValues extends NewPasswordValues {
  username: string;
  displayName: string;
  people: string;
  loadDemoData: boolean;
  method: "password_totp" | "security_key";
  keyLabel: string;
}

/** First account and optional demo preparation (REQ-IDN-004, REQ-OPS-016, ADR 0015). */
export function SetupScreen({ onCreated }: { onCreated: () => void }) {
  const RestoreFlow = useContext(RestoreFlowContext);
  const [restoring, setRestoring] = useState(false);
  const [recoveryCode, setRecoveryCode] = useState<string | null>(null);
  const startup = useCommandQuery<StartupStatus>("startup_status");
  const isDemo = startup.data?.installation === "demo";
  const create = useCommandMutation<null, { input: CreateFirstAdministratorInput }>(
    "create_first_administrator",
    { invalidates: [SETUP_STATUS] },
  );
  const createKey = useCommandMutation<KeyRecoveryResult, { input: SecurityKeySignupInput }>(
    "security_key_signup",
    { invalidates: [] },
  );
  const form = useForm<SetupValues>({
    shouldUnregister: true,
    defaultValues: {
      username: "",
      displayName: "",
      newPassword: "",
      confirmPassword: "",
      people: "several",
      loadDemoData: true,
      method: "password_totp",
      keyLabel: "",
    },
  });
  const prepareDemo = isDemo && form.watch("loadDemoData");
  const pending = create.isPending || createKey.isPending;
  const keyRoute = form.watch("method") === "security_key";
  const onSubmit = form.handleSubmit(
    ({ username, displayName, newPassword, people, keyLabel, method }) => {
      const account = {
        username,
        displayName,
        onePerson: prepareDemo || people === "alone",
        ...(isDemo ? { loadDemoData: prepareDemo } : {}),
      };
      if (method === "security_key") {
        createKey.mutate(
          { input: { ...account, keyLabel, loadDemoData: isDemo ? prepareDemo : null } },
          {
            onSuccess: (result) => {
              setRecoveryCode(result.recoveryCode);
              createKey.reset();
            },
          },
        );
        return;
      }
      create.mutate(
        {
          input: {
            username,
            displayName,
            password: newPassword,
            onePerson: prepareDemo || people === "alone",
            ...(isDemo ? { loadDemoData: prepareDemo } : {}),
          },
        },
        { onSuccess: onCreated },
      );
    },
  );

  if (RestoreFlow && restoring) return <RestoreFlow onExit={() => setRestoring(false)} />;
  if (startup.isPending) return <LoadingState label="Checking the laboratory…" />;
  if (startup.isError)
    return (
      <ErrorState
        title="The laboratory could not be identified"
        error={startup.error}
        onRetry={() => void startup.refetch()}
      />
    );
  if (recoveryCode) return <SetupKeyRecovery code={recoveryCode} />;
  return (
    <div className={styles.setup} data-first-run>
      <header className={styles.heading}>
        <h1>{isDemo ? "Set up your demo laboratory" : "Set up this laboratory"}</h1>
        <p>
          {isDemo
            ? "Create your own account and choose your starting data. Everything stays on this computer."
            : "Explore with sample data, or create an account for an empty laboratory."}
        </p>
      </header>
      <RestoreEntry part="notices" />
      <div className={styles.grid} data-demo={isDemo || undefined}>
        <SetupIntroduction isDemo={isDemo} />
        <section className={styles.account} aria-labelledby="setup-account-title">
          <div className={styles.heading}>
            <h2 id="setup-account-title">
              {isDemo ? "Create your demo account" : "Start an empty laboratory"}
            </h2>
            <p>Enter your details to create the first administrator on this computer.</p>
          </div>
          <FormProvider {...form}>
            <Form
              aria-label="Create the first administrator"
              onSubmit={onSubmit}
              error={keyRoute ? createKey.error : create.error}
              errorTitle="The administrator was not created"
              actions={
                <Button type="submit" variant="primary" isPending={pending}>
                  {isDemo ? "Create demo account" : "Create administrator"}
                </Button>
              }
            >
              <fieldset className={styles.fields} disabled={pending} aria-label="Account details">
                <SetupAccountFields />
                <SetupDataOptions isDemo={isDemo} isPending={pending} />
              </fieldset>
              <p className={styles.next}>
                {keyRoute
                  ? "Next: verify your security key, then save your recovery code."
                  : "Next: sign in and connect an authenticator app to secure your account."}
              </p>
            </Form>
          </FormProvider>
        </section>
      </div>
      <div className={styles.restore}>
        <RestoreEntry part="action" onStart={RestoreFlow ? () => setRestoring(true) : undefined} />
      </div>
    </div>
  );
}
