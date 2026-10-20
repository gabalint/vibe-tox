import { useWatch } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { CheckList } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { SecurityPolicyView } from "@/generated/SecurityPolicyView";
import type { SetSecurityPolicyInput } from "@/generated/SetSecurityPolicyInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { CURRENT_SESSION } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { GET_SECURITY_POLICY, SET_SECURITY_POLICY } from "../contract";
import { SettingsFields } from "./SettingsDialog";
import { type NumericSecurityPolicy, SECURITY_FIELDS } from "./settingFields";

type PolicyValues = Record<keyof NumericSecurityPolicy, string> & {
  allowSecurityKeyTouchOnly: string[];
  reason: string;
  password: string;
};

const TOUCH_ONLY = "allow";

export function SecurityPolicyDialog({
  view,
  onClose,
}: {
  view: SecurityPolicyView;
  onClose: () => void;
}) {
  const toInput = (values: PolicyValues): SetSecurityPolicyInput => {
    const allowSecurityKeyTouchOnly = values.allowSecurityKeyTouchOnly.includes(TOUCH_ONLY);
    return {
      expectedVersion: view.version,
      idleTimeoutMinutes: Number(values.idleTimeoutMinutes.trim()),
      lockoutFailures: Number(values.lockoutFailures.trim()),
      lockoutMinutes: Number(values.lockoutMinutes.trim()),
      allowSecurityKeyTouchOnly,
      reason: view.version !== null || allowSecurityKeyTouchOnly ? values.reason.trim() : null,
      password:
        allowSecurityKeyTouchOnly !== view.values.allowSecurityKeyTouchOnly
          ? values.password
          : null,
    };
  };
  return (
    <CommandDialog<PolicyValues, SetSecurityPolicyInput, SecurityPolicyView>
      title="Change security policy"
      submitLabel="Save settings"
      command={{
        name: SET_SECURITY_POLICY,
        invalidates: [GET_SECURITY_POLICY, CURRENT_SESSION, "get_authentication_status"],
      }}
      defaultValues={{
        idleTimeoutMinutes: String(view.values.idleTimeoutMinutes),
        lockoutFailures: String(view.values.lockoutFailures),
        lockoutMinutes: String(view.values.lockoutMinutes),
        allowSecurityKeyTouchOnly: view.values.allowSecurityKeyTouchOnly ? [TOUCH_ONLY] : [],
        reason: "",
        password: "",
      }}
      toInput={toInput}
      successTitle="Security policy saved"
      onClose={onClose}
    >
      <SettingsFields<NumericSecurityPolicy> fields={SECURITY_FIELDS} view={view} nested={false} />
      <CheckList
        name="allowSecurityKeyTouchOnly"
        legend="Security key sign-in"
        options={[
          {
            id: TOUCH_ONLY,
            label: "Allow security keys without a PIN for sign-in",
            details: ["Off by default: require the key's PIN or biometric verification."],
          },
        ]}
      />
      <PolicyConfirmation view={view} />
    </CommandDialog>
  );
}

function PolicyConfirmation({ view }: { view: SecurityPolicyView }) {
  const selected: string[] = useWatch({ name: "allowSecurityKeyTouchOnly" });
  const allowed = selected.includes(TOUCH_ONLY);
  const changed = allowed !== view.values.allowSecurityKeyTouchOnly;
  return (
    <>
      {allowed && (
        <Alert tone="warning" title="Possession of a key can be enough to sign in">
          A lost or unattended key could let someone use its owner's account. Newly registered keys
          may use touch only; keys already requiring a PIN or biometric check keep that protection.
          Sensitive actions still require a PIN or biometric check.
        </Alert>
      )}
      {changed && (
        <p>
          Confirm this change with your password or a key's PIN or biometric check. All other
          laboratory sessions will end; this session stays signed in. Turning this option off
          requires PIN or biometric verification for keys previously enrolled without it.
        </p>
      )}
      {(view.version !== null || allowed) && <ReasonField />}
      {changed && <AuthenticationField />}
    </>
  );
}
