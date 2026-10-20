import { Controller, useFormContext } from "react-hook-form";
import { ChoiceInput } from "@/components/FormFields";
import { TextField } from "@/components/TextField";
import { textFieldProps } from "@/lib/form";
import { KeyNameField } from "./KeyNameField";
import { NewPasswordFields } from "./NewPasswordFields";
import styles from "./setup.module.css";

/** Account details reused by the empty-laboratory and demo first-run paths. */
export function SetupAccountFields() {
  const { control, watch } = useFormContext();
  const key = watch("method") === "security_key";
  return (
    <>
      <div className={styles.fieldGrid}>
        <Controller
          control={control}
          name="displayName"
          rules={{ required: "Enter the name shown in the app and the audit trail." }}
          render={(controller) => (
            <TextField
              {...textFieldProps(controller)}
              label="Display name"
              description="Your name in the app and audit trail."
              autoComplete="name"
              isRequired
            />
          )}
        />
        <Controller
          control={control}
          name="username"
          rules={{ required: "Enter a username." }}
          render={(controller) => (
            <TextField
              {...textFieldProps(controller)}
              label="Username"
              description="The name you will use to sign in."
              autoComplete="username"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              isRequired
            />
          )}
        />
      </div>
      <ChoiceInput
        name="method"
        label="Sign-in method"
        options={[
          { id: "password_totp", label: "Password + authenticator app" },
          { id: "security_key", label: "Security key" },
        ]}
        description="A security key uses its own PIN or biometric verification. No separate LIS password or authenticator app is needed."
      />
      {key ? (
        <KeyNameField />
      ) : (
        <div className={styles.fieldGrid}>
          <NewPasswordFields label="Password" engineField="password" />
        </div>
      )}
    </>
  );
}
