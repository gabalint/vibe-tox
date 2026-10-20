import { useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import type { BillingWorkItem } from "@/generated/BillingWorkItem";
import type { HandoffView } from "@/generated/HandoffView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { HANDOFF_QUERIES } from "../contract";
import { handoffPath } from "../paths";

/**
 * The bulk action of billing work: several approved derivations go into one handoff file,
 * each as its row's `allowedActions` permit. The engine rechecks each one before creating it
 * (REQ-BIL-002), then the new handoff opens.
 */
export function CreateHandoffAction({ selected }: { selected: readonly BillingWorkItem[] }) {
  const navigate = useNavigate();
  const [isConfirming, setConfirming] = useState(false);
  const derivationIds = selected.flatMap((row) =>
    row.derivationId && row.allowedActions.includes("create_billing_handoff")
      ? [row.derivationId]
      : [],
  );
  if (derivationIds.length === 0) return <span>None of these is approved for handoff.</span>;
  const count = derivationIds.length;
  return (
    <>
      <Button variant="primary" onPress={() => setConfirming(true)}>
        Create handoff
      </Button>
      {isConfirming && (
        <CommandConfirmDialog<{ derivationIds: string[] }, HandoffView>
          title="Create a handoff file?"
          confirmLabel="Create handoff"
          variant="primary"
          command={{ name: "create_billing_handoff", invalidates: HANDOFF_QUERIES }}
          input={{ derivationIds }}
          successTitle="Handoff created"
          onDone={(handoff) => navigate(handoffPath(handoff.handoffId))}
          onClose={() => setConfirming(false)}
        >
          <p>
            {`${count} approved ${count === 1 ? "derivation goes" : "derivations go"} into one handoff file for the billing partner. Each is handed off once; a later change becomes an adjustment.`}
          </p>
        </CommandConfirmDialog>
      )}
    </>
  );
}
