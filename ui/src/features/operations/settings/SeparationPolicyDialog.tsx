import { CheckList } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { SeparationPolicyView } from "@/generated/SeparationPolicyView";
import type { SetSeparationPolicyInput } from "@/generated/SetSeparationPolicyInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import { GET_SEPARATION_POLICY, SET_SEPARATION_POLICY } from "../contract";
import { DUTIES } from "./separationDuties";

interface PolicyValues {
  /** The duties one person may do alone. */
  allowed: string[];
  reason: string;
  password: string;
}

const OPTIONS = DUTIES.map(({ id, label, details }) => ({ id, label, details: [details] }));

/**
 * Sets the next version of the separation-of-duties policy (ADR 0015), prefilled from the one
 * in force. It carries the version shown, so a change made meanwhile is refused; it needs a
 * reason and the person's password again. The engine checks `manage_quality` and records it.
 */
export function SeparationPolicyDialog({
  view,
  onClose,
}: {
  view: SeparationPolicyView;
  onClose: () => void;
}) {
  const toInput = ({ allowed, reason, password }: PolicyValues): SetSeparationPolicyInput => ({
    expectedVersion: view.version,
    values: {
      selfApproveConfiguration: allowed.includes("selfApproveConfiguration"),
      selfReleaseReports: allowed.includes("selfReleaseReports"),
      selfVerifyDestinations: allowed.includes("selfVerifyDestinations"),
    },
    reason: reason.trim(),
    password,
  });
  return (
    <CommandDialog<PolicyValues, SetSeparationPolicyInput, SeparationPolicyView>
      title="Change separation of duties"
      submitLabel="Save policy"
      command={{ name: SET_SEPARATION_POLICY, invalidates: [GET_SEPARATION_POLICY] }}
      defaultValues={{
        allowed: DUTIES.filter(({ id }) => view.values[id]).map(({ id }) => id),
        reason: "",
        password: "",
      }}
      toInput={toInput}
      successTitle="Separation of duties saved"
      onClose={onClose}
    >
      <CheckList name="allowed" legend="One person may do both steps of" options={OPTIONS} />
      <p>A duty left unticked needs a second person.</p>
      <ReasonField />
      <AuthenticationField />
    </CommandDialog>
  );
}
