import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { SecurityPolicyView } from "@/generated/SecurityPolicyView";
import { useCommandQuery } from "@/lib/commands";
import { GET_SECURITY_POLICY } from "../contract";
import { SecurityPolicyDialog } from "./SecurityPolicyDialog";
import { SettingsFacts } from "./SettingsFacts";
import { type NumericSecurityPolicy, SECURITY_FIELDS } from "./settingFields";

export function SecurityPolicySection() {
  const view = useCommandQuery<SecurityPolicyView>(GET_SECURITY_POLICY);
  const [isOpen, setOpen] = useState(false);
  return (
    <Section
      title="Security policy"
      actions={view.data && <Button onPress={() => setOpen(true)}>Change security policy</Button>}
    >
      <QueryContent query={view} what="security policy">
        {(data) => (
          <>
            <SettingsFacts<NumericSecurityPolicy> fields={SECURITY_FIELDS} view={data} />
            <Facts
              items={[
                {
                  label: "Security key sign-in",
                  value: data.values.allowSecurityKeyTouchOnly
                    ? "Keys without a PIN are allowed"
                    : "PIN or biometric verification required (default)",
                },
              ]}
            />
          </>
        )}
      </QueryContent>
      {isOpen && view.data && (
        <SecurityPolicyDialog view={view.data} onClose={() => setOpen(false)} />
      )}
    </Section>
  );
}
