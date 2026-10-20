import { TextInput } from "@/components/FormFields";
import type { DerivationView } from "@/generated/DerivationView";
import type { ResolveBillingContextConflictInput } from "@/generated/ResolveBillingContextConflictInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { DERIVATION_QUERIES } from "../contract";

export function ContextResolutionDialog({
  derivation,
  onClose,
}: {
  derivation: DerivationView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ reason: string }, ResolveBillingContextConflictInput>
      title="Resolve billing context conflict"
      submitLabel="Record reconciliation"
      command={{ name: "resolve_billing_context_conflict", invalidates: DERIVATION_QUERIES }}
      defaultValues={{ reason: "" }}
      toInput={(values) => ({
        derivationId: derivation.derivationId,
        expectedRevision: derivation.revision,
        reason: values.reason.trim(),
      })}
      successTitle="Billing context reconciliation recorded"
      onClose={onClose}
    >
      <p>
        Agree the allocation with the external billing owner and record the decision. The combined
        performed service and all order contexts are retained; this action does not create
        additional services or claims.
      </p>
      <TextInput name="reason" label="Allocation decision, evidence and reason" isRequired />
    </CommandDialog>
  );
}
